import { test } from "node:test";
import assert from "node:assert/strict";
import { build } from "esbuild";
import { chromium } from "@playwright/test";
import { fileURLToPath } from "node:url";

test("collapsed activity, editor focus during updates, and IME submission", async () => {
  const result = await build({
    entryPoints: [
      fileURLToPath(new URL("./editor-display.fixture.tsx", import.meta.url)),
    ],
    bundle: true,
    write: false,
    format: "iife",
    jsx: "automatic",
    alias: {
      "@": fileURLToPath(
        new URL("../../packages/web-core/src", import.meta.url),
      ),
    },
    plugins: [
      {
        name: "isolated-draft",
        setup(builder) {
          builder.onResolve(
            { filter: /useScratch$|useHostId$|HostIdProvider$|useAppRuntime$/ },
            (args) => ({ path: args.path, namespace: "draft-mock" }),
          );
          builder.onLoad(
            { filter: /.*/, namespace: "draft-mock" },
            ({ path }) => ({
              contents: path.endsWith("useScratch")
                ? "export const useScratch = () => ({ ...window.scratchSnapshot, updateScratch: async () => {}, deleteScratch: async () => {} });"
                : path.endsWith("useAppRuntime")
                  ? `export const useAppRuntime = () => 'local';`
                  : `export const useHostId = () => undefined;`,
            }),
          );
        },
      },
    ],
    nodePaths: [
      fileURLToPath(
        new URL("../../packages/web-core/node_modules", import.meta.url),
      ),
    ],
  });
  const browser = await chromium.launch({
    headless: true,
    args: ["--no-sandbox"],
  });
  try {
    const page = await browser.newPage();
    const errors = [];
    page.on("pageerror", (e) => errors.push(e.message));
    await page.setContent('<div id="root"></div>');
    await page.addScriptTag({ content: result.outputFiles[0].text });
    const pending = page.locator("#pending-composer");
    await pending.fill("読み込み中の入力");
    assert.equal(await pending.textContent(), "読み込み中の入力");
    assert.equal(
      await page.evaluate(() => window.pendingDraft.isScratchLoading),
      true,
    );
    await page.evaluate(() =>
      window.setScratchSnapshot({
        isLoading: false,
        scratch: {
          payload: { type: "DRAFT_FOLLOW_UP", data: { message: "古い下書き" } },
        },
      }),
    );
    await page.waitForFunction(() => !window.pendingDraft.isScratchLoading);
    assert.equal(await pending.textContent(), "読み込み中の入力");
    // A controlled restore with trailing blank paragraphs is normalized by
    // Lexical after render. That echo must not count as a newer user draft.
    await page.evaluate(() => window.restoreAndSubmit("改行付き\n\n"));
    await pending.focus();
    await page.waitForTimeout(50);
    await page.evaluate(() => window.finishSend(true));
    await page.waitForFunction(
      () => document.querySelector("#pending-composer").textContent === "",
      null, {timeout: 2000}
    );
    // The keyboard flush emits the same serialization once more before send.
    // This must not invalidate the acknowledgement revision on selection updates.
    await pending.fill("送信内容");
    await pending.press("Shift+Enter");
    await page.evaluate(() => window.pendingEnter(false));
    await page.waitForTimeout(50);
    assert.ok((await page.evaluate(() => window.sent)).startsWith("送信内容"));
    await page.evaluate(() => window.finishSend(true));
    await page.waitForFunction(
      () => document.querySelector("#pending-composer").textContent === "",
      null, {timeout: 2000}
    );
    await pending.fill("失敗時の下書き");
    await page.evaluate(() => window.pendingEnter(false));
    await page.evaluate(() => window.finishSend(false));
    assert.equal(await pending.textContent(), "失敗時の下書き");
    await page.evaluate(() => window.pendingEnter(false));
    await pending.fill("送信待ち中の追加入力");
    await page.evaluate(() => window.finishSend(true));
    assert.equal(await pending.textContent(), "送信待ち中の追加入力");
    const composer = page.locator("#composer");
    await composer.fill("入力");
    await page.evaluate(() => window.setDraft("遅れて復元された別の入力欄"));
    await page.waitForFunction(() =>
      document.querySelector("#other-draft").textContent.includes("復元"),
    );
    assert.equal(
      await composer.evaluate((e) => e === document.activeElement),
      true,
      "background draft restoration must not steal focus",
    );
    await page.evaluate(() => window.setLog("実行ログの更新"));
    await page.waitForFunction(() =>
      document.querySelector("#log").textContent.includes("更新"),
    );
    assert.equal(
      await composer.evaluate((e) => e === document.activeElement),
      true,
      "streaming output must not steal focus",
    );
    await page.keyboard.type("abc");
    assert.equal(await composer.textContent(), "入力abc");
    await page.evaluate(() => {
      window.enter(true);
      window.enter(false, false, true);
    });
    assert.equal(
      await page.evaluate(() => window.submissions),
      0,
      "IME confirmation must not submit",
    );
    await page.evaluate(() => window.setShortcut("ModifierEnter"));
    await page.waitForTimeout(50);
    await page.evaluate(() => window.enter(true, true));
    assert.equal(await page.evaluate(() => window.submissions), 0);
    await page.evaluate(() => window.enter(false, true));
    assert.equal(await page.evaluate(() => window.submissions), 1);
    await page.evaluate(() => window.setValue(""));
    await page.waitForFunction(
      () => document.querySelector("#composer").textContent === "",
    );
    for (const text of [
      "検索結果",
      "ファイルの内容",
      "確認した内容",
      "単独の出力",
    ]) {
      assert.equal(await page.getByText(text, { exact: true }).count(), 0);
    }
    const tools = page.getByRole("button", { name: "ツール呼び出し完了" });
    assert.equal(await tools.getAttribute("aria-expanded"), "false");
    await tools.click();
    const search = page.getByRole("button", { name: "検索", exact: true });
    await search.click();
    assert.equal(
      await page.getByText("rg needle", { exact: true }).isVisible(),
      true,
    );
    assert.equal(
      await page.getByText("検索結果", { exact: true }).isVisible(),
      true,
    );
    await search.click();
    assert.equal(await page.getByText("検索結果", { exact: true }).count(), 0);
    await page.getByRole("button", { name: "読み込み", exact: true }).click();
    assert.equal(
      await page.getByText("ファイルの内容", { exact: true }).isVisible(),
      true,
    );
    await tools.click();
    assert.equal(
      await page.getByText("ファイルの内容", { exact: true }).count(),
      0,
    );
    const thinking = page.getByRole("button", { name: "思考内容" });
    await thinking.click();
    assert.equal(
      await page.getByText("確認した内容", { exact: true }).isVisible(),
      true,
    );
    await thinking.click();
    assert.equal(
      await page.getByText("確認した内容", { exact: true }).count(),
      0,
    );
    const single = page.getByRole("button", { name: "単独コマンド" });
    await single.click();
    assert.equal(
      await page.getByText("単独の出力", { exact: true }).isVisible(),
      true,
    );
    await single.click();
    assert.equal(
      await page.getByText("単独の出力", { exact: true }).count(),
      0,
    );
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});
