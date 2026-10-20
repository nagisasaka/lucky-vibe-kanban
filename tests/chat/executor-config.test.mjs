import { test } from 'node:test';
import assert from 'node:assert/strict';
import { build } from 'esbuild';
import { chromium } from '@playwright/test';
import { fileURLToPath } from 'node:url';

test('executor hook preserves explicit reasoning and inherited model across remounts', async () => {
  const result = await build({
    entryPoints: [fileURLToPath(new URL('./executor-config.fixture.tsx', import.meta.url))],
    bundle: true, write: false, format: 'iife', jsx: 'automatic',
    nodePaths: [fileURLToPath(new URL('../../packages/web-core/node_modules', import.meta.url))],
    alias: { '@': fileURLToPath(new URL('../../packages/web-core/src', import.meta.url)) },
    plugins: [{ name: 'synthetic-preset', setup(builder) {
      builder.onResolve({ filter: /usePresetOptions$/ }, () => ({ path: 'preset', namespace: 'mock' }));
      builder.onLoad({ filter: /.*/, namespace: 'mock' }, () => ({ contents: 'export const usePresetOptions = () => ({data: {executor: "CODEX", reasoning_id: "low"}});' }));
    } }],
  });
  const browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
  try {
    const page = await browser.newPage();
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.setContent('<div id="root"></div>');
    await page.addScriptTag({ content: result.outputFiles[0].text });
    const read = async (id = 'config') => JSON.parse(await page.locator(`#${id}`).textContent());
    const click = label => page.getByRole('button', { name: label, exact: true }).click();
    await page.locator('#config').waitFor();
    assert.equal((await read()).reasoning_id, 'high', 'legacy partial draft inherits the launch model');
    await click('Ultra');
    assert.equal((await read('saved')).model_id, 'test-model');
    await click('Other override');
    assert.equal((await read('saved')).reasoning_id, 'ultra');
    await click('Remount');
    assert.equal((await read()).reasoning_id, 'ultra');
    await click('Switch workspace');
    assert.equal((await read()).reasoning_id, undefined, 'a sibling must not borrow manual or preset reasoning');
    await click('Switch workspace');
    assert.equal((await read()).reasoning_id, 'ultra');
    await click('Follow Settings');
    await click('Remount');
    assert.equal((await read()).reasoning_id, null, 'explicit inheritance blocks the previous launch effort');
    await click('Ultra');
    await click('Change model');
    await click('Remount');
    assert.equal((await read()).model_id, 'other-model');
    assert.equal((await read()).reasoning_id, null, 'changing model must not restore preset/launch reasoning');
    assert.deepEqual(errors, []);
  } finally { await browser.close(); }
});
