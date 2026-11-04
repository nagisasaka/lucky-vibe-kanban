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
  output = path.resolve('test-results/pets-integration');
await mkdir(output, { recursive: true });
let browser, server;
const errors = [],
  requests = [],
  cases = [];
try {
  const alias = {
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
    entryPoints: ['tests/pets/integration.fixture.tsx'],
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
  await page.route('**/*', (r) =>
    new URL(r.request().url()).origin === base ? r.continue() : r.abort()
  );
  await page.goto(base);
  const panel = page.getByTestId('environment-panel'),
    pet = panel.getByRole('button', { name: 'LVK Rabbit', exact: true });
  await expect(pet).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Files', exact: true })
  ).toBeVisible();
  for (const count of [0, 1, 2, 0, 2]) {
    await page.getByRole('button', { name: 'Repositories ' + count }).click();
    await expect(page.getByTestId('main-terminal-repository')).toHaveCount(0);
    await expect(page.getByTestId('terminal-working-directory')).toHaveCount(0);
    await expect(
      page.getByText('Terminal repository', { exact: true })
    ).toHaveCount(0);
    const terminal = page.getByRole('button', {
      name: 'Terminal',
      exact: true,
    });
    await page.evaluate(() => {
      window.terminalStarts = [];
    });
    if (count === 0) await expect(terminal).toBeDisabled();
    else {
      await expect(terminal).toBeEnabled();
      expect(await page.evaluate(() => window.terminalStarts)).toEqual([]);
      await terminal.click();
      expect(await page.evaluate(() => window.terminalStarts)).toEqual([
        { scope: 'main:fixture-project', cwd: '/isolated/first' },
      ]);
    }
  }
  await page.getByRole('button', { name: 'Repositories 0' }).click();
  const files = page.getByRole('button', { name: 'Files', exact: true });
  await files.click();
  for (const id of ['one', 'two']) {
    await expect(
      page.getByText('File repo ' + id, { exact: true })
    ).toBeVisible();
    await page.getByRole('button', { name: id + '.txt', exact: true }).click();
    await expect(page).toHaveTitle('Selected repo-' + id);
  }
  await files.click();
  cases.push(
    'File tree retains both repository groups and file selection from each'
  );
  await page.screenshot({
    path: path.join(output, 'terminal-menu-removed.png'),
  });
  cases.push(
    'Files title only; terminal selector/cwd menu absent; 0/1/2 repos and async empty/refill use first root only on click'
  );
  await expect(panel).toContainText('isolated-fixture-host');
  await expect(
    page.getByRole('button', { name: 'Close utilities' })
  ).toHaveCount(0);
  cases.push(
    'Kanban real ProjectUtilities contains exactly one environment pet and host panel'
  );
  await page.getByRole('button', { name: 'Toggle sidebar' }).click();
  await expect(panel).toHaveCount(0);
  await page.getByRole('button', { name: 'Toggle sidebar' }).click();
  await expect(pet).toBeVisible();
  cases.push(
    'Kanban has no Close utilities button; product visibility store still unmounts sidebar'
  );
  await page.getByRole('button', { name: 'Workspace screen' }).click();
  await expect(pet).toBeVisible();
  await expect(panel).toContainText('isolated-fixture-host');
  await expect(
    page.getByRole('button', { name: 'Git', exact: true })
  ).toBeVisible();
  cases.push(
    'Workspace real RightSidebar contains exactly one environment pet and Git section'
  );
  assert.equal(await page.getByTestId('sidebar-pet').count(), 1);
  const zip = path.join(scratch, 'shared.zip');
  execFileSync('python3', [
    '-c',
    `import zipfile,sys,json\nfrom pathlib import Path\np=Path(sys.argv[1]);m=json.loads((p/'pet.json').read_text());m['displayName']='Shared imported rabbit'\nwith zipfile.ZipFile(sys.argv[2],'w',zipfile.ZIP_DEFLATED) as z:\n z.writestr('pet.json',json.dumps(m));z.write(p/'spritesheet.webp','spritesheet.webp')`,
    'packages/public/pets/lvk-rabbit',
    zip,
  ]);
  await page.getByTestId('sidebar-pet').locator('summary').first().click();
  await page
    .getByTestId('sidebar-pet')
    .locator('input[type=file]')
    .setInputFiles(zip);
  await expect(
    panel.getByRole('button', { name: 'Shared imported rabbit' })
  ).toBeVisible();
  await page.getByRole('button', { name: 'Kanban screen' }).click();
  await expect(
    panel.getByRole('button', { name: 'Shared imported rabbit' })
  ).toBeVisible();
  await page.reload();
  await expect(
    panel.getByRole('button', { name: 'Shared imported rabbit' })
  ).toBeVisible();
  cases.push(
    'real v1 import through product attachment API client and shared selection survives screen switch and reload'
  );
  await page.getByTestId('sidebar-pet').locator('summary').first().click();
  await page.getByTestId('sidebar-pet').getByRole('checkbox').uncheck();
  await expect(panel.locator('[data-pet-state]')).toHaveCount(0);
  await page.getByRole('button', { name: 'Workspace screen' }).click();
  await expect(panel.locator('[data-pet-state]')).toHaveCount(0);
  await page.getByTestId('sidebar-pet').locator('summary').first().click();
  await page
    .getByTestId('sidebar-pet')
    .getByRole('button', { name: 'Restore LVK Rabbit' })
    .click();
  await expect(pet).toBeVisible();
  await page.getByRole('button', { name: 'Workspace screen' }).click();
  await expect(pet).toBeVisible();
  cases.push(
    'real product preferences and registered writer share hide/reset between two screen sidebars'
  );
  for (const screen of ['Kanban', 'Workspace']) {
    await page.getByRole('button', { name: screen + ' screen' }).click();
    await page.getByRole('button', { name: 'Toggle sidebar' }).click();
    await expect(panel).toHaveCount(0);
    await expect(page.getByTestId('sidebar-pet')).toHaveCount(0);
    await page.getByRole('button', { name: 'Toggle sidebar' }).click();
    await expect(pet).toBeVisible();
  }
  cases.push(
    'both screen sidebar unmounts remove pet and remount preserves it'
  );
  for (const width of [320, 375, 768, 1000])
    for (const screen of ['Kanban', 'Workspace']) {
      await page.setViewportSize({ width, height: 800 });
      await page.getByRole('button', { name: screen + ' screen' }).click();
      await expect(pet).toBeVisible();
      const bounds = await pet.boundingBox();
      expect(bounds.width).toBe(80);
      expect(bounds.height).toBe(80);
      const hostname = await page.getByTestId('environment-hostname').boundingBox();
      assert.ok(bounds.x + bounds.width <= hostname.x, 'pet must not overlap hostname');
      const metrics = await page.getByTestId('environment-metrics').boundingBox();
      assert.ok(bounds.y + bounds.height <= metrics.y, 'pet must not overlap metrics');
      const geometry = await panel.evaluate((el) => ({
        right: el.getBoundingClientRect().right,
        width: document.documentElement.clientWidth,
        scroll: document.documentElement.scrollWidth,
      }));
      assert.ok(
        geometry.right <= width && geometry.scroll <= width,
        JSON.stringify({ screen, ...geometry })
      );
      await page.screenshot({
        path: path.join(output, `${screen}-${width}.png`),
      });
      if (width === 320 || width === 375) {
        await page
          .getByTestId('sidebar-pet')
          .locator('summary')
          .first()
          .click();
        const settings = page
          .getByTestId('sidebar-pet')
          .locator('details > div')
          .first();
        await expect(settings).toBeVisible();
        const bounds = await settings.evaluate((el) => ({
          left: el.getBoundingClientRect().left,
          right: el.getBoundingClientRect().right,
          scroll: document.documentElement.scrollWidth,
        }));
        assert.ok(
          bounds.left >= 0 && bounds.right <= width && bounds.scroll <= width,
          JSON.stringify({ screen, width, ...bounds })
        );
        await page.screenshot({
          path: path.join(output, `${screen}-${width}-settings.png`),
        });
        await page
          .getByTestId('sidebar-pet')
          .locator('summary')
          .first()
          .click();
      }
    }
  cases.push(
    'both actual sidebar compositions retain pet/host content without horizontal overflow at 320/375/768/1000px; settings popovers stay in viewport at 320/375px'
  );
  assert.deepEqual(errors, []);
  await writeFile(
    path.join(output, 'results.json'),
    JSON.stringify({ status: 'passed', cases, errors, requests }, null, 2)
  );
  console.log(
    JSON.stringify({ status: 'passed', cases, errors, requests }, null, 2)
  );
} finally {
  await browser?.close();
  if (server) await new Promise((r) => server.close(r));
  await rm(scratch, { recursive: true, force: true });
}
