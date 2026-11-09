import assert from 'node:assert/strict';
import { build } from 'esbuild';
import { chromium, expect } from '@playwright/test';
import { createRequire } from 'node:module';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
const root = process.cwd();
const output = `${root}/.build/markdown-reading`;
await mkdir(output, { recursive: true });
const require = createRequire(`${root}/packages/web-core/package.json`);
const baseline = process.env.MARKDOWN_BASELINE;
const bundle = await build({
  entryPoints: ['tests/markdown/reading.fixture.tsx'],
  bundle: true,
  write: false,
  format: 'iife',
  jsx: 'automatic',
  alias: { '@': `${root}/packages/web-core/src` },
  nodePaths: [`${root}/packages/web-core/node_modules`],
  loader: { '.css': 'empty', '.woff2': 'empty', '.woff': 'empty' },
  define: { 'import.meta.env': '{}' },
  plugins: baseline
    ? [
        {
          name: 'pinned-baseline',
          setup(build) {
            build.onLoad({ filter: /\.(tsx|ts)$/ }, (args) => {
              const relative = args.path.slice(root.length + 1);
              if (!relative.startsWith('packages/')) return;
              try {
                return {
                  contents: execFileSync(
                    'git',
                    ['show', `${baseline}:${relative}`],
                    { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }
                  ),
                  loader: args.path.endsWith('tsx') ? 'tsx' : 'ts',
                };
              } catch {
                return;
              }
            });
          },
        },
      ]
    : [],
});
const config = require('tailwindcss/loadConfig')(
  `${root}/packages/local-web/tailwind.new.config.js`
);
const rawCss = baseline
  ? execFileSync(
      'git',
      ['show', `${baseline}:packages/web-core/src/app/styles/new/index.css`],
      { encoding: 'utf8' }
    )
  : await readFile('packages/web-core/src/app/styles/new/index.css', 'utf8');
const css = await require('postcss')([
  require('tailwindcss')({
    ...config,
    content: [
      `${root}/packages/{ui,web-core}/src/**/*.{tsx,ts}`,
      `${root}/tests/markdown/*.tsx`,
    ],
  }),
]).process(rawCss.replace(/^@(import|config).*$/gm, ''), { from: undefined });
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
  await expect(page.locator('#lexical h1')).toHaveCount(1);
  if (!baseline) {
    await expect(
      page.locator('#lexical [data-markdown-alert="decision"]')
    ).toHaveCount(1);
    await expect(
      page.locator('#preview [data-markdown-alert="decision"]')
    ).toHaveCount(1);
    assert.deepEqual(await page.evaluate(() => window.changes), []);
  }
  for (const dark of [false, true]) {
    await page.evaluate((d) => {
      document.documentElement.classList.toggle('dark', d);
      window.setTheme(d ? 'dark' : 'light');
    }, dark);
    for (const width of [780, 1600]) {
      await page.setViewportSize({ width, height: 1000 });
      await page.locator('#lexical').evaluate((e) => (e.scrollTop = 0));
      await page.locator('#preview').evaluate((e) => (e.scrollTop = 0));
      if (!baseline) {
        for (const id of ['lexical', 'preview']) {
          assert.equal(
            await page.locator(`#${id} [data-markdown-alert]`).count(),
            6
          );
          const normal = page
            .locator(`#${id} blockquote`)
            .filter({ hasText: '人間の意図' });
          assert.equal(await normal.getAttribute('data-markdown-alert'), null);
          const geometry = await page
            .locator(`#${id} .markdown-body`)
            .evaluate((e) => ({
              font: getComputedStyle(e).fontSize,
              line: getComputedStyle(e).lineHeight,
              width: e.clientWidth,
              scroll: e.scrollWidth,
              quote: getComputedStyle(
                e.querySelector('[data-markdown-alert="decision"]')
              ).borderLeftColor,
              warning: getComputedStyle(
                e.querySelector('[data-markdown-alert="warning"]')
              ).borderLeftColor,
            }));
          assert.equal(geometry.font, '17px');
          assert.ok(
            geometry.scroll <= geometry.width + 1,
            JSON.stringify(geometry)
          );
          assert.notEqual(geometry.quote, geometry.warning);
          measurements.push({ dark, width, id, ...geometry });
        }
      }
      if (width === 780) {
        await page.locator('#issue').screenshot({
          path: `${output}/${baseline ? 'before' : 'after'}-issue-${dark ? 'dark' : 'light'}.png`,
        });
        await page.evaluate(() => window.scrollTo(0, 0));
      }
      await page.screenshot({
        path: `${output}/${baseline ? 'before' : 'after'}-${width}-${dark ? 'dark' : 'light'}.png`,
      });
    }
  }
  if (!baseline) {
    await page.evaluate(() =>
      Object.defineProperty(navigator, 'clipboard', {
        configurable: true,
        value: {
          writeText: async (value) => {
            window.copiedMarkdown = value;
          },
        },
      })
    );
    await page
      .locator('#lexical')
      .getByRole('button', { name: 'Markdownとしてコピー', exact: true })
      .click();
    assert.equal(
      await page.evaluate(() => window.copiedMarkdown),
      await page.evaluate(() => window.source)
    );
    // Duplicate Japanese headings target distinct nodes; only the selected pane moves.
    await page.locator('#lexical nav > button').click();
    await page
      .locator('#lexical nav li button')
      .filter({ hasText: '日本語の見出し' })
      .nth(1)
      .click();
    assert.ok(await page.locator('#lexical').evaluate((e) => e.scrollTop > 0));
    assert.equal(
      await page.locator('#preview').evaluate((e) => e.scrollTop),
      0
    );
    assert.equal(await page.evaluate(() => location.hash), '');
    assert.equal(
      await page.evaluate(() => document.activeElement.textContent),
      '日本語の見出し'
    );
    assert.deepEqual(await page.evaluate(() => window.changes), []);
    await page.evaluate(() => window.switchDocument());
    await expect(page.locator('#lexical nav > button')).toHaveAttribute(
      'aria-expanded',
      'false'
    );
    // TOC doesn't enter Issue editing or cause an autosave.
    await page.locator('#issue nav > button').click();
    await page.locator('#issue nav li button').last().click();
    await expect(page.locator('#issue [contenteditable]')).toHaveAttribute(
      'contenteditable',
      'false'
    );
    assert.equal(await page.evaluate(() => window.issueChanges), undefined);
    // Existing safe links and file action retain their behavior.
    await page.locator('#lexical').evaluate((e) => (e.scrollTop = 0));
    assert.equal(
      await page
        .locator('#lexical a')
        .filter({ hasText: '安全なリンク' })
        .getAttribute('rel'),
      'noopener noreferrer'
    );
    assert.equal(
      await page
        .locator('#lexical a')
        .filter({ hasText: '危険なリンク' })
        .getAttribute('href'),
      null
    );
    await page.locator('#lexical .clickable-code').last().click();
    assert.equal(
      await page.evaluate(() => window.fileOpened),
      'src/example.ts'
    );
    // Switching to editing restores the literal marker and removes display decorations.
    await page.evaluate(() => window.setEditing(true));
    await expect(page.locator('#lexical nav')).toHaveCount(0);
    await expect(page.locator('#lexical [data-markdown-alert]')).toHaveCount(0);
    await expect(page.locator('#lexical [contenteditable]')).toContainText(
      '[!DECISION]'
    );
    await page.locator('#lexical [contenteditable]').click();
    await page.keyboard.press('Control+End');
    await page.keyboard.type(' EDIT');
    await expect
      .poll(() => page.evaluate(() => window.currentValue))
      .toContain('EDIT');
    assert.ok(
      (await page.evaluate(() => window.currentValue)).includes('> [!DECISION]')
    );
    await page.evaluate(() => window.setEditing(false));
    await expect(
      page.locator('#lexical [data-markdown-alert="decision"]')
    ).toHaveCount(1);
    // Fewer headings removes the outline and obsolete references.
    await page.evaluate(() =>
      window.changeBody('## 短い本文\n\n> [!DECISION]\n> 判断してください')
    );
    await expect(page.locator('#lexical nav')).toHaveCount(0);
    await expect(page.locator('#preview nav')).toHaveCount(0);
    assert.equal(
      await page
        .locator('#composer [contenteditable]')
        .evaluate((e) => getComputedStyle(e).fontSize),
      '16px'
    );
    // Existing attachment control still uses local metadata and opens its scoped URL.
    await page.evaluate(() => {
      window.open = (url, target, features) => {
        window.openedAttachment = { url, target, features };
        return null;
      };
      window.changeBody(window.sourceAttachment);
    });
    await page
      .locator('#lexical [role="button"]')
      .filter({ hasText: 'sample.txt' })
      .click();
    assert.deepEqual(await page.evaluate(() => window.openedAttachment), {
      url: 'https://fixture.invalid/sample.txt',
      target: '_blank',
      features: 'noopener,noreferrer',
    });
    // Existing sanitized details stays interactive; its closed heading isn't in the TOC.
    await page.evaluate(() =>
      window.changeBody(
        '## 表示1\n\n## 表示2\n\n<details><summary>補足</summary>\n\n## 補足見出し\n\n</details>'
      )
    );
    await expect(page.locator('#preview nav')).toHaveCount(0);
    await page.locator('#preview summary').click();
    await expect(page.locator('#preview nav > button')).toHaveCount(1);
    await page.locator('#preview nav > button').focus();
    await page.keyboard.press('Enter');
    await expect(page.locator('#preview nav li')).toHaveCount(3);
    assert.equal(await page.evaluate(() => location.hash), '');
    // Unknown labels and markers in prose/code are ordinary text, not inferred alerts.
    await page.evaluate(() =>
      window.changeBody(
        '> [!UNKNOWN]\n> ordinary\n\n\x60[!DECISION]\x60\n\n> [!DECISION] inline prose'
      )
    );
    await expect(page.locator('#lexical [data-markdown-alert]')).toHaveCount(0);
    await expect(page.locator('#preview [data-markdown-alert]')).toHaveCount(0);
  }
  assert.deepEqual(errors, []);
  await writeFile(
    `${output}/${baseline ? 'before' : 'after'}-measurements.json`,
    JSON.stringify(measurements, null, 2)
  );
  console.log(
    `Markdown ${baseline ? 'baseline capture' : 'regressions'} passed; evidence: ${output}`
  );
} finally {
  await browser.close();
}
