// Real ProjectUtilities / RightSidebar / UtilitySidebar / EnvironmentPanel; no live LVK backend.
import assert from 'node:assert/strict';
import { build } from 'esbuild';
import { chromium, expect } from '@playwright/test';
import { createServer } from 'node:http';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
const root = process.cwd(),
  scratch = await mkdtemp(path.join(tmpdir(), 'lvk-pet-integration-')),
  output = path.resolve('test-results/host-pet');
await mkdir(output, { recursive: true });
let browser, server;
const errors = [],
  requests = [],
  cases = [];
try {
  const alias = {
    '@/shared/dialogs/settings/SettingsDialog': path.join(
      root,
      'tests/pets/settings-dialog.fixture.ts'
    ),
    'virtual:executor-schemas': path.join(
      root,
      'tests/pets/integration.schemas.fixture.ts'
    ),
    '@': path.join(root, 'packages/web-core/src'),
  };
  for (const module of [
    'shared/hooks/useTerminal',
    'shared/hooks/useLogsPanel',
    'features/wiki/ui/WorkspaceWikiProvider',
  ])
    alias['@/' + module] = path.join(
      root,
      'tests/pets/integration.hooks.fixture.ts'
    );
  const bundle = await build({
    entryPoints: ['tests/host-pet/browser.fixture.tsx'],
    bundle: true,
    write: false,
    outdir: scratch,
    format: 'iife',
    jsx: 'automatic',
    define: { 'import.meta.env.DEV': 'false', 'import.meta.hot': 'undefined' },
    alias,
    nodePaths: [path.join(root, 'packages/web-core/node_modules')],
    loader: {
      '.woff2': 'dataurl',
      '.woff': 'dataurl',
      '.ttf': 'dataurl',
      '.svg': 'dataurl',
    },
  });
  execFileSync(
    path.join(root, 'packages/local-web/node_modules/.bin/tailwindcss'),
    [
      '-c',
      'tailwind.new.config.js',
      '--content',
      './src/**/*.{ts,tsx},../web-core/src/**/*.{ts,tsx},../ui/src/**/*.{ts,tsx},../../tests/host-pet/*.tsx',
      '-i',
      '../web-core/src/app/styles/new/index.css',
      '-o',
      path.join(scratch, 'style.css'),
    ],
    { cwd: path.join(root, 'packages/local-web'), stdio: 'pipe' }
  );
  const css = await readFile(path.join(scratch, 'style.css')),
    atlas = await readFile('packages/public/pets/lvk-rabbit/spritesheet.webp');
  server = createServer((req, res) => {
    if (
      req.url === '/api/attachments/11111111-1111-4111-8111-111111111111/file'
    ) {
      res.setHeader('Content-Type', 'image/webp');
      res.end(atlas);
      return;
    }
    if (req.url.startsWith('/api/')) {
      requests.push(req.url);
      res.setHeader('Content-Type', 'application/json');
      req.resume();
      const data =
        req.url.includes('/files/tree') || req.url.includes('/utilities/tree')
          ? {
              repos: ['one', 'two'].map((id) => ({
                workspace_id: 'fixture-project',
                repo_id: 'repo-' + id,
                repo_name: id,
                repo_display_name: 'File repo ' + id,
                truncated: false,
                entries: [
                  {
                    workspace_id: 'fixture-project',
                    repo_id: 'repo-' + id,
                    repo_name: id,
                    path: id + '.txt',
                    name: id + '.txt',
                    kind: 'file',
                    size_bytes: 1,
                    modified_at: null,
                    mime_type: 'text/plain',
                    is_binary: false,
                  },
                ],
              })),
            }
          : req.url === '/api/attachments/upload'
            ? { id: '11111111-1111-4111-8111-111111111111' }
            : req.url === '/api/environment-info'
              ? {
                  hostname: 'isolated-fixture-host',
                  container: true,
                  running_sessions: 0,
                  observed_at: new Date().toISOString(),
                }
              : {};
      res.end(JSON.stringify({ success: true, data }));
      return;
    }
    const [mime, body] =
      req.url === '/app.js'
        ? [
            'text/javascript',
            bundle.outputFiles.find((f) => f.path.endsWith('.js')).text,
          ]
        : req.url === '/style.css'
          ? ['text/css', css]
          : req.url === '/pets/lvk-rabbit/spritesheet.webp'
            ? ['image/webp', atlas]
            : [
                'text/html',
                '<!doctype html><html><head><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/style.css"></head><body><div id="root"></div><script src="/app.js"></script></body></html>',
              ];
    res.setHeader('Content-Type', mime);
    res.end(body);
  });
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const base = `http://127.0.0.1:${server.address().port}`;
  browser = await chromium.launch({
    headless: true,
    executablePath: process.env.LVK_E2E_CHROMIUM,
  });
  const page = await browser.newPage({
    viewport: { width: 1000, height: 800 },
  });
  page.on('pageerror', (e) => {
    errors.push(e.message);
    console.error('BROWSER', e.message);
  });

  let stored = null,
    mode = 'supported';
  const saves = [];
  await page.route('**/api/**', async (route) => {
    const url = new URL(route.request().url());
    const success = (data) => route.fulfill({ json: { success: true, data } });
    if (url.pathname === '/api/environment-info')
      return success({
        hostname: '0a67c618ae0c',
        host_display_name: mode === 'legacy' ? undefined : stored,
        container: true,
        uptime_seconds: null,
        running_sessions: 2,
        observed_at: new Date(Date.now() - 1000).toISOString(),
      });
    if (url.pathname === '/api/host-display-name') {
      if (mode === 'legacy')
        return route.fulfill({ status: 404, json: { success: false } });
      if (route.request().method() === 'PUT') {
        saves.push(route.request().postDataJSON());
        if (mode === 'failure')
          return route.fulfill({ status: 500, json: { success: false } });
        if (mode === 'drop-field') return success({});
        stored = route.request().postDataJSON().host_display_name;
      }
      return success({ host_display_name: stored });
    }
    if (url.pathname === '/api/provider-usage/read')
      return success({ result: { status: 'unsupported' } });
    if (url.pathname === '/api/resource-observation')
      return success({
        timestamp: new Date(Date.now() - 1000).toISOString(),
        system_visible: {},
        disks: [],
      });
    return success({});
  });
  const baseURL = base;
  const title = page.getByTestId('environment-hostname');
  const input = page.locator('#host-display-name');
  for (const lang of ['ja', 'en']) {
    const saveName = lang === 'ja' ? '保存' : 'Save';
    const openName =
      lang === 'ja' ? 'ホストとペットを編集' : 'Edit host and pet';
    for (const width of [360, 1280]) {
      await page.setViewportSize({ width, height: 900 });
      await page.goto(`${baseURL}/?lang=${lang}`);
      await expect(input).toBeEnabled();
      await expect(title).toHaveText(stored || '0a67c618ae0c');
      await expect(
        page.getByTestId('sidebar-pet').locator('summary')
      ).toHaveCount(0);
      await title.click();
      let first = await page.evaluate(
        () => document.documentElement.dataset.openedSettings
      );
      await page.getByRole('button', { name: openName, exact: true }).click();
      assert.equal(
        await page.evaluate(
          () => document.documentElement.dataset.openedSettings
        ),
        first
      );
      const long =
        lang === 'ja' ? 'うさぎ'.repeat(26) : 'Rabbit '.repeat(11).trim();
      await input.fill(long);
      await page.getByRole('button', { name: saveName, exact: true }).click();
      await expect(title).toHaveText(long);
      await page.reload();
      await expect(input).toHaveValue(long);
      await expect(title).toHaveText(long);
      await page
        .getByTestId('host-pet-settings')
        .getByRole('checkbox')
        .uncheck();
      await expect(
        page.getByTestId('sidebar-pet').locator('[data-pet-state]')
      ).toHaveCount(0);
      await page.getByTestId('host-pet-settings').getByRole('checkbox').check();
      await expect(
        page
          .getByTestId('sidebar-pet')
          .getByRole('button', { name: 'LVK Rabbit', exact: true })
      ).toBeVisible();
      await page.evaluate(
        (dark) => document.documentElement.classList.toggle('dark', dark),
        width === 360
      );
      await page.waitForTimeout(300);
      assert.equal(
        await page.evaluate(
          () => document.documentElement.scrollWidth > innerWidth
        ),
        false
      );
      requests.push({
        lang,
        width,
        title: await title.evaluate((el) => ({
          color: getComputedStyle(el).color,
          disabled: el.hasAttribute('disabled'),
          text: el.textContent,
        })),
      });
      await page.screenshot({
        path: path.join(output, `${lang}-${width}.png`),
        fullPage: true,
      });
      await input.fill('');
      await page.getByRole('button', { name: saveName, exact: true }).click();
      await expect(title).toHaveText('0a67c618ae0c');
    }
  }
  cases.push(
    'English/Japanese, 360/1280px, long names, title/pencil same entry, save/reload, empty fallback and existing pet hide/show'
  );
  await input.fill('x'.repeat(81));
  await expect(
    page.getByRole('button', { name: 'Save', exact: true })
  ).toBeDisabled();
  await input.fill('Unsaved pet');
  mode = 'failure';
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Could not save');
  await expect(title).toHaveText('0a67c618ae0c');
  await page.reload();
  await expect(input).toHaveValue('');
  mode = 'drop-field';
  await input.fill('Not supported');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Could not save');
  await expect(title).toHaveText('0a67c618ae0c');
  cases.push(
    'overlong value disabled, failure and successful-but-missing-field response never claim save success'
  );
  mode = 'legacy';
  await page.reload();
  await expect(input).toBeDisabled();
  await expect(page.getByRole('alert')).toContainText('unsupported');
  await expect(title).toHaveText('0a67c618ae0c');
  await page.getByTestId('host-pet-settings').getByRole('checkbox').uncheck();
  await expect(
    page.getByTestId('sidebar-pet').locator('[data-pet-state]')
  ).toHaveCount(0);
  await page.screenshot({
    path: path.join(output, 'legacy-backend.png'),
    fullPage: true,
  });
  cases.push(
    'old backend keeps hostname and pet settings; unsupported name editing disabled'
  );
  assert.deepEqual(errors, []);
  await writeFile(
    path.join(output, 'results.json'),
    JSON.stringify(
      { status: 'passed', cases, errors, saves, requests },
      null,
      2
    )
  );
  console.log(JSON.stringify({ status: 'passed', cases, errors }, null, 2));
} finally {
  await browser?.close();
  if (server) await new Promise((r) => server.close(r));
  await rm(scratch, { recursive: true, force: true });
}
