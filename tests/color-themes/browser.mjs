// No running backend/dev server or shared data. The saved config is fixture state.
import { build } from 'esbuild';
import { chromium, expect } from '@playwright/test';
import { createRequire } from 'node:module';
import { mkdir, readFile } from 'node:fs/promises';
const root = process.cwd();
const require = createRequire(`${root}/packages/web-core/package.json`);
const bundle = await build({
  entryPoints: ['tests/color-themes/fixture.tsx'],
  bundle: true,
  write: false,
  format: 'iife',
  jsx: 'automatic',
  alias: {
    '@': `${root}/packages/web-core/src`,
    shared: `${root}/shared`,
  },
  nodePaths: [
    `${root}/packages/web-core/node_modules`,
    `${root}/packages/ui/node_modules`,
  ],
  loader: { '.css': 'empty', '.woff2': 'empty', '.woff': 'empty' },
  define: { 'import.meta.env': '{}', 'import.meta.hot': 'undefined' },
});
const config = require('tailwindcss/loadConfig')(
  `${root}/packages/local-web/tailwind.new.config.js`
);
const styles = [];
for (const file of [
  'packages/web-core/src/app/styles/new/index.css',
  'packages/web-core/src/app/styles/diff-style-overrides.css',
]) {
  const source = (await readFile(file, 'utf8')).replace(
    /^@(import|config).*$/gm,
    ''
  );
  const processed = await require('postcss')([
    require('tailwindcss')({
      ...config,
      content: [
        `${root}/tests/color-themes/fixture.tsx`,
        `${root}/packages/web-core/src/shared/dialogs/settings/settings/ColorThemeSettings.tsx`,
        `${root}/packages/web-core/src/shared/dialogs/settings/settings/SettingsComponents.tsx`,
        `${root}/packages/ui/src/components/PrimaryButton.tsx`,
      ],
    }),
  ]).process(source, { from: undefined });
  styles.push(processed.css);
}
const browser = await chromium.launch({
  headless: true,
  executablePath: process.env.LVK_E2E_CHROMIUM,
  args: ['--no-sandbox'],
});
try {
  const page = await browser.newPage({
    viewport: { width: 1100, height: 900 },
  });
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.route('**/*', (route) =>
    route.fulfill({
      contentType: 'text/html',
      body: '<!doctype html><div id="root"></div>',
    })
  );
  await page.emulateMedia({ colorScheme: 'light' });
  await page.goto('https://fixture.invalid/');
  for (const css of styles) await page.addStyleTag({ content: css });
  await page.addScriptTag({ content: bundle.outputFiles[0].text });
  const html = page.locator('html');
  const save = page.getByRole('button', { name: '保存' });
  const groups = page.getByRole('radiogroup');

  // System mode: both OS rows, standard light applied without an attribute.
  await expect(groups).toHaveCount(3);
  await expect(page.getByText('OS がライトのとき')).toBeVisible();
  await expect(page.getByText('OS がダークのとき')).toBeVisible();
  await expect(html).toHaveClass(/\blight\b/);
  await expect(html).not.toHaveAttribute('data-color-theme');

  // Keyboard: Tab reaches the mode group, then the light row; arrows select.
  await page.keyboard.press('Tab');
  await expect(page.getByRole('radio', { name: 'システム' })).toBeFocused();
  await page.keyboard.press('Tab');
  const lightStandard = groups.nth(1).getByRole('radio').first();
  await expect(lightStandard).toBeFocused();
  await page.keyboard.press('ArrowRight');
  const paper = page.getByRole('radio', { name: 'Paper' });
  await expect(paper).toBeFocused();
  await expect(paper).toHaveAttribute('aria-checked', 'true');
  const ring = await paper.evaluate(
    (element) => getComputedStyle(element).boxShadow
  );
  if (!ring || ring === 'none')
    throw new Error('focused card has no visible focus ring');
  await page.getByRole('radio', { name: 'Navy' }).click();

  // A draft choice is never applied before saving.
  await expect(html).not.toHaveAttribute('data-color-theme');
  await save.click();
  await expect(html).toHaveAttribute('data-color-theme', 'paper');

  // System follows the OS while keeping each side's saved colour theme.
  await page.emulateMedia({ colorScheme: 'dark' });
  await expect(html).toHaveClass(/\bdark\b/);
  await expect(html).toHaveAttribute('data-color-theme', 'navy');
  await page.emulateMedia({ colorScheme: 'light' });
  await expect(html).toHaveClass(/\blight\b/);
  await expect(html).toHaveAttribute('data-color-theme', 'paper');

  // Fixed modes show one row only.
  await page.getByRole('radio', { name: 'ダーク' }).click();
  await expect(groups).toHaveCount(2);
  await expect(page.getByText('ダークのカラーテーマ')).toBeVisible();
  await expect(html).toHaveClass(/\blight\b/);

  await save.click();
  await expect(html).toHaveClass(/\bdark\b/);
  await expect(html).toHaveAttribute('data-color-theme', 'navy');
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.emulateMedia({ colorScheme: 'light' });
  await expect(html).toHaveClass(/\bdark\b/);
  // Returning to System retains the independently saved light/dark choices.
  await page.getByRole('radio', { name: 'システム' }).click();
  await expect(page.getByRole('radio', { name: 'Paper' })).toHaveAttribute(
    'aria-checked',
    'true'
  );
  await expect(page.getByRole('radio', { name: 'Navy' })).toHaveAttribute(
    'aria-checked',
    'true'
  );
  await save.click();
  await expect(html).toHaveAttribute('data-color-theme', 'paper');

  // Computed browser colours exercise Tailwind mappings, not only raw tokens.
  const buttonContrast = (button) =>
    button.evaluate((element) => {
      const style = getComputedStyle(element);
      const luminance = (value) => {
        const channels = value
          .match(/[\d.]+/g)
          .slice(0, 3)
          .map(Number)
          .map((v) => {
            const c = v / 255;
            return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
          });
        return (
          channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722
        );
      };
      const a = luminance(style.color),
        b = luminance(style.backgroundColor);
      return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
    });

  // Screenshots of every saved combination at desktop and mobile widths.
  await mkdir('test-results/color-themes', { recursive: true });
  const combos = [
    ['LIGHT', 'STANDARD', 'STANDARD', 'light-standard'],
    ['LIGHT', 'PAPER', 'STANDARD', 'light-paper'],
    ['LIGHT', 'LINEN', 'STANDARD', 'light-linen'],
    ['DARK', 'STANDARD', 'STANDARD', 'dark-standard'],
    ['DARK', 'STANDARD', 'NAVY', 'dark-navy'],
    ['DARK', 'STANDARD', 'INDIGO', 'dark-indigo'],
  ];
  for (const [theme, light, dark, name] of combos) {
    await page.evaluate((saved) => window.setSavedTheme(saved), {
      theme,
      light,
      dark,
    });
    const attribute = name.split('-')[1];
    if (attribute === 'standard')
      await expect(html).not.toHaveAttribute('data-color-theme');
    else await expect(html).toHaveAttribute('data-color-theme', attribute);
    await expect(
      page.getByRole('radio', {
        name: theme === 'LIGHT' ? 'ライト' : 'ダーク',
        exact: true,
      })
    ).toHaveAttribute('aria-checked', 'true');
    await page.setViewportSize({ width: 1100, height: 900 });
    for (const label of ['新規 Issue', '開始', '削除', '承認']) {
      const button = page.getByRole('button', { name: label, exact: true });
      await expect
        .poll(() => buttonContrast(button))
        .toBeGreaterThanOrEqual(4.5);
      if (label === '新規 Issue' || label === '開始') {
        await button.hover();
        await expect
          .poll(() => buttonContrast(button))
          .toBeGreaterThanOrEqual(4.5);
        await page.mouse.move(0, 0);
      }
    }
    await page.screenshot({
      path: `test-results/color-themes/${name}.png`,
      fullPage: true,
      animations: 'disabled',
    });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.screenshot({
      path: `test-results/color-themes/${name}-mobile.png`,
      fullPage: true,
      animations: 'disabled',
    });
  }
  if (errors.length) throw new Error(errors.join('\n'));
  console.log('color theme browser checks passed');
} finally {
  await browser.close();
}
