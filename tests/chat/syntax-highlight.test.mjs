import { test } from 'node:test';
import assert from 'node:assert/strict';
import { build } from 'esbuild';
import { chromium } from '@playwright/test';
import { fileURLToPath } from 'node:url';

test('chat syntax preserves streamed, unknown, plain and long text without moving focus', async () => {
  const result = await build({
    entryPoints: [
      fileURLToPath(new URL('./syntax-highlight.fixture.tsx', import.meta.url)),
    ],
    bundle: true,
    write: false,
    format: 'iife',
    jsx: 'automatic',
    nodePaths: [
      fileURLToPath(
        new URL('../../packages/web-core/node_modules', import.meta.url)
      ),
    ],
  });
  const browser = await chromium.launch({
    headless: true,
    args: ['--no-sandbox'],
  });
  try {
    const page = await browser.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(e.message));
    await page.setContent('<div id="root"></div>');
    await page.addScriptTag({ content: result.outputFiles[0].text });
    // Loaded history must highlight on mount, before any streamed update.
    await page.waitForFunction(() =>
      document.querySelector(
        '#markdown code[data-language="json"] [class*="syntax-"]'
      )
    );
    assert.equal(
      await page
        .locator('#markdown code[data-language="unknown-language"]')
        .textContent(),
      '  untouched  '
    );
    await page.locator('#composer').fill('入力保持');
    const code = '  echo "$HOME" && printf \'<script> & 日本語\\n\'\t\r\n';
    for (const value of [code.slice(0, 10), code, 'echo "unterminated']) {
      await page.evaluate((value) => window.setCode(value), value);
      await page.waitForFunction(
        (value) => document.querySelector('#code').textContent === value,
        value
      );
      assert.equal(
        await page
          .locator('#composer')
          .evaluate((e) => e === document.activeElement),
        true
      );
    }
    await page.evaluate((code) => window.setCode(code), code);
    await page.waitForFunction(() =>
      document.querySelector('#code span[class]')
    );
    assert.equal(await page.locator('#code script').count(), 0);
    await page.getByRole('button', { name: 'command', exact: true }).click();
    assert.equal(
      await page.locator('pre').nth(1).textContent(),
      code,
      'command details preserve leading/trailing whitespace'
    );
    for (const language of ['unknown-language', 'text', undefined]) {
      await page.evaluate((language) => window.setLanguage(language), language);
      await page.waitForFunction(() => !document.querySelector('#code span'));
      assert.equal(await page.locator('#code').textContent(), code);
    }
    const long = 'echo "$HOME"\n'.repeat(3000);
    await page.evaluate((long) => {
      window.setLanguage('bash');
      window.setCode(long);
    }, long);
    await page.waitForFunction(
      (long) => document.querySelector('#code').textContent === long,
      long
    );
    assert.equal(await page.locator('#code span').count(), 0);
    for (const [language, source, highlighted] of [
      ['bash', 'echo "$HOME"', true],
      ['sh', 'echo "hello"', true],
      ['json', '{"ok":true}', true],
      ['powershell', 'Write-Output "hello"', true],
      ['typescript', 'const count = 42;', true],
      ['unknown-language', 'const count = 42;', false],
      ['', 'const count = 42;', false],
      ['text', 'const count = 42;', false],
      ['bash', long.trimEnd(), false],
    ]) {
      for (const closing of ['', '\n```']) {
        await page.evaluate(
          (value) => window.setMarkdown(value),
          '```' + language + '\n' + source + closing
        );
        await page.waitForFunction((source) => {
          const code = document
            .querySelector('#markdown code')
            ?.cloneNode(true);
          if (!code) return false;
          code.querySelectorAll('br').forEach((br) => br.replaceWith('\n'));
          return code.textContent === source;
        }, source);
        assert.equal(
          (await page.locator('#markdown [class*="syntax-"]').count()) > 0,
          highlighted,
          language
        );
      }
    }
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});
