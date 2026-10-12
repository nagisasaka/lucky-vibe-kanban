import { test } from "node:test";
import assert from "node:assert/strict";
import { build } from "esbuild";
import { chromium } from "@playwright/test";
import { fileURLToPath } from "node:url";

test("Astra isolates typing, follows latest, preserves reading position and shows lifecycle", async () => {
  const base = fileURLToPath(
    new URL("../../packages/web-core/src", import.meta.url),
  );
  const mocks = {
    useAppNavigation: "export const useAppNavigation = () => ({});",
    useTheme:
      "export const useTheme = () => ({theme:'light'}); export const getResolvedTheme = () => 'light';",
    AsyncAgentQuestions: "export const AsyncAgentQuestions = () => null;",
    agentRunApi: "export const agentRunsApi = {};",
    api: "export const handleApiResponse = async r => r;",
    localApiTransport: `export const makeLocalApiRequest = async (url, options) => {
   if (options?.method === 'POST') return 'accepted';
   const after = Number(new URL(url, 'http://test').searchParams.get('after'));
   return {...window.astraView, messages:window.astraView.messages.filter(m => m.sequence > after)};
  };`,
    MarkdownPreview: `import React from 'react'; export function MarkdownPreview({content}) {window.markdownRenders=(window.markdownRenders||0)+1; return <p style={{height:40}}>{content}</p>;}`,
  };
  const result = await build({
    entryPoints: [
      fileURLToPath(new URL("./astra.fixture.tsx", import.meta.url)),
    ],
    bundle: true,
    write: false,
    format: "iife",
    jsx: "automatic",
    alias: { "@": base },
    nodePaths: [base + "/../node_modules"],
    plugins: [
      {
        name: "isolate",
        setup(b) {
          b.onResolve(
            {
              filter:
                /\/(useAppNavigation|useTheme|AsyncAgentQuestions|agentRunApi|api|localApiTransport|MarkdownPreview)$/,
            },
            (a) => ({ path: a.path.split("/").at(-1), namespace: "mock" }),
          );
          b.onLoad({ filter: /.*/, namespace: "mock" }, (a) => ({
            contents: mocks[a.path],
            loader: "tsx",
            resolveDir: base,
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
    page.on("pageerror", (e) => errors.push(e.message));
    await page.route("https://astra.test/", (route) =>
      route.fulfill({
        contentType: "text/html",
        body: '<div id="root"></div>',
      }),
    );
    await page.goto("https://astra.test/");
    await page.setContent(
      '<style>[aria-label="Conversation history"]{height:250px;overflow-y:auto} article{min-height:60px}</style><div id="root"></div>',
    );
    await page.addScriptTag({ content: result.outputFiles[0].text });
    const history = page.getByLabel("Conversation history");
    await page.waitForFunction(
      () => document.querySelectorAll("article").length === 100,
    );
    const bottom = () =>
      history.evaluate(
        (e) => e.scrollHeight - e.scrollTop - e.clientHeight < 65,
      );
    await page.waitForFunction(() => {
      const e = document.querySelector('[aria-label="Conversation history"]');
      return e.scrollTop > 0;
    });
    assert.ok(await bottom());
    const renders = await page.evaluate(() => window.markdownRenders);
    await page
      .getByLabel("Message to Astra")
      .pressSequentially("typing input test");
    assert.equal(
      await page.evaluate(() => window.markdownRenders),
      renders,
      "typing must not render history",
    );
    await history.evaluate((e) => {
      e.scrollTop = 0;
      e.dispatchEvent(new Event("scroll", { bubbles: true }));
    });
    await page.getByRole("button", { name: "最新へ" }).waitFor();
    await page.evaluate(async () => {
      window.astraView.messages.push({
        id: "new",
        sequence: 101,
        role: "user",
        status: "pending",
        content: "New",
        created_at: new Date(Date.now() - 65000).toISOString(),
      });
      await window.astraRefresh();
    });
    assert.equal(await history.evaluate((e) => e.scrollTop), 0);
    await page.getByText("順番待ち…", { exact: true }).waitFor();
    await page.getByText(/受付から 1分/).waitFor();
    await page.getByRole("button", { name: "最新へ" }).click();
    assert.ok(await bottom());
    await page.evaluate(async () => {
      window.astraView.messages.at(-1).status = "running";
      await window.astraRefresh();
    });
    await page.getByText("Astra が処理中…", { exact: true }).waitFor();
    await page.evaluate(async () => {
      Object.assign(window.astraView.messages.at(-1), {
        status: "pending",
        error: "retry",
      });
      await window.astraRefresh();
    });
    await page.getByText("接続エラー・再試行待ち", { exact: true }).waitFor();
    await page.evaluate(async () => {
      window.astraView.messages.at(-1).status = "done";
      window.astraView.messages.push({
        id: "reply",
        sequence: 102,
        role: "assistant",
        status: "done",
        content: "Reply",
        created_at: new Date().toISOString(),
      });
      await window.astraRefresh();
    });
    await page.getByText("Reply", { exact: true }).waitFor();
    assert.ok(await bottom());
    assert.equal(await page.getByText(/受付から/).count(), 0);
    await history.evaluate((e) => {
      e.scrollTop = 0;
      e.dispatchEvent(new Event("scroll", { bubbles: true }));
    });
    await page.getByRole("button", { name: "最新へ" }).waitFor();
    await page
      .getByRole("button", { name: "送信 / Send", exact: true })
      .click();
    await page.waitForFunction(
      () => document.querySelector("textarea").value === "",
    );
    assert.ok(await bottom(), "sending returns to latest");
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});
