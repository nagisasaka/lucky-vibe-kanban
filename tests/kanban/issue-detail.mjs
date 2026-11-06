// Isolated production UI and editor rendering. Does not start a backend or agents.
import assert from 'node:assert/strict';
import { build } from 'esbuild';
import { chromium, expect } from '@playwright/test';
import { createRequire } from 'node:module';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
const root = process.cwd();
const output = `${root}/.build/issue-layout`;
await mkdir(output, { recursive: true });
const require = createRequire(`${root}/packages/web-core/package.json`);
const bundle = await build({
  entryPoints: ['tests/kanban/issue-detail.fixture.tsx'],
  bundle: true,
  write: false,
  format: 'iife',
  jsx: 'automatic',
  alias: { '@': `${root}/packages/web-core/src` },
  nodePaths: [`${root}/packages/web-core/node_modules`],
  loader: { '.css': 'empty', '.woff2': 'empty', '.woff': 'empty' },
  define: { 'import.meta.env': '{}' },
});
const config = require('tailwindcss/loadConfig')(
  `${root}/packages/local-web/tailwind.new.config.js`
);
const css = await require('postcss')([
  require('tailwindcss')({
    ...config,
    content: [
      `${root}/packages/{ui,web-core}/src/**/*.{tsx,ts}`,
      `${root}/tests/kanban/issue-detail.fixture.tsx`,
    ],
  }),
]).process(
  (
    await readFile('packages/web-core/src/app/styles/new/index.css', 'utf8')
  ).replace(/^@(import|config).*$/gm, ''),
  { from: undefined }
);
const browser = await chromium.launch({
  headless: true,
  args: ['--no-sandbox'],
});
const measurements = [];
try {
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.route('https://fixture.invalid/**', (r) =>
    r.fulfill({ contentType: 'text/html', body: '<div id="root"></div>' })
  );
  await page.goto('https://fixture.invalid');
  await page.addStyleTag({ content: css.css });
  await page.addScriptTag({ content: bundle.outputFiles[0].text });
  await expect(page.locator('#issue article')).toHaveCount(3);
  for (const dark of [false, true]) {
    await page.evaluate(
      (d) => document.documentElement.classList.toggle('dark', d),
      dark
    );
    for (const pane of [320, 390, 720, 960, 1200]) {
      await page.setViewportSize({ width: pane * 2, height: 1100 });
      const geometry = await page.evaluate(() => {
        const rect = (selector) => {
          const e = document.querySelector(selector);
          const b = e.getBoundingClientRect();
          return {
            x: b.x,
            width: b.width,
            client: e.clientWidth,
            scroll: e.scrollWidth,
          };
        };
        const title = document.querySelector('#issue textarea');
        const reading = document.querySelector('.issue-detail-content');
        const table = document.querySelector('#issue table');
        return {
          issue: rect('#issue'),
          reading: rect('.issue-detail-content'),
          conversation: rect('#conversation'),
          composer: rect('#composer > div'),
          message: rect('#conversation > div'),
          workspace: rect('#workspace'),
          title: getComputedStyle(title).fontSize,
          gutter: parseFloat(getComputedStyle(reading).paddingLeft),
          table: { client: table.clientWidth, scroll: table.scrollWidth },
          page: {
            width: document.documentElement.clientWidth,
            scroll: document.documentElement.scrollWidth,
          },
        };
      });
      assert.equal(geometry.title, '24px');
      assert.ok(geometry.gutter >= 16 && geometry.gutter <= 32);
      assert.ok(geometry.reading.width <= 1024);
      assert.ok(Math.abs(geometry.conversation.x - geometry.composer.x) < 1);
      assert.ok(
        Math.abs(geometry.conversation.width - geometry.composer.width) < 1
      );
      assert.ok(geometry.message.width > geometry.conversation.width - 2);
      assert.ok(geometry.conversation.width <= 1024);
      assert.ok(geometry.page.scroll <= geometry.page.width + 1);
      if (pane < 600) assert.ok(geometry.table.scroll > geometry.table.client);
      measurements.push({ pane, dark, ...geometry });
      const scroll = page.locator('#issue > div > div').nth(1);
      await scroll.evaluate((e) => (e.scrollTop = 0));
      await page.screenshot({
        path: `${output}/after-${pane}-${dark ? 'dark' : 'light'}.png`,
      });
      await page
        .getByText('タスクの試行', { exact: true })
        .scrollIntoViewIfNeeded();
      await page.screenshot({
        path: `${output}/choices-${pane}-${dark ? 'dark' : 'light'}.png`,
      });
      for (const [name, action] of [
        ['新しいワークフロー試行', 'workflow'],
        ['新しい単一エージェント試行', 'single'],
        ['アリーナを開始', 'arena'],
      ]) {
        const button = page.getByRole('button', {
          name: new RegExp(`^${name}`),
        });
        await button.focus();
        await page.keyboard.press('Enter');
        assert.equal(await page.evaluate(() => window.lastAction), action);
      }
      await page.locator('#issue article').first().scrollIntoViewIfNeeded();
      await page.screenshot({
        path: `${output}/comments-${pane}-${dark ? 'dark' : 'light'}.png`,
      });
      await page
        .getByRole('button', { name: '返信', exact: true })
        .first()
        .click();
      assert.equal(await page.evaluate(() => window.lastAction), 'reply');
    }
  }
  // Scope checks: the same components retain their standalone/create defaults.
  await page.setViewportSize({ width: 2400, height: 1100 });
  await page
    .locator('#workspace')
    .evaluate((e) => e.classList.remove('kanban-workspace-pane'));
  assert.equal(
    Math.round((await page.locator('#composer > div').boundingBox()).width),
    768
  );
  assert.equal(
    await page
      .locator('#conversation > div')
      .evaluate((e) => getComputedStyle(e).maxWidth),
    '800px'
  );
  await page.evaluate(() => window.setCreateMode(true));
  await expect(page.locator('.issue-detail-content')).toHaveCount(0);
  assert.equal(
    await page
      .locator('#issue textarea')
      .first()
      .evaluate((e) => getComputedStyle(e).fontSize),
    '18px'
  );
  await page.screenshot({ path: `${output}/create-and-standalone.png` });
  assert.deepEqual(errors, []);
  await writeFile(
    `${output}/results.json`,
    JSON.stringify(
      {
        scope:
          'production components/editors with fixture data; no authenticated application, backend or live providers',
        measurements,
        errors,
      },
      null,
      2
    )
  );
  console.log(
    'PASS: 10 pane/theme combinations; horizontal containment, three method keyboard actions, comment reply, standalone chat and creation scope'
  );
} finally {
  await browser.close();
}
