import { test } from 'node:test';
import assert from 'node:assert/strict';
import { build } from 'esbuild';
import { chromium, expect } from '@playwright/test';
import { fileURLToPath } from 'node:url';

test('retired Card Context is ordinary visible, editable body with no controls', async () => {
  const root = fileURLToPath(new URL('../../', import.meta.url));
  const bundle = await build({
    entryPoints: [root + 'tests/chat/prompt-context.fixture.tsx'],
    loader: { '.css': 'empty', '.woff2': 'empty', '.woff': 'empty' },
    define: { 'import.meta.env': '{}' },
    bundle: true,
    write: false,
    format: 'iife',
    jsx: 'automatic',
    alias: { '@': root + 'packages/web-core/src', shared: root + 'shared' },
    nodePaths: [root + 'packages/web-core/node_modules'],
  });
  const browser = await chromium.launch({
    headless: true,
    args: ['--no-sandbox'],
  });
  try {
    const page = await browser.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(e.message));
    await page.setContent('<body><div id="root"></div></body>');
    await page.addScriptTag({ content: bundle.outputFiles[0].text });
    await page.waitForFunction(() => !!window.setContent);
    const initial = await page.evaluate(() => window.initial);
    const message = page.locator('#message');
    await expect(message.locator('details')).toHaveCount(0);
    assert.equal(await message.locator('pre').textContent(), initial);
    await message
      .getByRole('button', { name: 'メッセージを編集', exact: true })
      .click();
    assert.equal(await page.evaluate(() => window.edited), initial);
    await expect(page.locator('#issue [contenteditable=false]')).toContainText(
      '独自の要求'
    );
    await page.locator('#issue [contenteditable=false]').click();
    const description = page.locator('#issue [contenteditable=true]');
    await expect(description).toContainText('独自の要求');
    await expect(description).toContainText('保存済み独自段階');
    await expect(page.locator('#issue').getByRole('checkbox')).toHaveCount(0);
    await description.click();
    await description.press('Control+End');
    await page.keyboard.insertText(' 追記');
    await expect
      .poll(() => page.evaluate(() => window.description))
      .toContain('追記');
    const saved = await page.evaluate(() => window.description);
    assert.ok(saved.includes('独自の要求'));
    assert.ok(saved.includes('保存済み独自段階'));
    await description.fill('普通の本文');
    await expect
      .poll(() => page.evaluate(() => window.description))
      .toBe('普通の本文');
    for (const content of [
      '<!-- lvk:card-context:start -->\n不完全',
      '<!-- lvk:shared-directories:start -->\n編集済み案内\n<!-- lvk:shared-directories:end -->',
    ]) {
      await page.evaluate((text) => window.setContent(text), content);
      await expect(message.locator('details')).toHaveCount(0);
      assert.equal(await message.locator('pre').textContent(), content);
    }
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});
