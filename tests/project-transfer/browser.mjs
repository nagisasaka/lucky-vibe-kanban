// Real transfer panel/API client; synthetic HTTP responses only. No server or live DB.
import assert from 'node:assert/strict';
import { build } from 'esbuild';
import { chromium, expect } from '@playwright/test';
import { createRequire } from 'node:module';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
const root = process.cwd();
const output = `${root}/.build/project-transfer-browser`;
await mkdir(output, { recursive: true });
const require = createRequire(`${root}/packages/web-core/package.json`);
const bundle = await build({
  entryPoints: ['tests/project-transfer/browser.fixture.tsx'],
  bundle: true,
  write: false,
  format: 'iife',
  jsx: 'automatic',
  alias: { '@': `${root}/packages/web-core/src` },
  nodePaths: [`${root}/packages/web-core/node_modules`],
  loader: { '.css': 'empty', '.woff2': 'empty', '.woff': 'empty' },
  define: { 'import.meta.env': '{}' },
  plugins: [
    {
      name: 'fixture-api',
      setup(b) {
        b.onResolve(
          {
            filter:
              /^@\/shared\/(?:integrations\/electric\/hooks|lib\/electric\/collections)$/,
          },
          (args) =>
            args.importer.includes('/ProjectTransferPanel.')
              ? { path: `${root}/tests/project-transfer/api.fixture.ts` }
              : undefined
        );
        b.onResolve({ filter: /^(?:@\/shared\/lib\/api|\.\/api)$/ }, (args) =>
          args.importer.includes('/projectTransferApi.') ||
          args.importer.includes('/ProjectTransferPanel.')
            ? { path: `${root}/tests/project-transfer/api.fixture.ts` }
            : undefined
        );
      },
    },
  ],
});
const config = require('tailwindcss/loadConfig')(
  `${root}/packages/local-web/tailwind.new.config.js`
);
const css = await require('postcss')([
  require('tailwindcss')({
    ...config,
    content: [`${root}/packages/{ui,web-core}/src/**/*.{ts,tsx}`],
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
  executablePath:
    process.env.CHROMIUM_PATH ||
    '/opt/lvk-browsers/chromium-1217/chrome-linux64/chrome',
});
const requests = [];
const errors = [];
const missing = [{ name: 'missing-sample.txt', size_bytes: 42 }];
const report = {
  project_id: 'new-project',
  coverage: 'Project-owned fixture',
  missing_attachments: missing,
  external_reference_count: 1,
  history_omissions: 'Legacy logs omitted explicitly',
};
const text =
  '完成会話 [旧URL](https://old.invalid/private) ![添付](/api/attachments/old-id) /old-root/file <img src="https://old.invalid/secret">';
let delayed;
try {
  const page = await browser.newPage();
  page.on('pageerror', (e) => errors.push(e.message));
  await page.route('**/*', async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    if (url.pathname === '/')
      return route.fulfill({
        contentType: 'text/html',
        body: '<div id="root"></div>',
      });
    const body = request.postDataBuffer();
    requests.push({ path: url.pathname, body: body?.toString('base64') });
    let data;
    if (url.pathname.endsWith('/report')) data = report;
    else if (url.pathname === '/api/repos')
      data = [{ id: 'destination-repo', name: 'Prepared destination repo' }];
    else if (url.pathname.endsWith('/export'))
      return route.fulfill({
        contentType: 'application/vnd.sqlite3',
        body: Buffer.from('fixture download'),
      });
    else if (url.pathname.endsWith('/preview')) {
      if (body?.toString() === 'delay')
        await new Promise((resolve) => (delayed = resolve));
      data = {
        ...report,
        project_name: body.toString(),
        sha256: createHash('sha256').update(body).digest('hex'),
        issue_count: 1,
        version_count: 2,
        comment_count: 1,
        history_count: 1,
        manifest_count: 1,
        repositories: [
          { source_id: 'source-repo', display_name: 'Source repository' },
        ],
      };
    } else if (url.pathname.endsWith('/import')) data = report;
    else if (url.pathname.endsWith('/history'))
      data = [
        {
          issue_id: 'new-issue-id',
          workspace_id: 'new-workspace',
          session_id: 'new-session',
          run_id: 'new-run',
          status: 'succeeded',
          messages: [{ role: 'assistant', content: text }],
          manifests: [
            {
              id: 'new-manifest',
              repo_id: 'destination-repo',
              created_at: '2026-11-16T00:00:00Z',
              base_commit: 'a'.repeat(40),
              source_commit: 'b'.repeat(40),
              target_branch: 'fixture/main',
              changed_paths: [
                'packages/web-core/src/synthetic/' +
                  'long-relative-path-'.repeat(12) +
                  'fixture.tsx',
                'literal/[old-url](https://old.invalid/private).txt',
              ],
              semantics: {
                summary: 'Confirmed semantic summary',
                decisions: ['Synthetic decision'],
              },
            },
          ],
        },
      ];
    else throw Error(`Unexpected request: ${request.url()}`);
    return route.fulfill({
      contentType: 'application/json',
      body: JSON.stringify({ success: true, data }),
    });
  });
  await page.goto('https://fixture.invalid/');
  await page.addStyleTag({ content: css.css });
  await page.addScriptTag({ content: bundle.outputFiles[0].text });
  await expect(page.getByRole('checkbox')).not.toBeChecked();
  await expect(page.getByRole('status')).toContainText('missing-sample.txt');
  const ja = JSON.parse(
    await readFile('packages/web-core/src/i18n/locales/ja/common.json', 'utf8')
  ).projectTransfer;
  const button = (key) =>
    page
      .getByRole('button', { name: ja[key], exact: true })
      .and(page.locator('button:not([aria-expanded])'));
  await button('export').click();
  await expect
    .poll(() => requests.filter((r) => r.path.endsWith('/export')).length)
    .toBe(1);
  assert.deepEqual(
    JSON.parse(
      Buffer.from(
        requests.find((r) => r.path.endsWith('/export')).body,
        'base64'
      )
    ),
    { include_history: false }
  );
  await page.getByRole('checkbox').check();
  await button('export').click();
  await expect
    .poll(() => requests.filter((r) => r.path.endsWith('/export')).length)
    .toBe(2);
  assert.equal(
    JSON.parse(
      Buffer.from(
        requests.filter((r) => r.path.endsWith('/export'))[1].body,
        'base64'
      )
    ).include_history,
    true
  );
  const select = async (value) => {
    await page.locator('input[type=file]').setInputFiles({
      name: 'fixture.sqlite',
      mimeType: 'application/vnd.sqlite3',
      buffer: Buffer.from(value),
    });
    await expect(button('preview')).toBeEnabled();
  };
  await select('SQLite fixture A');
  await button('preview').click();
  await expect(
    page.getByText('SQLite fixture A', { exact: true })
  ).toBeVisible();
  await expect(button('import')).toBeDisabled();
  await page.screenshot({
    path: `${output}/preview-desktop.png`,
    fullPage: true,
  });
  await page.getByLabel(ja.key, { exact: true }).fill('UNIQUE');
  await page.getByLabel('Source repository').selectOption('destination-repo');
  await button('import').click();
  await expect(page.getByRole('status')).toContainText('missing-sample.txt');
  await page.screenshot({
    path: `${output}/import-report-desktop.png`,
    fullPage: true,
  });
  await expect(
    page.getByText(ja.importedRefresh, { exact: true })
  ).toBeVisible();
  await expect(
    page.getByRole('link', { name: ja.imported, exact: true })
  ).toHaveCount(0);
  await expect(button('import')).toHaveCount(0);
  await expect(button('preview')).toBeDisabled();
  await page.evaluate(() =>
    window.dispatchEvent(new Event('fixture-projects-visible'))
  );
  await expect(
    page.getByRole('link', { name: ja.imported, exact: true })
  ).toHaveAttribute('href', '/projects/new-project');
  const imported = JSON.parse(
    Buffer.from(requests.find((r) => r.path.endsWith('/import')).body, 'base64')
  );
  assert.equal(
    imported.data_base64,
    Buffer.from('SQLite fixture A').toString('base64')
  );
  assert.equal(
    imported.expected_sha256,
    createHash('sha256').update('SQLite fixture A').digest('hex')
  );
  assert.equal(imported.key, 'UNIQUE');
  assert.deepEqual(imported.repo_map, { 'source-repo': 'destination-repo' });
  await select('SQLite fixture B');
  await button('preview').click();
  await expect(
    page.getByText('SQLite fixture B', { exact: true })
  ).toBeVisible();
  await select('SQLite fixture C');
  await expect(page.getByText('SQLite fixture B', { exact: true })).toHaveCount(
    0
  );
  await expect(button('import')).toHaveCount(0);
  // Force a selection event while preview is pending, even though normal input is disabled.
  await select('delay');
  await button('preview').click();
  await expect.poll(() => Boolean(delayed)).toBe(true);
  await page.locator('input[type=file]').evaluate((input) => {
    input.disabled = false;
    const data = new DataTransfer();
    data.items.add(new File(['new selection'], 'changed.sqlite'));
    input.files = data.files;
    input.dispatchEvent(new Event('change', { bubbles: true }));
  });
  delayed();
  await expect(button('preview')).toBeEnabled();
  await expect(page.getByText('delay', { exact: true })).toHaveCount(0);
  await button('history').click();
  await page.getByText('new-run (succeeded)', { exact: true }).click();
  await expect(page.locator('pre').first()).toHaveText(text);
  await expect(page.locator('pre a, pre img, pre iframe')).toHaveCount(0);
  await expect(
    page.getByRole('link', { name: ja.issue, exact: true })
  ).toHaveAttribute(
    'href',
    '/projects/destination-project/issues/new-issue-id'
  );
  await page.getByText(`${ja.manifest} new-manifest`, { exact: true }).click();
  await expect(
    page.getByText('Confirmed semantic summary', { exact: true })
  ).toBeVisible();
  for (const value of [
    '2026-11-16T00:00:00Z',
    'a'.repeat(40),
    'b'.repeat(40),
    'fixture/main',
  ])
    await expect(page.locator('dd').filter({ hasText: value })).toBeVisible();
  await expect(
    page.locator('dd').filter({ hasText: 'long-relative-path-' })
  ).toContainText('literal/[old-url](https://old.invalid/private).txt');
  await expect(page.locator('dd a, dd img')).toHaveCount(0);
  // Imported Issue descriptions/comments share this actual editor wrapper.
  await expect(page.locator('#imported-issue pre')).toContainText(
    '[old link](https://old.invalid/private)'
  );
  await expect(
    page.locator('#imported-issue a, #imported-issue img')
  ).toHaveCount(0);
  await page.locator('#imported-issue button').click();
  await expect(page.locator('#imported-issue textarea')).toHaveValue(
    '[old link](https://old.invalid/private) ![attachment](/api/attachments/old-id) <img src="https://old.invalid/secret">'
  );
  await page
    .locator('#imported-issue textarea')
    .fill('Edited literal [old link](https://old.invalid/private)');
  for (const dark of [false, true])
    for (const width of [390, 1440]) {
      await page.evaluate(
        (d) => document.documentElement.classList.toggle('dark', d),
        dark
      );
      await page.setViewportSize({ width, height: 1200 });
      assert.ok(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth + 1
        )
      );
      await page.screenshot({
        path: `${output}/${dark ? 'dark' : 'light'}-${width}.png`,
        fullPage: true,
      });
    }
  const requestBoundary = requests.length;
  await page.goto('https://fixture.invalid/?import-only');
  await page.addStyleTag({ content: css.css });
  await page.addScriptTag({ content: bundle.outputFiles[0].text });
  await expect(page.getByRole('checkbox')).toHaveCount(0);
  await expect(button('export')).toHaveCount(0);
  await expect(button('history')).toHaveCount(0);
  await select('Empty destination fixture');
  await button('preview').click();
  await page.getByLabel(ja.key, { exact: true }).fill('EMPTYDEST');
  await page.getByLabel('Source repository').selectOption('destination-repo');
  await button('import').click();
  await expect(
    page.getByText(ja.importedRefresh, { exact: true })
  ).toBeVisible();
  await expect(button('import')).toHaveCount(0);
  await expect(button('preview')).toBeDisabled();
  await expect(page.getByRole('status')).toContainText('missing-sample.txt');
  assert.ok(
    requests
      .slice(requestBoundary)
      .every(
        (r) =>
          !r.path.endsWith('/report') &&
          !r.path.endsWith('/export') &&
          !r.path.endsWith('/history')
      )
  );
  assert.deepEqual(errors, []);
  assert.ok(requests.every((r) => !r.path.includes('attachments')));
  await writeFile(
    `${output}/results.json`,
    JSON.stringify(
      {
        checks:
          'opt-in, identical bytes/hash, explicit key/repo, selection reset/stale response, persisted report, archive/Manifest inert text, remapped structured Issue URL, responsive light/dark',
        requests: requests.map((r) => r.path),
        errors,
      },
      null,
      2
    )
  );
  console.log(`Project transfer browser fixture passed: ${output}`);
} finally {
  await browser.close();
}
