// Offline component QA only. No real app server, auth or approval endpoint.
import { build } from 'esbuild';
import { chromium, expect } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
const root = process.cwd();
const output = `${root}/.build/local-295/mcp-approval`;
await mkdir(output, { recursive: true });
const bundle = await build({
  entryPoints: ['tests/chat/mcp-approval.fixture.tsx'],
  bundle: true,
  write: false,
  format: 'iife',
  jsx: 'automatic',
  nodePaths: [`${root}/packages/web-core/node_modules`],
});
const css = execFileSync(
  `${root}/packages/local-web/node_modules/.bin/tailwindcss`,
  [
    '-i',
    '../web-core/src/app/styles/new/index.css',
    '--config',
    'tailwind.new.config.js',
    '--minify',
  ],
  {
    cwd: `${root}/packages/local-web`,
    encoding: 'utf8',
    maxBuffer: 20 * 1024 * 1024,
  }
).replace(/@import\s*(?:url\([^)]*\)|"[^"]*"|'[^']*')\s*;/g, '');
const html = `<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><style>${css}</style><div id="root"></div><script>${bundle.outputFiles[0].text.replaceAll('</script', '<\\/script')}</script>`;
await writeFile(`${output}/fixture.html`, html);
console.log(`Offline fixture: ${output}/fixture.html`);
if (!process.argv.includes('--prepare-only')) {
  const browser = await chromium.launch({
    headless: true,
    args: ['--no-sandbox'],
  });
  try {
    const page = await browser.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(e.message));
    await page.route('**/*', (route) => route.abort());
    for (const mobile of [false, true]) {
      await page.setViewportSize(
        mobile ? { width: 390, height: 844 } : { width: 1280, height: 900 }
      );
      await page.setContent(html);
      if (mobile)
        await page.evaluate(() =>
          document.documentElement.classList.add('dark')
        );
      await expect(page.locator('main')).toContainText(
        'MCP third-party server="lvk_fixture"'
      );
      await expect(page.locator('#mcp-target')).toContainText(
        'get_dev_server_status'
      );
      await expect(page.locator('#mcp-target')).toContainText('one call only');
      await expect(page.locator('output')).toHaveText('No mock decision');
      const approve = page.getByRole('button', { name: '承認', exact: true });
      await expect(approve).toBeEnabled();
      const details = page.getByRole('region', { name: '実行承認の詳細' });
      // Native section role requires an accessible name; expand in place next to controls.
      await details.getByText('実行承認の詳細', { exact: true }).click();
      await expect(details).toContainText('lvk_fixture/get_dev_server_status');
      await expect(details).not.toContainText('&#x2F;');
      await expect(details).toContainText('project-fixture');
      await expect(details).toContainText('workspace-fixture');
      await expect(details).toContainText('session-fixture');
      await expect(details).toContainText('run-fixture');
      await expect(details).toContainText('[REDACTED: credential field]');
      await page
        .getByRole('button', { name: 'Toggle empty arguments' })
        .click();
      await expect(details).toContainText('引数なし ({})');
      await approve.click();
      await expect(page.locator('output')).toHaveText('MOCK ONLY: approve');
      if (mobile) {
        await page.getByRole('button', { name: '拒否', exact: true }).click();
        await expect(page.locator('output')).toHaveText('MOCK ONLY: deny');
      }

      await page.screenshot({
        path: `${output}/${mobile ? 'mobile-dark' : 'desktop-light'}.png`,
        fullPage: true,
      });
      await page
        .getByRole('button', { name: 'Toggle third-party tool' })
        .click();
      await expect(page.locator('#mcp-target')).toContainText(
        'third_party_example'
      );
      await expect(page.locator('#mcp-target')).toContainText('run_process');
      await page.getByRole('button', { name: 'Toggle timeout' }).click();
      await expect(approve).toHaveCount(0);
    }
    if (errors.length) throw new Error(errors.join('\n'));
    console.log(
      'PASS offline approval components: target identity/scope, no automatic mock decision, timeout removes approval controls; no network/API/provider'
    );
  } finally {
    await browser.close();
    console.log('CLEANUP fixture browser closed');
  }
}
