import assert from 'node:assert/strict';
import { expect } from '@playwright/test';

// Native route and file API; provider account and discovery are isolated by the runner.
export async function verifyNativeFileLink(
  page,
  base,
  workspaceId,
  repoId,
  sessionId
) {
  await page.goto(`${base}/workspaces/${workspaceId}?session_id=${sessionId}`, {
    waitUntil: 'domcontentloaded',
  });
  // Fresh-browser first-visit guide is dismissed through its real UI.
  const guide = page.getByRole('button', { name: 'Close', exact: true });
  await expect(guide).toBeVisible();
  await guide.click();
  await expect(guide).toHaveCount(0);
  const link = page.getByRole('link', {
    name: 'Open native source',
    exact: true,
  });
  await expect(link).toBeVisible();
  await expect(link).not.toHaveAttribute('aria-disabled', 'true');
  await expect(
    page.getByRole('button', { name: 'Close preview', exact: true })
  ).toHaveCount(0);
  await expect(page.locator('[aria-current="location"]')).toHaveCount(0);
  const contentResponse = page.waitForResponse((response) => {
    const url = new URL(response.url());
    return (
      url.pathname === `/api/workspaces/${workspaceId}/files/content` &&
      url.searchParams.get('repo_id') === repoId &&
      url.searchParams.get('path') === 'src/example.ts'
    );
  });
  const [response] = await Promise.all([contentResponse, link.click()]);
  assert.equal(response.status(), 200);
  const fileResponse = await response.json();
  const serialized = JSON.stringify(fileResponse);
  assert.match(serialized, /const nativeSelected = 120;/);
  assert.match(serialized, /\\r\\n/, 'file API preserves CRLF');
  await expect(
    page.getByRole('button', { name: 'Close preview', exact: true })
  ).toBeVisible();
  const selected = page.locator('[aria-current="location"]');
  await expect(selected).toHaveCount(1);
  await expect(selected.locator('code')).toHaveText(
    'const nativeSelected = 120;'
  );
  await expect.poll(() => selected.locator('.shiki-token').count()).toBeGreaterThan(1);
  const code = selected.locator('code');
  const token = selected.locator('.shiki-token').first();
  const light = await token.evaluate((el) => getComputedStyle(el).color);
  await page.evaluate(() => document.documentElement.classList.add('dark'));
  const dark = await token.evaluate((el) => getComputedStyle(el).color);
  assert.notEqual(light, dark, 'file code uses both Shiki themes');
  await expect(code).toHaveText('const nativeSelected = 120;');
  await page.screenshot({ path: '.build/local432-file-preview.png', fullPage: true });
  await expect(selected.locator('span').first()).toHaveText('120');
  await expect(selected.locator('xpath=parent::*').locator(':scope > div')).toHaveCount(160);
  await selected.evaluate((row) => {
    const first = row.querySelector('code');
    const second = row.nextElementSibling.querySelector('code');
    const range = document.createRange();
    range.setStart(first, 0);
    range.setEnd(second, second.childNodes.length);
    const selection = window.getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
  });
  await page.keyboard.press('Control+c');
  assert.equal(await page.evaluate(() => navigator.clipboard.readText()),
    'const nativeSelected = 120;\n// native line 121');
  await expect(selected).toBeInViewport();
  // Read actual layout geometry: the row is inside its scrolled viewer viewport.
  const position = await selected.evaluate((row) => {
    const scroller = row.parentElement.parentElement;
    const r = row.getBoundingClientRect();
    const v = scroller.getBoundingClientRect();
    return {
      scrollTop: scroller.scrollTop,
      rowTop: r.top,
      rowBottom: r.bottom,
      viewerTop: v.top,
      viewerBottom: v.bottom,
    };
  });
  assert.ok(
    position.scrollTop > 0,
    'line 120 requires scrolling the native viewer'
  );
  assert.ok(
    position.rowTop >= position.viewerTop &&
      position.rowBottom <= position.viewerBottom
  );
  assert.equal(page.context().pages().length, 1);
  console.log(
    'Native chat → layout file panel → viewer passed:',
    JSON.stringify(position)
  );
}
