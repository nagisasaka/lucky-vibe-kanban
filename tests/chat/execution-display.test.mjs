import { test } from 'node:test';
import assert from 'node:assert/strict';
import { build } from 'esbuild';
import { chromium, expect } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

test('execution previews disclose status, retain source and copy exact text across themes and widths', async () => {
  const root = fileURLToPath(new URL('../../', import.meta.url));
  const evidence = root + 'test-results/local32-execution';
  mkdirSync(evidence, { recursive: true });
  const bundle = await build({
    entryPoints: [root + 'tests/chat/execution-display.fixture.tsx'],
    bundle: true,
    write: false,
    format: 'iife',
    jsx: 'automatic',
    nodePaths: [root + 'packages/web-core/node_modules'],
  });
  const css = execFileSync(
    root + 'packages/local-web/node_modules/.bin/tailwindcss',
    [
      '-i',
      '../web-core/src/app/styles/new/index.css',
      '--config',
      'tailwind.new.config.js',
      '--minify',
    ],
    {
      cwd: root + 'packages/local-web',
      encoding: 'utf8',
      maxBuffer: 20 * 1024 * 1024,
    }
  ).replace(/@import\s*(?:url\([^)]*\)|"[^"]*"|'[^']*')\s*;/g, '');
  const browser = await chromium.launch({
    headless: true,
    args: ['--no-sandbox'],
  });
  const report = {
    head: execFileSync('git', ['rev-parse', 'HEAD'], {
      cwd: root,
      encoding: 'utf8',
    }).trim(),
    source:
      'dirty worktree; fixed component fixture, real theme CSS; no backend or provider connection',
    checks: [],
    errors: [],
  };
  try {
    const page = await browser.newPage({
      viewport: { width: 1280, height: 1000 },
    });
    page.on('pageerror', (error) => report.errors.push(error.message));
    await page.setContent(`<style>${css}</style><div id="root"></div>`);
    await page.addScriptTag({ content: bundle.outputFiles[0].text });
    await expect(page.locator('#failure')).toContainText('失敗');
    await expect(page.locator('#failure')).toContainText('Error: expected 2');
    await expect(page.locator('#failure')).toContainText('exit 1');
    await expect(page.locator('#empty')).toContainText('出力なし');
    await expect(page.locator('#cancelled')).toContainText('キャンセル');
    await expect(page.locator('#unknown')).toContainText('結果未確認');
    await expect(page.locator('#unknown')).not.toContainText('出力なし');
    await expect(page.locator('#denied')).toContainText('拒否');
    await expect(page.locator('#running')).toContainText('途中まで受信');
    await expect(page.locator('#mixed')).toContainText('失敗 1');
    await expect(page.locator('#mixed')).toContainText('Error: regression');
    assert.equal(await page.locator('#mixed .bg-success').count(), 0);
    report.checks.push(
      'failure diagnostic, exit code, empty, cancelled, denied, unknown, partial and mixed aggregate'
    );
    await page.locator('#composer').fill('入力を保持');
    await page.evaluate(() => {
      window.copied = [];
      Object.defineProperty(navigator, 'clipboard', {
        configurable: true,
        value: {
          writeText: async (value) => {
            window.copied.push(value);
          },
        },
      });
    });
    await page.locator('#failure > div > button').click();
    const original = '  printf "a\\_b\\n"\t\n  pnpm test  ';
    assert.equal(
      await page.locator('#failure pre').first().textContent(),
      original
    );
    await page
      .locator('#failure')
      .getByRole('button', { name: 'コマンドをコピー' })
      .click();
    assert.equal(await page.evaluate(() => window.copied[0]), original);
    await page
      .locator('#failure')
      .getByRole('button', { name: '出力をコピー' })
      .click();
    assert.equal(
      await page.evaluate(() => window.copied[1]),
      await page.locator('#failure pre').last().textContent()
    );
    await expect(page.locator('#failure')).toContainText('終了を確認');
    await expect(page.locator('#failure')).toContainText(
      'provider報告: 1.25秒'
    );
    await page.locator('#failure > div > button').click();
    report.checks.push(
      'expanded original command/output, exact clipboard API arguments including spaces, tab and backslash, completion metadata'
    );
    for (const width of [1280, 375]) {
      await page.setViewportSize({ width, height: 1000 });
      for (const theme of ['light', 'dark']) {
        if (theme === 'dark') await page.locator('#theme').click();
        await page.locator('#history').evaluate((el) => {
          el.scrollTop = 0;
        });
        assert.equal(
          await page.evaluate(
            () => document.documentElement.scrollWidth <= window.innerWidth
          ),
          true
        );
        await page.screenshot({ path: `${evidence}/${width}-${theme}.png` });
        if (theme === 'dark') await page.locator('#theme').click();
      }
    }
    const long = '  ' + '日本語\\_literal '.repeat(3000) + '\t\n';
    await page.locator('#composer').focus();
    await page.evaluate(
      (long) =>
        window.setCases([
          {
            id: 'stream',
            title: 'stream',
            entries: [
              {
                summary: 'echo long',
                command: 'echo long',
                expansionKey: 'stream',
                status: { status: 'created' },
                active: true,
                content: long,
                outputState: 'partial',
              },
            ],
          },
        ]),
      long
    );
    await expect(page.locator('#stream')).toContainText('途中まで受信');
    await expect(page.locator('#composer')).toBeFocused();
    await expect(page.locator('#composer')).toHaveValue('入力を保持');
    await page.locator('#stream > div > button').click();
    assert.equal(await page.locator('#stream pre').last().textContent(), long);
    await page
      .locator('#stream')
      .getByRole('button', { name: '出力をコピー' })
      .click();
    assert.equal(await page.evaluate(() => window.copied.at(-1)), long);
    report.checks.push(
      'desktop/narrow light/dark no horizontal page overflow; long streamed output preserves input focus/value and exact details/copy'
    );
    assert.deepEqual(report.errors, []);
  } finally {
    writeFileSync(
      evidence + '/report.json',
      JSON.stringify(report, null, 2) + '\n'
    );
    await browser.close();
  }
});
