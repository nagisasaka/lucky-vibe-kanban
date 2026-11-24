import { test } from "node:test";
import assert from "node:assert/strict";
import { build } from "esbuild";
import { chromium, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
const require = createRequire(`${root}/packages/web-core/package.json`);
const bundle = await build({
  entryPoints: [`${root}/tests/file-viewer/frontmatter.fixture.tsx`],
  bundle: true,
  write: false,
  format: "iife",
  jsx: "automatic",
  alias: { "@": `${root}/packages/web-core/src` },
  nodePaths: [`${root}/packages/web-core/node_modules`],
  define: { "import.meta.env": "{}", "import.meta.hot": "undefined" },
});
const config = require("tailwindcss/loadConfig")(
  `${root}/packages/local-web/tailwind.new.config.js`,
);
const rawCss = await readFile(
  `${root}/packages/web-core/src/app/styles/new/index.css`,
  "utf8",
);
const css = await require("postcss")([
  require("tailwindcss")({
    ...config,
    content: [`${root}/packages/{ui,web-core}/src/**/*.{tsx,ts}`],
  }),
]).process(rawCss.replace(/^@(import|config).*$/gm, ""), { from: undefined });
const issue = await readFile(
  `${root}/docs/changes/2026-11-24-LOCAL-460-ca49b042.md`,
  "utf8",
);
const general =
  '---\ntitle: Metadata title\ntags: [one, two]\noptions:\n  draft: true\ndescription: |\n  First line\n  Second line\nunsafe: "<img src=x onerror=alert(1)>"\n---\n# Article\n\nBody text\n\n---\n\nAfter rule';

test("file Markdown metadata collapse, expansion, source and document switching", async () => {
  const browser = await chromium.launch({
    headless: true,
    args: ["--no-sandbox"],
  });
  try {
    for (const width of [1440, 390])
      for (const dark of [false, true]) {
        const page = await browser.newPage({
          viewport: { width, height: 844 },
          colorScheme: dark ? "dark" : "light",
        });
        const errors = [];
        page.on("pageerror", (e) => errors.push(e.message));
        await page.route("https://fixture.invalid/**", (r) =>
          r.fulfill({
            contentType: "text/html",
            body: '<div id="root"></div>',
          }),
        );
        await page.goto("https://fixture.invalid");
        await page.addStyleTag({ content: css.css });
        await page.evaluate(
          (dark) => document.documentElement.classList.toggle("dark", dark),
          dark,
        );
        await page.addScriptTag({ content: bundle.outputFiles[0].text });
        await page.waitForFunction(() => window.showMarkdown);
        for (const [key, content] of [
          ["issue", issue],
          ["general", general],
        ]) {
          await page.evaluate(
            ({ key, content }) => window.showMarkdown(content, key),
            { key, content },
          );
          const metadata = page
            .locator("details")
            .filter({
              has: page.locator("summary", { hasText: "メタデータ" }),
            });
          await expect(metadata).toHaveCount(1);
          await expect(metadata).not.toHaveAttribute("open");
          await expect(metadata.locator("pre")).not.toBeVisible();
          assert.equal(
            await page
              .locator(".markdown-body")
              .textContent()
              .then((t) => t.includes("title:")),
            false,
          );
          await metadata.locator("summary").focus();
          await page.keyboard.press("Enter");
          await expect(metadata.locator("pre")).toBeVisible();
          await expect(metadata.locator("pre")).toContainText("title:");
          assert.equal(await metadata.locator("img").count(), 0);
          if (key === "general") {
            await expect(page.locator(".markdown-body h1")).toHaveText(
              "Article",
            );
            await expect(page.locator(".markdown-body hr")).toHaveCount(1);
          }
          assert.ok(
            await page.evaluate(
              () => document.documentElement.scrollWidth <= innerWidth,
            ),
          );
          if (
            process.env.FRONTMATTER_SCREENSHOT &&
            key === "general" &&
            width === 390 &&
            dark
          )
            await page.screenshot({ path: process.env.FRONTMATTER_SCREENSHOT });
          await page.getByRole("button", { name: "Toggle source" }).click();
          await expect
            .poll(() => page.locator("code").allTextContents())
            .toEqual(content.split(/\r\n|\r|\n/));
          await page.getByRole("button", { name: "Toggle source" }).click();
          await expect(metadata).not.toHaveAttribute("open");
          await metadata.locator("summary").click();
        }
        for (const content of [
          "# Plain\n\n---\n\nBody",
          "---\ntitle: [broken\n---\n# Body",
          "---\ntitle: Unclosed\n# Body",
        ]) {
          await page.evaluate(
            (content) => window.showMarkdown(content, content),
            content,
          );
          await expect(page.locator("details")).toHaveCount(0);
          await expect(page.locator(".markdown-body")).toContainText("Body");
        }
        assert.deepEqual(errors, []);
        await page.close();
      }
  } finally {
    await browser.close();
  }
});
