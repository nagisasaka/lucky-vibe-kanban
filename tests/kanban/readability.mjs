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
  define: { 'import.meta.env.VITE_KANBAN_MOTION': JSON.stringify(process.env.VITE_KANBAN_MOTION || 'crisp') },
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
  for (const status of ['pending', 'starting']) {
    const preparing=page.locator(`#preparing-${status}`);
    await expect(preparing).toContainText('Preparing…');
    await expect(preparing.getByRole('img', {name:'Dev server running', exact:true})).toBeVisible();
    await expect(preparing.getByRole('img', {name:'Running', exact:true})).toHaveCount(0);
    await expect(preparing).not.toContainText(/1m|65s/);
  }
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
  await expect(a.locator('[aria-live]')).toHaveCount(0);
  await expect(preview).toBeVisible();
  await page.getByText('Toggle workspaces', { exact: true }).click();
  await expect(a.getByText('Workspace 0', { exact: true })).toBeVisible();
  for (const width of [240, 260, 340]) {
    await a.evaluate((e, width) => (e.style.width = `${width}px`), width);
    const id = a.locator('[title="LOCAL-VERY-LONG-PROJECT-ID-123456789"]');
    const priority = a.getByRole('button', { name: 'Priority', exact: true });
    const assignee = a.getByRole('button', { name: 'Assignee', exact: true });
    const [idBox, priorityBox, assigneeBox, cardBox] = await Promise.all([id,priority,assignee,a].map((e) => e.boundingBox()));
    assert(idBox.x + idBox.width <= priorityBox.x + 1);
    assert(assigneeBox.x + assigneeBox.width <= cardBox.x + cardBox.width);
    assert(Math.abs(idBox.y - priorityBox.y) < 8);
    await priority.focus(); await page.keyboard.press('Enter');
    await expect(page.locator('output')).toHaveText('Priority');
    await assignee.focus(); await page.keyboard.press('Enter');
    await expect(page.locator('output')).toHaveText('Assignees');
  }
  await a.evaluate((e) => (e.style.width = ''));
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
    await expect(running.getByRole('img', { name: 'Running', exact: true })).toBeVisible();
    await expect(a.getByRole('button', {name: /Workspace 1/}).getByRole('img', {name: 'Approval required', exact: true})).toBeVisible();
    const failed = a.getByRole('button', { name: /Workspace 2/ });
    for (const name of ['Failed', 'Unseen activity', 'Dev server running']) {
      await expect(failed.getByRole('img', { name, exact: true })).toBeVisible();
    }
    await expect(a.getByRole('button', { name: /Workspace 3/ }).getByRole('img', { name: 'Read', exact: true })).toBeVisible();
    const input = card('b').getByRole('button', { name: /Workspace 4/ });
    for (const name of ['Input required', 'Dev server running']) {
      await expect(input.getByRole('img', { name, exact: true })).toBeVisible();
    }
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
  await expect(a.locator('[aria-live]')).toHaveCount(0);
  await page.getByText('Toggle workspaces', { exact: true }).click();
  await expect(a.locator('[aria-live]')).toHaveCount(0);
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
  const ghost = page.locator('[data-kanban-ghost]');
  await expect(ghost).toBeVisible();
  const ghostBounds = await ghost.boundingBox();
  assert(ghostBounds.x > to.x - 50, 'ghost follows pointer into destination');
  await page.screenshot({ path: `${output}/cross-column-ghost.png` });
  await page.mouse.up();
  await expect(ghost).toHaveCount(0);
  await expect(
    page.locator('[data-kanban-column="3"] [data-kanban-card="c"]')
  ).toHaveCount(1);
  // Escape cancels without moving the source card.
  await card('e').focus();
  await page.keyboard.press('Space', { delay: 100 });
  await expect(card('e')).toHaveAttribute('aria-pressed', 'true');
  await expect(page.locator('[data-kanban-ghost]')).toBeVisible();
  await page.keyboard.press('Escape', { delay: 100 });
  await expect(page.locator('[data-kanban-ghost]')).toHaveCount(0);
  await expect(page.locator('[data-kanban-column="2"] [data-kanban-card="e"]')).toHaveCount(1);
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
