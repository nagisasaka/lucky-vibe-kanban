import { test } from "node:test";
import assert from "node:assert/strict";
import { build } from "esbuild";
import { chromium } from "@playwright/test";
import { fileURLToPath } from "node:url";

test("conversation preserves prepend anchor and isolates workspace/session layout", async () => {
  const mocks = {
    DisplayConversationEntry: `export default ({entry}) => <div style={{height: entry.content === "stream-long" ? 700 : entry.content === "terminal" ? 48 : 120}}>{entry.content}</div>;`,
    ApprovalForm: `export const ApprovalFormProvider = ({children}) => children;`,
    EntriesContext: `const reset = () => {}; const setEntries = () => {}; const setUsage = () => {}; export const useEntriesActions = () => ({reset, setEntries}); export const useSetTokenUsageInfo = () => setUsage;`,
    useResetProcess: `export const useResetProcess = () => ({});`,
    useConversationHistory: `import {useEffect} from "react"; export const useConversationHistory = ({onTimelineUpdated}) => { useEffect(()=>{window.publish = onTimelineUpdated},[onTimelineUpdated]); return {isFirstTurn: false, isLoadingHistory: false}; };`,
    WorkspaceTokenSummary: `export const WorkspaceTokenSummary = () => <div style={{height:24}}>Workspace tokens</div>;`,
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
                /DisplayConversationEntry$|ApprovalForm$|EntriesContext$|useResetProcess$|hooks\/useConversationHistory$|ScriptFixerDialog$|WorkspaceTokenSummary$/,
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
    await page
      .waitForFunction(() => window.publish, null, { timeout: 3000 })
      .catch((e) => {
        throw new Error(errors.join("\n") || e.message);
      });
    await page.evaluate(
      () => (
        window.publish(window.source(80, 99), "initial", false),
        window.publish(window.source(80, 100), "running", false)
      ),
    );
    await page
      .waitForFunction(() => document.body.textContent.includes("a-99"), null, {
        timeout: 3000,
      })
      .catch((e) => {
        throw new Error(errors.join("\n") || e.message);
      });
    await page.waitForFunction(() => {
      const el = document.querySelector(".overflow-y-auto");
      return Math.abs(el.scrollHeight - el.clientHeight - el.scrollTop) <= 2;
    });
    // Exact public sequence: read history, send/jump, grow response, terminal.
    await page.evaluate(() => {
      const el = document.querySelector(".overflow-y-auto");
      el.dispatchEvent(new WheelEvent("wheel", { deltaY: -800 }));
      el.scrollTop -= 800;
    });
    await page.waitForTimeout(80);
    await page.evaluate(() => {
      window.jump();
      const source = window.source(80, 101);
      source.canonical.isRunning = true;
      source.canonical.entries.at(-1).content.content = "stream-long";
      window.publish(source, "running", false);
    });
    await page.waitForTimeout(120);
    await page.evaluate(() => {
      const source = window.source(80, 102);
      source.canonical.entries.at(-1).content.content = "stream-long";
      window.publish(source, "running", false);
    });
    await page.waitForTimeout(200);
    assert.ok(
      await page.evaluate(() => {
        const el = document.querySelector(".overflow-y-auto");
        return Math.abs(el.scrollHeight - el.clientHeight - el.scrollTop) <= 2;
      }),
      "own send from history must keep following through response and terminal",
    );
    // A measurement/anchor adjustment is not user intent. It may produce a
    // separate scroll event after the resize itself has settled.
    await page.evaluate(() => {
      const el = document.querySelector(".overflow-y-auto");
      el.scrollTop -= 120;
    });
    await page.waitForTimeout(60);
    await page.evaluate(() => {
      const source = window.source(80, 103);
      source.canonical.entries.at(-1).content.content = "stream-long";
      window.publish(source, "running", false);
    });
    await page.waitForTimeout(160);
    assert.ok(
      await page.evaluate(() => {
        const el = document.querySelector(".overflow-y-auto");
        return Math.abs(el.scrollHeight - el.clientHeight - el.scrollTop) <= 2;
      }),
      "layout scroll without user input must not cancel follow intent",
    );
    await page.evaluate(() =>
      window.publish(window.source(80, 100), "running", false),
    );
    await page.waitForTimeout(100);
    await page.evaluate(() => {
      const el = document.querySelector(".overflow-y-auto");
      el.dispatchEvent(new WheelEvent("wheel", { deltaY: -100 }));
      // Outside the 64px bottom lock, with offscreen tail rows above us.
      el.scrollTop -= 80;
    });
    await page.waitForTimeout(100);
    // One touch gesture changes direction, then continues with momentum.
    // Each resulting viewport position must replace the reading anchor.
    await page.evaluate(() => {
      const el = document.querySelector(".overflow-y-auto");
      const touch = (type, y) => {
        const event = new Event(type);
        Object.defineProperty(event, "touches", { value: [{ clientY: y }] });
        el.dispatchEvent(event);
      };
      touch("touchstart", 100);
      touch("touchmove", 150);
      el.scrollTop -= 80;
      window.touchDown = () => {
        touch("touchmove", 120);
        el.scrollTop += 30;
      };
    });
    await page.waitForTimeout(40);
    await page.evaluate(() => window.touchDown());
    await page.waitForTimeout(40);
    await page.evaluate(
      () => (document.querySelector(".overflow-y-auto").scrollTop += 20),
    );
    await page.waitForTimeout(40);
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
      const source = window.source(0, 120);
      source.canonical.isRunning = true;
      source.canonical.entries.at(-1).content.content = "stream-long";
      window.publish(source, "running", false);
    });
    await page.waitForTimeout(150);
    await page.evaluate(() => window.jump());
    await page.waitForTimeout(50);
    await page.evaluate(() => {
      const source = window.source(0, 116);
      source.canonical.entries.at(-1).content.content = "terminal";
      window.publish(source, "running", false);
    });
    await page.waitForTimeout(200);
    assert.ok(
      await page.evaluate(() => {
        const el = document.querySelector(".overflow-y-auto");
        return Math.abs(el.scrollHeight - el.clientHeight - el.scrollTop) < 2;
      }),
      "terminal shrink and virtualization must retain latest",
    );
    await page.evaluate(() => {
      const el = document.querySelector(".overflow-y-auto");
      el.dispatchEvent(new WheelEvent("wheel", { deltaY: -500 }));
      el.scrollTop -= 500;
    });
    await page.waitForTimeout(80);
    const readingTop = await page.evaluate(
      () => document.querySelector(".overflow-y-auto").scrollTop,
    );
    await page.evaluate(() => {
      const source = window.source(0, 116);
      source.canonical.entries.at(-1).content.content = "stream-long";
      window.publish(source, "running", false);
    });
    await page.waitForTimeout(150);
    assert.ok(
      Math.abs(
        (await page.evaluate(
          () => document.querySelector(".overflow-y-auto").scrollTop,
        )) - readingTop,
      ) < 2,
      "manual reading must survive terminal tail remeasure",
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
