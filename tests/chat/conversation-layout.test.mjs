import { test } from "node:test";
import assert from "node:assert/strict";
import { build } from "esbuild";
import { chromium } from "@playwright/test";
import { fileURLToPath } from "node:url";

test("conversation preserves prepend anchor and isolates workspace/session layout", async () => {
  const mocks = {
    DisplayConversationEntry: `export default ({entry}) => <div style={{height: 120}}>{entry.content}</div>;`,
    ApprovalForm: `export const ApprovalFormProvider = ({children}) => children;`,
    EntriesContext: `const reset = () => {}; const setEntries = () => {}; const setUsage = () => {}; export const useEntriesActions = () => ({reset, setEntries}); export const useSetTokenUsageInfo = () => setUsage;`,
    useResetProcess: `export const useResetProcess = () => ({});`,
    useConversationHistory: `export const useConversationHistory = ({onTimelineUpdated}) => { window.publish = onTimelineUpdated; return {isFirstTurn: false, isLoadingHistory: false}; };`,
    ScriptFixerDialog: `export const ScriptFixerDialog = {};`,
  };
  const bundle = await build({
    entryPoints: [
      fileURLToPath(
        new URL("./conversation-layout.fixture.tsx", import.meta.url),
      ),
    ],
    bundle: true,
    write: false,
    jsx: "automatic",
    format: "iife",
    alias: {
      "@": fileURLToPath(
        new URL("../../packages/web-core/src", import.meta.url),
      ),
    },
    nodePaths: [
      fileURLToPath(
        new URL("../../packages/web-core/node_modules", import.meta.url),
      ),
    ],
    plugins: [
      {
        name: "layout-fixture",
        setup(builder) {
          builder.onResolve(
            {
              filter:
                /DisplayConversationEntry$|ApprovalForm$|EntriesContext$|useResetProcess$|hooks\/useConversationHistory$|ScriptFixerDialog$/,
            },
            (args) => ({
              path: args.path.split("/").at(-1),
              namespace: "mock",
            }),
          );
          builder.onLoad({ filter: /.*/, namespace: "mock" }, (args) => ({
            contents: mocks[args.path],
            resolveDir: fileURLToPath(
              new URL("../../packages/web-core", import.meta.url),
            ),
            loader: "tsx",
          }));
        },
      },
    ],
  });
  const browser = await chromium.launch({
    headless: true,
    args: ["--no-sandbox"],
  });
  try {
    const page = await browser.newPage();
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.setContent(
      `<style>html,body,#root{height:100%;margin:0}.h-full{height:100%}.overflow-hidden{overflow:hidden}.overflow-y-auto{overflow-y:auto}</style><div id="root"></div>`,
    );
    await page.addScriptTag({ content: bundle.outputFiles[0].text });
    await page.waitForFunction(() => window.publish);
    await page.evaluate(
      () => (
        window.publish(window.source(80, 99), "initial", false),
        window.publish(window.source(80, 100), "running", false)
      ),
    );
    await page.waitForFunction(() =>
      document.body.textContent.includes("a-99"),
    );
    await page.waitForFunction(() => {
      const el = document.querySelector(".overflow-y-auto");
      return Math.abs(el.scrollHeight - el.clientHeight - el.scrollTop) <= 2;
    });
    await page.evaluate(() => {
      const el = document.querySelector(".overflow-y-auto");
      el.dispatchEvent(new WheelEvent("wheel", { deltaY: -100 }));
      // Outside the 64px bottom lock, with offscreen tail rows above us.
      el.scrollTop -= 80;
    });
    await page.waitForTimeout(100);
    const anchor = await page.evaluate(() => {
      const el = document.querySelector(".overflow-y-auto");
      const row = [...el.querySelectorAll("[data-semantic-key]")].find(
        (node) => node.getBoundingClientRect().bottom > 0,
      );
      return {
        key: row.dataset.semanticKey,
        top: row.getBoundingClientRect().top,
      };
    });
    await page.evaluate(() =>
      window.publish(window.source(60, 100), "running", false),
    );
    await page.waitForTimeout(200);
    const top = await page.evaluate(
      (key) =>
        [...document.querySelectorAll("[data-semantic-key]")]
          .find((node) => node.dataset.semanticKey === key)
          ?.getBoundingClientRect().top,
      anchor.key,
    );
    assert.ok(
      Math.abs(top - anchor.top) <= 2,
      `prepend moved anchor: ${anchor.top} -> ${top}`,
    );
    // Simulate an image/DOM resize without publishing entries or a React update.
    // The changed row must belong to the unvirtualized tail, above the viewport.
    await page.evaluate(() => {
      const row = [...document.querySelectorAll("[data-semantic-key]")].find(
        (node) =>
          !node.hasAttribute("data-index") &&
          node.getBoundingClientRect().bottom < 0,
      );
      if (!row) throw new Error("No offscreen unvirtualized tail row");
      window.delayedTailResizeDone = false;
      setTimeout(() => {
        row.firstElementChild.style.height = "360px";
        window.delayedTailResizeDone = true;
      }, 50);
    });
    await page.waitForFunction(() => window.delayedTailResizeDone);
    await page.waitForTimeout(200);
    const resizedTop = await page.evaluate(
      (key) =>
        [...document.querySelectorAll("[data-semantic-key]")]
          .find((node) => node.dataset.semanticKey === key)
          ?.getBoundingClientRect().top,
      anchor.key,
    );
    assert.ok(
      Math.abs(resizedTop - anchor.top) <= 2,
      `delayed tail resize moved anchor: ${anchor.top} -> ${resizedTop}`,
    );
    await page.evaluate(() =>
      window.publish(window.source(40, 100), "initial", false),
    );
    await page.waitForTimeout(150);
    const lateTop = await page.evaluate(
      (key) =>
        [...document.querySelectorAll("[data-semantic-key]")]
          .find((node) => node.dataset.semanticKey === key)
          ?.getBoundingClientRect().top,
      anchor.key,
    );
    assert.ok(
      Math.abs(lateTop - anchor.top) <= 2,
      `late initial replay moved anchor: ${anchor.top} -> ${lateTop}`,
    );
    await page.evaluate(() => {
      window.publish(window.source(0, 100), "running", false);
      window.setScope("b");
    });
    await page.waitForTimeout(100);
    await page.evaluate(() =>
      window.publish(window.source(0, 2, "b"), "initial", false),
    );
    await page.waitForFunction(() => document.body.textContent.includes("b-1"));
    assert.equal(await page.getByText("a-99", { exact: true }).count(), 0);
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});
