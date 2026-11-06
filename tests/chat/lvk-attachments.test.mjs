import test from "node:test";
import assert from "node:assert/strict";
import { build } from "esbuild";
import { chromium } from "@playwright/test";
import { fileURLToPath } from "node:url";

test("production attachment nodes restore and serialize canonical LVK paths", async () => {
  const bundle = await build({
    entryPoints: [
      fileURLToPath(new URL("./lvk-attachments.fixture.tsx", import.meta.url)),
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
    const page = await browser.newPage();
    await page.addScriptTag({ content: bundle.outputFiles[0].text });
    for (const [markdown, type] of [
      ["[report.pdf](.lvk/attachments/report.pdf)", "attachment"],
      ["![image](.lvk/attachments/image.png)", "image"],
      ["[file](attachment://file-id)", "attachment"],
      ["[pending](pending-attachment://file-id)", "attachment"],
    ]) {
      const result = await page.evaluate(
        (value) => window.parseAttachments(value),
        markdown,
      );
      assert.equal(result.markdown, markdown);
      assert.equal(result.state.root.children[0].children[0].type, type);
    }
  } finally {
    await browser.close();
  }
});
