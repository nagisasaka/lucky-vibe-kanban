// Representative creation UI checks against production components and synthetic APIs.
import { build } from 'esbuild';
import { chromium, expect } from '@playwright/test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { readFile, mkdir } from 'node:fs/promises';
const root = process.cwd();
const bundle = await build({
  entryPoints: ['tests/workspace-creation/dialog.fixture.tsx'],
  bundle: true,
  write: false,
  format: 'iife',
  jsx: 'automatic',
  alias: {
    '@/shared/lib/api': `${root}/tests/workspace-creation/api.fixture.ts`,
    '@': `${root}/packages/web-core/src`,
  },
  plugins: [
    {
      name: 'synthetic-picker',
      setup(b) {
        b.onResolve({ filter: /^\.\/ServerFolderPickerDialog$/ }, () => ({
          path: `${root}/tests/workspace-creation/picker.fixture.ts`,
        }));
      },
    },
  ],
  nodePaths: [`${root}/packages/web-core/node_modules`],
  define: { 'import.meta.env': '{}' },
  loader: { '.css': 'empty' },
});
const require = createRequire(`${root}/packages/web-core/package.json`);
const config = require('tailwindcss/loadConfig')(
  `${root}/packages/local-web/tailwind.new.config.js`
);
const css = await require('postcss')([
  require('tailwindcss')({
    ...config,
    content: [`${root}/packages/{ui,web-core}/src/**/*.{tsx,ts}`],
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
try {
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.setContent('<div id="root"></div>');
  await page.addStyleTag({ content: css.css });
  await page.addScriptTag({ content: bundle.outputFiles[0].text });
  await page.getByRole('button', { name: 'Project new', exact: true }).click();
  const modal = page.getByRole('dialog');
  await expect(modal.getByRole('checkbox')).toHaveCount(2);
  await expect(modal.getByRole('checkbox').nth(0)).toBeChecked();
  await expect(modal.getByRole('checkbox').nth(1)).toBeChecked();
  if (process.env.LVK349_EVIDENCE_DIR) {
    await mkdir(process.env.LVK349_EVIDENCE_DIR, { recursive: true });
    for (const [width, dark] of [
      [390, false],
      [1100, true],
    ]) {
      await page.setViewportSize({ width, height: 900 });
      await page.evaluate(
        (dark) => document.documentElement.classList.toggle('dark', dark),
        dark
      );
      await expect(modal).toBeVisible();
      assert.ok(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth
        )
      );
      await page.screenshot({
        path: `${process.env.LVK349_EVIDENCE_DIR}/creation-${width}-${dark ? 'dark' : 'light'}.png`,
      });
    }
  }

  await expect(modal.getByText('Repo outside')).toHaveCount(0);
  await modal.getByRole('checkbox').nth(1).uncheck();
  await modal.getByRole('checkbox').nth(0).uncheck();
  const confirm = modal.getByRole('button', {
    name: commonLabel('useWorkspace'),
  });
  await expect(confirm).toBeDisabled();
  await expect(modal.getByRole('tab').first()).toHaveAttribute(
    'aria-selected',
    'true'
  );
  await modal.getByRole('checkbox').nth(1).check();
  await expect(confirm).toBeEnabled();
  await modal.getByRole('button', { name: 'main', exact: true }).click();
  await page.getByRole('menuitem').filter({ hasText: 'develop' }).click();
  await confirm.click();
  const selected = JSON.parse(await page.locator('#result').textContent());
  assert.equal(selected.selection.repos.length, 1);
  assert.equal(selected.selection.repos[0].repo.id, 'b');
  assert.equal(selected.selection.repos[0].targetBranch, 'develop');
  await page.getByRole('button', { name: 'Orphan new', exact: true }).click();
  await expect(modal.getByRole('checkbox')).toHaveCount(3);
  await modal.getByRole('checkbox').nth(0).check();
  await modal.getByRole('checkbox').nth(2).check();
  await expect(modal.getByRole('alert')).toHaveCount(0);
  await expect(
    modal.getByRole('button', { name: commonLabel('useWorkspace') })
  ).toBeEnabled();

  await modal.getByRole('button', { name: commonLabel('useWorkspace') }).click();
  const sameNamed = JSON.parse(await page.locator('#result').textContent());
  assert.deepEqual(sameNamed.selection.repos.map((item) => item.repo.id).sort(), ['a', 'outside']);
  await page.getByRole('button', { name: 'Orphan new', exact: true }).click();
  await modal.getByRole('tab').nth(1).click();
  await expect(modal.getByRole('checkbox')).toHaveCount(0);
  await modal.getByRole('textbox').fill('/synthetic/non-git');
  await modal.getByRole('button', { name: 'キャンセル', exact: true }).click();
  await page.getByRole('button', { name: 'Project new', exact: true }).click();
  await expect(modal.getByRole('checkbox').nth(0)).toBeChecked();
  await expect(modal.getByRole('checkbox').nth(1)).toBeChecked();
  await modal.getByRole('button', { name: 'キャンセル', exact: true }).click();
  await page.getByRole('button', { name: 'Preset', exact: true }).click();
  await expect(modal.getByRole('tab')).toHaveCount(0);
  await expect(modal.getByText('main', { exact: true })).toHaveCount(0);
  await expect(modal.getByRole('checkbox')).toHaveCount(3);
  assert.deepEqual(errors, []);
  console.log(
    'PASS: Project preset subset/empty, orphan free/same-name selection, direct tab, cancel/reopen, preset without branches'
  );
} finally {
  await browser.close();
}
function commonLabel(key) {
  return { useWorkspace: 'このワークスペースを使用' }[key];
}
