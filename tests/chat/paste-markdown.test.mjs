import test from "node:test";
import assert from "node:assert/strict";
import { build } from "esbuild";
import { chromium } from "@playwright/test";
import { fileURLToPath } from "node:url";

test("browser paste keeps Markdown conversion, HTML and raw-paste shortcuts", async () => {
  const bundle = await build({
    entryPoints: [
      fileURLToPath(new URL("./paste-markdown.fixture.tsx", import.meta.url)),
    ],
    bundle: true,
    write: false,
    format: "iife",
    jsx: "automatic",
    nodePaths: [
      fileURLToPath(
        new URL("../../packages/local-web/node_modules", import.meta.url),
      ),
    ],
  });
  const browser = await chromium.launch({
    headless: true,
    args: ["--no-sandbox"],
  });
  try {
    for (const scenario of [
      {
        plain: "**Markdown**",
        html: "",
        raw: false,
        expected: "Markdown",
        bold: true,
      },
      {
        plain: "HTML",
        html: "<b>HTML</b>",
        raw: false,
        expected: "HTML",
        bold: true,
      },
      {
        plain: "**raw**",
        html: "<b>raw</b>",
        raw: true,
        expected: "**raw**",
        bold: false,
      },
    ]) {
      const page = await browser.newPage();
      const errors = [];
      page.on("pageerror", (error) => errors.push(error.message));
      await page.setContent('<div id="root"></div>');
      await page.addScriptTag({ content: bundle.outputFiles[0].text });
      const editor = page.getByRole("textbox");
      await editor.focus();
      await editor.evaluate((element, input) => {
        // Exercise the native ClipboardEvent path without clipboard permissions
        // or a desktop bridge. The OS clipboard itself is not accessed.
        window.dispatchEvent(
          new KeyboardEvent("keydown", {
            key: "v",
            ctrlKey: true,
            shiftKey: input.raw,
            bubbles: true,
          }),
        );
        const clipboardData = new DataTransfer();
        clipboardData.setData("text/plain", input.plain);
        if (input.html) clipboardData.setData("text/html", input.html);
        element.dispatchEvent(
          new ClipboardEvent("paste", {
            clipboardData,
            bubbles: true,
            cancelable: true,
          }),
        );
        window.dispatchEvent(
          new KeyboardEvent("keyup", { key: "v", bubbles: true }),
        );
      }, scenario);
      await page.waitForFunction(
        (expected) =>
          document.querySelector("[contenteditable]")?.textContent === expected,
        scenario.expected,
      );
      assert.equal(
        (await editor.locator("strong, b").count()) > 0,
        scenario.bold,
      );
      assert.deepEqual(errors, []);
      await page.close();
    }
  } finally {
    await browser.close();
  }
});
