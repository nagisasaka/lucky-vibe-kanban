import { test } from "node:test";
import assert from "node:assert/strict";
import { build } from "esbuild";
import { chromium, expect } from "@playwright/test";
import { readFile, readdir, mkdir } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = fileURLToPath(new URL("../../", import.meta.url));
const assets = path.join(root, "packages/local-web/dist/assets");
const css = (
  await Promise.all(
    (await readdir(assets))
      .filter((name) => /^index-.*\.css$/.test(name))
      .map((name) => readFile(path.join(assets, name), "utf8")),
  )
).join("\n");
const bundle = await build({
  entryPoints: [path.join(root, "tests/file-viewer/syntax.fixture.tsx")],
  bundle: true,
  write: false,
  format: "iife",
  jsx: "automatic",
  alias: { "@": path.join(root, "packages/web-core/src") },
  nodePaths: [path.join(root, "packages/web-core/node_modules")],
});

test("Prism file viewer preserves safe text, multiline syntax, themes, line jumps and mobile scrolling", async () => {
  const browser = await chromium.launch({
    headless: true,
    args: ["--no-sandbox"],
  });
  const colors = [];
  try {
    await mkdir(path.join(root, ".build/file-highlighting"), {
      recursive: true,
    });
    for (const width of [1440, 390]) {
      for (const dark of [false, true]) {
        const page = await browser.newPage({
          viewport: { width, height: 844 },
        });
        const errors = [];
        page.on("pageerror", (error) => errors.push(error.message));
        await page.setContent(
          '<!doctype html><html><head></head><body style="margin:0"><div id="root"></div></body></html>',
        );
        await page.addStyleTag({ content: css });
        await page.evaluate(
          (dark) => document.documentElement.classList.toggle("dark", dark),
          dark,
        );
        await page.addScriptTag({ content: bundle.outputFiles[0].text });
        await page.waitForFunction(() => window.showViewer);
        const show = async (content, language, line) => {
          await page.evaluate(
            ({ content, language, line }) =>
              window.showViewer(content, language, line),
            { content, language, line },
          );
          await expect
            .poll(() => page.locator("code").allTextContents())
            .toEqual(content.split(/\r\n|\r|\n/));
        };
        for (const [language, content] of [
          ["typescript", "/* first\r\nsecond */\nconst value: number = 42;\n"],
          ["javascript", "const template = `first\n${1 + 2}\nlast`;"],
          ["rust", 'fn main() { let value = r#"first\nsecond"#; }'],
          ["json", '{\n  "ok": true,\n  "value": 42\n}'],
        ]) {
          await show(content, language);
          assert.ok(
            await page.locator('code [class*="syntax-"]').count(),
            language,
          );
          if (language === "typescript") {
            await expect(
              page.locator("code").nth(1).locator('[class*="syntax-comment"]'),
            ).toHaveText("second */");
            const color = await page
              .locator('code [class*="syntax-keyword"]')
              .first()
              .evaluate((el) => getComputedStyle(el).color);
            const plain = await page
              .locator("code")
              .first()
              .evaluate((el) => getComputedStyle(el).color);
            assert.notEqual(color, plain);
            colors.push({ width, dark, color });
          }
          if (language === "rust")
            assert.ok(
              await page
                .locator("code")
                .nth(1)
                .locator('[class*="syntax-string"]')
                .count(),
            );
        }
        const unsafe =
          'const html = "<img src=x onerror=window.__executed=true><script>window.__executed=true</script>";';
        await show(unsafe, "javascript");
        assert.equal(await page.locator("code img, code script").count(), 0);
        assert.equal(await page.evaluate(() => window.__executed), undefined);
        const selection = await page
          .locator("code")
          .first()
          .evaluate((el) => {
            const range = document.createRange();
            range.selectNodeContents(el);
            const selection = getSelection();
            selection.removeAllRanges();
            selection.addRange(range);
            return selection.toString();
          });
        assert.equal(
          selection,
          unsafe,
          "syntax decoration must not alter selected/copyable text",
        );
        assert.equal(
          await page.locator("[contenteditable],textarea").count(),
          0,
        );
        for (const [language, content] of [
          ["unknown-language", "  untouched\t日本語\n"],
          ["text", "<b>plain</b>"],
          ["typescript", "x".repeat(4097)],
          ["javascript", "x;\n".repeat(6000)],
          ["typescript", ""],
        ]) {
          await show(content, language);
          assert.equal(
            await page.locator('code [class*="syntax-"]').count(),
            0,
          );
        }
        const source = Array.from({ length: 160 }, (_, i) =>
          i === 119 ? "const selected: number = 120;" : `// line ${i + 1}`,
        ).join("\n");
        await show(source, "typescript", 120);
        const selected = page.locator('[aria-current="location"]');
        await expect(selected).toHaveCount(1);
        await expect(selected).toBeInViewport();
        await expect(selected.locator("code")).toHaveText(
          "const selected: number = 120;",
        );
        await expect(
          selected.locator('code [class*="syntax-keyword"]').first(),
        ).toBeVisible();
        assert.ok(
          await selected.evaluate(
            (el) => el.parentElement.parentElement.scrollTop > 0,
          ),
        );
        await page.screenshot({
          path: path.join(
            root,
            `.build/file-highlighting/${width}-${dark ? "dark" : "light"}.png`,
          ),
        });
        assert.ok(
          await page.evaluate(
            () => document.documentElement.scrollWidth <= innerWidth,
          ),
        );
        await show("x".repeat(5000), "javascript");
        const scroller = page.locator("main > div");
        assert.ok(
          await scroller.evaluate((el) => {
            el.scrollLeft = 200;
            return el.scrollLeft > 0;
          }),
        );
        assert.deepEqual(errors, []);
        await page.close();
      }
    }
    for (const width of [1440, 390]) {
      const pair = colors.filter((item) => item.width === width);
      assert.notEqual(
        pair[0].color,
        pair[1].color,
        "theme must change syntax colors",
      );
    }
  } finally {
    await browser.close();
  }
});
