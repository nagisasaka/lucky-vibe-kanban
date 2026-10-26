import { test } from 'node:test';
import assert from 'node:assert/strict';
import { build } from 'esbuild';
import { chromium, expect } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';

test('canonical single command stays readable through real tool_calls aggregation without an outer expansion', async () => {
  const root = process.cwd();
  const evidence =
    process.env.LVK_EXECUTION_EVIDENCE ?? 'test-results/local32-initial-group';
  mkdirSync(evidence, { recursive: true });
  const bundle = await build({
    entryPoints: ['tests/chat/execution-group.fixture.tsx'],
    bundle: true,
    write: false,
    format: 'iife',
    jsx: 'automatic',
    alias: { '@': root + '/packages/web-core/src' },
    nodePaths: [root + '/packages/web-core/node_modules'],
    loader: { '.css': 'empty', '.woff2': 'dataurl', '.wasm': 'binary' },
    define: { 'import.meta.env': '{}' },
    logLevel: 'error',
  });
  const css = execFileSync(
    root + '/packages/local-web/node_modules/.bin/tailwindcss',
    [
      '-i',
      '../web-core/src/app/styles/new/index.css',
      '--config',
      'tailwind.new.config.js',
      '--minify',
    ],
    {
      cwd: root + '/packages/local-web',
      encoding: 'utf8',
      maxBuffer: 20 * 1024 * 1024,
    }
  ).replace(/@import\s*(?:url\([^)]*\)|"[^"]*"|'[^']*')\s*;/g, '');
  const browser = await chromium.launch({ args: ['--no-sandbox'] });
  const errors = [];
  try {
    const page = await browser.newPage({
      viewport: { width: 1180, height: 900 },
    });
    page.on('pageerror', (e) => errors.push(e.message));
    await page.setContent(`<style>${css}</style><div id="root"></div>`);
    await page.addScriptTag({ content: bundle.outputFiles[0].text });
    const group = page.locator('#actual-group');
    await expect(group).toContainText("/bin/bash -lc 'ls -la'");
    await expect(group).toContainText('README.md');
    await expect(group).not.toContainText('ツール呼び出し完了');
    await expect(group).toContainText('<1s');
    const footer = page.locator('#actual-footer');
    await expect(footer).toContainText('13s');
    await expect(footer.locator('time')).toHaveAttribute(
      'datetime',
      '2026-10-25T10:54:13Z'
    );
    await expect(footer).not.toContainText('&#');
    assert.equal(
      await page.evaluate(() => window.projectedRows[0].aggregationType),
      'tool_calls'
    );
    await page.evaluate(() =>
      Object.defineProperty(navigator, 'clipboard', {
        configurable: true,
        value: { writeText: async (text) => (window.copied = text) },
      })
    );
    await page.screenshot({ path: evidence + '/collapsed-1180.png' });
    await group.getByRole('button').first().click();
    await expect(
      group.getByRole('button', { name: 'コマンドをコピー' })
    ).toBeVisible();
    await group.getByRole('button', { name: 'コマンドをコピー' }).click();
    assert.equal(
      await page.evaluate(() => window.copied),
      "/bin/bash -lc 'ls -la'"
    );
    await page.screenshot({ path: evidence + '/details-1180.png' });
    for (const width of [1180, 375])
      for (const theme of ['light', 'dark']) {
        await page.setViewportSize({ width, height: 900 });
        await page.evaluate(
          (t) =>
            document.documentElement.classList.toggle('dark', t === 'dark'),
          theme
        );
        assert.equal(
          await page.evaluate(
            () => document.documentElement.scrollWidth <= innerWidth
          ),
          true
        );
        await page.screenshot({ path: `${evidence}/${width}-${theme}.png` });
      }
    assert.deepEqual(errors, []);
    writeFileSync(
      evidence + '/report.json',
      JSON.stringify(
        {
          head: execFileSync('git', ['rev-parse', 'HEAD'], {
            encoding: 'utf8',
          }).trim(),
          dirty: !!execFileSync('git', ['status', '--porcelain'], {
            encoding: 'utf8',
          }).trim(),
          scope:
            'canonical event -> projection -> actual aggregateConsecutiveEntries -> actual AggregatedGroupEntry; no provider or public DB access',
          checks: [
            'operation and result visible before clicking',
            'single click to raw details and exact copy',
            'retained completion timestamp and duration',
            'desktop/narrow light/dark no horizontal overflow',
          ],
          errors,
        },
        null,
        2
      )
    );
  } finally {
    await browser.close();
  }
});
