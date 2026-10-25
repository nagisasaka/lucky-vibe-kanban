// Isolated rendering of production components. No backend/API coverage is claimed.
import { build } from 'esbuild';
import { chromium, expect } from '@playwright/test';
import { createRequire } from 'node:module';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
const root = process.cwd();
const output = `${root}/.build/kanban-readability`;
await mkdir(output, { recursive: true });
const require = createRequire(`${root}/packages/web-core/package.json`);
const bundle = await build({
  entryPoints: ['tests/kanban/readability.fixture.tsx'],
  bundle: true,
  write: false,
  format: 'iife',
  jsx: 'automatic',
  alias: { '@': `${root}/packages/web-core/src` },
  nodePaths: [`${root}/packages/web-core/node_modules`],
  loader: { '.css': 'empty' },
});
const config = require('tailwindcss/loadConfig')(
  `${root}/packages/local-web/tailwind.new.config.js`
);
const css = await require('postcss')([
  require('tailwindcss')({
    ...config,
    content: [
      `${root}/packages/ui/src/components/*.tsx`,
      `${root}/packages/web-core/src/features/kanban/ui/*.tsx`,
      `${root}/tests/kanban/readability.fixture.tsx`,
    ],
  }),
]).process(
  (
    await readFile('packages/web-core/src/app/styles/new/index.css', 'utf8')
  ).replace(/^@(import|config).*$/gm, '') +
    (
      await readFile(
        'packages/web-core/src/features/kanban/ui/kanban-grid.css',
        'utf8'
      )
    ).replace(/^@config.*$/gm, ''),
  { from: undefined }
);
const browser = await chromium.launch({
  headless: true,
  args: ['--no-sandbox'],
});
try {
  const page = await browser.newPage({
    viewport: { width: 2560, height: 1440 },
    deviceScaleFactor: 1.5,
  });
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.route('https://fixture.invalid/**', (r) =>
    r.fulfill({
      contentType: 'text/html',
      body: '<html><body><div id="root"></div></body></html>',
    })
  );
  await page.goto('https://fixture.invalid/');
  await page.addStyleTag({ content: css.css });
  await page.addScriptTag({ content: bundle.outputFiles[0].text });
  const card = (id) => page.locator(`[data-kanban-card="${id}"]`);
  const a = card('a');
  const preview = a.getByTestId('issue-description-preview');
  await expect(preview).toBeVisible();
  await expect(preview).toContainText('Specification Readable link');
  await expect(card('c').getByTestId('issue-description-preview')).toHaveCount(
    0
  );
  await expect(a.getByText('Workspace 0', { exact: true })).toBeVisible();
  await expect(a.getByText('0 files', { exact: true })).toBeVisible();
  await expect(a.getByText('+0', { exact: true })).toBeVisible();
  await expect(a.getByText('— files', { exact: true })).toBeVisible();
  await expect(a.getByText('32s', { exact: true })).toBeVisible();
  await expect(a.getByText('just now', { exact: true })).toHaveCount(0);
  await a.locator('summary').click();
  await expect(preview).toBeVisible();
  await expect(a.locator('details p')).toContainText('let value = 1');
  await a.locator('summary').click();
  await expect(a.getByText('Workspace 0', { exact: true })).toBeVisible();
  await a.getByRole('button', { name: /Workspace 0/ }).focus();
  await page.keyboard.press('Enter');
  await expect(page.locator('output')).toHaveText('Workspace 0');
  await a.locator('summary').focus();
  await page.evaluate(() =>
    [...document.querySelectorAll('button')]
      .find((b) => b.textContent === 'Summary update')
      .click()
  );
  await expect(a.locator('summary')).toBeFocused();
  await page.waitForTimeout(1100);
  await expect(a.getByText('32s', { exact: true })).toBeVisible();
  const measurements = [];
  for (const width of [2560, 1280, 390, 2560]) {
    await page.setViewportSize({ width, height: 1440 });
    const values = await page
      .locator('[data-kanban-column]')
      .evaluateAll((cols) =>
        cols.map((c) => ({
          width: c.clientWidth,
          columns: getComputedStyle(c).gridTemplateColumns.split(' ').length,
        }))
      );
    for (const v of values) assert.equal(v.columns, v.width >= 536 ? 2 : 1);
    const p = await preview.evaluate((e) => ({
      height: e.clientHeight,
      line: parseFloat(getComputedStyle(e).lineHeight),
      scroll: e.scrollWidth,
      width: e.clientWidth,
    }));
    assert.ok(p.height <= p.line * 2 + 1);
    assert.ok(p.scroll <= p.width + 1);
    const overflows = await a
      .locator('[role="button"]')
      .evaluateAll((els) =>
        els
          .filter((e) => e.scrollWidth > e.clientWidth + 1)
          .map((e) => e.textContent)
      );
    assert.deepEqual(overflows, []);
    measurements.push({ viewport: width, columns: values, preview: p });
    await page.screenshot({ path: `${output}/width-${width}.png` });
  }
  for (const width of [535, 536, 537, 536, 535]) {
    await page
      .locator('.kanban-grid-column')
      .evaluateAll(
        (els, w) => els.forEach((e) => (e.style.width = `${w}px`)),
        width
      );
    const counts = await page
      .locator('[data-kanban-column]')
      .evaluateAll((els) =>
        els.map(
          (e) => getComputedStyle(e).gridTemplateColumns.split(' ').length
        )
      );
    assert.ok(counts.every((n) => n === (width >= 536 ? 2 : 1)));
    measurements.push({ columnWidth: width, counts });
  }
  await page
    .locator('.kanban-grid-column')
    .evaluateAll((els) => els.forEach((e) => (e.style.width = '')));
  await page.getByText('Toggle workspaces', { exact: true }).click();
  await expect(a.getByText('Workspace 0', { exact: true })).toHaveCount(0);
  await expect(a.getByText('Approval required', { exact: true })).toBeVisible();
  await expect(preview).toBeVisible();
  await page.getByText('Toggle workspaces', { exact: true }).click();
  await expect(a.getByText('Workspace 0', { exact: true })).toBeVisible();
  // Added LOCAL81 scope: tag geometry, keyboard editor, DevServer/Agent coexistence.
  const tagRow = a.getByTestId('kanban-tags');
  const tagTrigger = a.locator('button:has([data-testid="kanban-tags"])');
  await tagTrigger.focus();
  await page.keyboard.press('Enter');
  await expect(page.getByText('Tag editor', { exact: true })).toBeVisible();
  await page.waitForTimeout(150);
  await page.keyboard.press('Escape');
  await expect(tagTrigger).toBeFocused();
  for (const dark of [false, true]) {
    await page.evaluate(
      (d) => document.documentElement.classList.toggle('dark', d),
      dark
    );
    for (const width of [220, 300, 600, 220]) {
      await a.evaluate((e, w) => (e.style.width = `${w}px`), width);
      await page.waitForTimeout(80);
      const geometry = await tagRow.evaluate((e) => {
        const r = e.getBoundingClientRect();
        return [...e.children]
          .filter((n) => n.getAttribute('aria-hidden') !== 'true')
          .map((n) => ({
            right: n.getBoundingClientRect().right,
            limit: r.right,
            text: n.textContent,
          }));
      });
      assert.ok(
        geometry.every((g) => g.right <= g.limit + 1),
        JSON.stringify(geometry)
      );
      assert.ok(geometry.some((g) => g.text === '+2'));
    }
    const running = a.getByRole('button', { name: /Workspace 0/ });
    await expect(
      running.getByRole('img', { name: 'Dev server running' })
    ).toBeVisible();
    const colors = await Promise.all(
      [0, 3].map((n) =>
        a
          .getByRole('button', { name: new RegExp(`Workspace ${n}`) })
          .evaluate((e) => getComputedStyle(e).backgroundColor)
      )
    );
    assert.notEqual(colors[0], colors[1]);
    await page.screenshot({
      path: `${output}/tags-devserver-${dark ? 'dark' : 'light'}.png`,
    });
  }
  await a.evaluate((e) => (e.style.width = '600px'));
  await page.getByText('Toggle long tags', { exact: true }).click();
  await expect(
    tagRow.locator(':scope > span').filter({ hasText: /^\+2$/ })
  ).toHaveCount(0);
  for (let n = 0; n < 5; n++) {
    await page.getByText('Change tags', { exact: true }).click();
    await page.waitForTimeout(50);
    if ((await tagRow.count()) === 0) {
      await expect(a.locator('button[aria-haspopup="dialog"]')).toBeVisible();
      continue;
    }
    assert.ok(
      await tagRow.evaluate(
        (e) =>
          e.getBoundingClientRect().right <=
          e.closest('button').getBoundingClientRect().right + 1
      )
    );
  }
  await expect(a.locator('[aria-live]')).not.toContainText(
    'Dev server running'
  );
  await page.getByText('Toggle workspaces', { exact: true }).click();
  await expect(a.locator('[aria-live]')).toContainText('Dev server running');
  await expect(a.locator('[aria-live]')).toContainText('Approval required');
  await page.getByText('Toggle workspaces', { exact: true }).click();
  await a.evaluate((e) => (e.style.width = ''));
  // Existing keyboard DnD crosses columns; pointer drop reaches an empty column.
  await card('d').focus();
  await page.keyboard.press('Space', { delay: 100 });
  await page.keyboard.press('ArrowRight', { delay: 100 });
  await page.keyboard.press('Space', { delay: 100 });
  await expect(
    page.locator('[data-kanban-column="2"] [data-kanban-card="d"]')
  ).toHaveCount(1);
  await expect(card('d')).toBeFocused();
  const from = await card('c').boundingBox();
  const to = await page.locator('[data-kanban-column="3"]').boundingBox();
  await page.mouse.move(from.x + 12, from.y + 12);
  await page.mouse.down();
  await page.mouse.move(from.x + 24, from.y + 12, { steps: 4 });
  await page.mouse.move(to.x + 40, to.y + 50, { steps: 20 });
  await page.mouse.up();
  await expect(
    page.locator('[data-kanban-column="3"] [data-kanban-card="c"]')
  ).toHaveCount(1);
  assert.deepEqual(errors, []);
  await writeFile(
    `${output}/results.json`,
    JSON.stringify(
      { measurements, errors, scope: 'component fixtures; no backend/API' },
      null,
      2
    )
  );
  console.log('PASS isolated readability component browser checks');
} finally {
  await browser.close();
  console.log('CLEANUP browser closed');
}
