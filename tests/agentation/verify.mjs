// Uses synthetic page routing and generated UI only; no live app data or credentials.
import assert from 'node:assert/strict';
import { build as bundle } from 'esbuild';
import { build as viteBuild } from '../../packages/local-web/node_modules/vite/dist/node/index.js';
import { chromium, expect } from '@playwright/test';
import { mkdir, writeFile, readFile } from 'node:fs/promises';
import path from 'node:path';
const root = process.cwd();
const output = path.resolve('test-results/agentation');
await mkdir(output, { recursive: true });
const results = { cases: [], errors: [], unexpectedRequests: [] };
let browser;
try {
  // Use Vite's real production constant replacement and Rollup graph, without a full distribution build.
  const production = await viteBuild({
    configFile: false,
    root: path.join(root, 'packages/local-web'),
    mode: 'production',
    logLevel: 'silent',
    esbuild: { jsx: 'automatic' },
    build: {
      write: false,
      minify: false,
      lib: {
        entry: path.join(
          root,
          'packages/local-web/src/app/entry/DevAgentation.tsx'
        ),
        formats: ['es'],
      },
      rollupOptions: { external: ['react', 'react/jsx-runtime'] },
    },
  });
  const chunks = (Array.isArray(production) ? production : [production])
    .flatMap((result) => result.output)
    .filter((item) => item.type === 'chunk');
  const modules = chunks.flatMap((item) => Object.keys(item.modules));
  assert.ok(modules.some((id) => id.endsWith('DevAgentation.tsx')));
  assert.ok(
    !modules.some(
      (id) =>
        id.includes('node_modules/agentation') || id.includes('/agentation@')
    )
  );
  assert.ok(
    !chunks.some(
      (chunk) =>
        chunk.code.includes('import("agentation")') ||
        chunk.dynamicImports.length
    )
  );
  results.production = {
    modules,
    chunks: chunks.length,
    bytes: chunks.reduce((n, c) => n + c.code.length, 0),
  };
  results.cases.push(
    'Vite production graph excludes agentation package and dynamic imports'
  );
  const manifest = JSON.parse(
    await readFile(
      'packages/local-web/node_modules/agentation/package.json',
      'utf8'
    )
  );
  assert.equal(manifest.version, '3.1.2');
  assert.equal(manifest.license, 'PolyForm-Shield-1.0.0');
  assert.equal(manifest.dependencies, undefined);
  const js = await bundle({
    entryPoints: ['tests/agentation/browser.fixture.tsx'],
    bundle: true,
    write: false,
    format: 'iife',
    jsx: 'automatic',
    define: { 'import.meta.env.DEV': 'true' },
    nodePaths: [path.join(root, 'packages/local-web/node_modules')],
  });
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    viewport: { width: 1280, height: 800 },
    permissions: ['clipboard-read', 'clipboard-write'],
  });
  await context.route('**/*', async (route) => {
    const url = new URL(route.request().url());
    if (url.pathname === '/fixture.js')
      return route.fulfill({
        contentType: 'text/javascript',
        body: js.outputFiles[0].text,
      });
    if (url.pathname === '/isolated-agentation')
      return route.fulfill({
        contentType: 'text/html',
        body: '<!doctype html><title>Isolated Agentation</title><div id="root"></div><script src="/fixture.js"></script>',
      });
    results.unexpectedRequests.push({ origin: url.origin, path: url.pathname });
    await route.abort();
  });
  const page = await context.newPage();
  page.on('pageerror', (error) => results.errors.push(error.message));
  await page.goto('https://preview2.luckyvibekanban.com/isolated-agentation');
  await expect(page.locator('[data-agentation-toolbar]')).toBeVisible();
  await page.screenshot({ path: path.join(output, 'toolbar.png') });
  results.cases.push('preview2 development toolbar renders');
  // Subsequent assertions exercise the real library UI, not an annotation mock.
  await page
    .getByRole('button', {
      name: 'Start feedback mode',
    })
    .click();
  await page
    .getByRole('button', { name: 'Quota fixture', exact: true })
    .click();
  await page
    .getByPlaceholder('What should change?', { exact: true })
    .fill('Make the remaining quota easier to read');
  await page.getByRole('button', { name: 'Add', exact: true }).click();
  await page.getByRole('button', { name: /Copy/i }).click();
  const markdown = await page.evaluate(() => navigator.clipboard.readText());
  assert.ok(markdown.includes('Make the remaining quota easier to read'));
  assert.ok(markdown.includes('quota-fixture') || markdown.includes('button'));
  await writeFile(path.join(output, 'copied.md'), markdown);
  await page.screenshot({ path: path.join(output, 'annotated.png') });
  results.cases.push(
    'element selection, annotation and actual Markdown clipboard export'
  );
  await page.waitForTimeout(11000);
  assert.deepEqual(results.unexpectedRequests, []);
  results.cases.push(
    'no endpoint, webhook, telemetry or other network requests during local workflow and health interval'
  );
  await page.goto('https://other-preview.invalid/isolated-agentation');
  await expect(
    page.getByRole('button', { name: 'Quota fixture', exact: true })
  ).toBeVisible();
  await expect(page.locator('[data-agentation-toolbar]')).toHaveCount(0);
  results.cases.push('other development hosts do not render the toolbar');
  assert.deepEqual(results.errors, []);
  results.status = 'passed';
} catch (error) {
  results.status = 'failed';
  results.error = error.stack;
  throw error;
} finally {
  await browser?.close();
  await writeFile(
    path.join(output, 'results.json'),
    JSON.stringify(results, null, 2)
  );
  console.log(JSON.stringify(results, null, 2));
}
