import assert from 'node:assert/strict';
import { expect } from '@playwright/test';

// Native route only: no DOM replacement, module injection, provider substitution,
// API/WS mocks, or pre-setting file selection/panel preferences.
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
  assert.match(
    JSON.stringify(await response.json()),
    /const nativeSelected = 120;/
  );
  await expect(
    page.getByRole('button', { name: 'Close preview', exact: true })
  ).toBeVisible();
  const selected = page.locator('[aria-current="location"]');
  await expect(selected).toHaveCount(1);
  await expect(selected.locator('code')).toHaveText(
    'const nativeSelected = 120;'
  );
  await expect(selected.locator('span')).toHaveText('120');
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
