import { expect } from '@playwright/test';
import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';

// CDP sends trusted touch input through Chromium's gesture recognizer, including
// native scrolling, pointercancel and compatibility clicks. No dispatchEvent.
export async function touchChecks({
  browser,
  page,
  url,
  issues,
  statuses,
  api,
  output,
  tag,
}) {
  const results = [];
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.addInitScript(() => {
    window.touchTrace = [];
    for (const type of [
      'pointerdown',
      'pointermove',
      'pointercancel',
      'touchstart',
      'touchmove',
      'touchend',
      'touchcancel',
      'mousedown',
      'click',
      'focusin',
    ]) {
      document.addEventListener(
        type,
        (e) => {
          const target = e.target;
          window.touchTrace.push({
            type,
            target: target.tagName,
            card: target.closest?.('[data-kanban-card]')?.dataset.kanbanCard,
            text: target.textContent?.slice(0, 55),
            x: e.clientX ?? e.changedTouches?.[0]?.clientX,
            y: e.clientY ?? e.changedTouches?.[0]?.clientY,
          });
        },
        true
      );
    }
  });
  const card = (id) => page.locator(`[data-kanban-card="${id}"]`);
  const reset = async () => {
    await page.goto(url);
    await expect(card(issues[0].id)).toBeVisible({ timeout: 90000 });
    await expect(
      card(issues[0].id).getByRole('button', { name: 'Touch tag', exact: true })
    ).toBeVisible();
    await page.waitForTimeout(200);
  };
  const cdp = await page.context().newCDPSession(page);
  const point = async (tags = false) => {
    const locator = tags
      ? card(issues[0].id).getByRole('button', {
          name: 'Touch tag',
          exact: true,
        })
      : card(issues[0].id).getByText(issues[0].title, { exact: true });
    const box = await locator.boundingBox();
    assert.ok(box);
    return {
      x: box.x + Math.min(box.width / 2, 100),
      y: box.y + box.height / 2,
    };
  };
  const send = (type, p) =>
    cdp.send('Input.dispatchTouchEvent', {
      type,
      touchPoints: p ? [{ ...p, id: 1 }] : [],
    });
  const gesture = async (start, dx, dy, hold = 0) => {
    await send('touchStart', start);
    if (hold) await page.waitForTimeout(hold);
    for (let i = 1; i <= 8; i++) {
      await send('touchMove', {
        x: start.x + (dx * i) / 8,
        y: start.y + (dy * i) / 8,
      });
      await page.waitForTimeout(12);
    }
    await send('touchEnd');
    await page.waitForTimeout(350);
  };
  const check = async (label, fn) => {
    await reset();
    try {
      await fn();
      results.push({ label, result: 'passed' });
    } catch (e) {
      await send('touchCancel').catch(() => {});
      results.push({ label, result: 'failed', error: e.message });
    }
    results.at(-1).trace = await page.evaluate(() => window.touchTrace);
    await writeFile(`${output}/results.json`, JSON.stringify(results, null, 2));
  };
  for (const tags of [false, true]) {
    for (const [axis, dx, dy] of [
      ['vertical', 0, -120],
      ['horizontal', -120, 0],
      ['small-x', 7, 0],
      ['small-y', 0, 7],
      ['out-and-back', 0, 0],
    ]) {
      await check(`${tags ? 'tags' : 'body'} ${axis}`, async () => {
        const start = await point(tags);
        await page.evaluate(() => {
          window.scrollProbe = [...document.querySelectorAll('*')].map((n) => ({
            n,
            x: n.scrollLeft,
            y: n.scrollTop,
          }));
        });
        if (axis === 'out-and-back') {
          await send('touchStart', start);
          await send('touchMove', { x: start.x + 7, y: start.y });
          await send('touchMove', start);
          await send('touchEnd');
          await page.waitForTimeout(350);
        } else await gesture(start, dx, dy);
        assert.equal(page.url(), url, 'scroll must not navigate');
        await expect(page.getByRole('menu')).toHaveCount(0);
        if (axis === 'vertical' || axis === 'horizontal') {
          const scrolled = await page.evaluate(
            (axis) =>
              window.scrollProbe.some(({ n, x, y }) =>
                axis === 'vertical'
                  ? Math.abs(n.scrollTop - y) > 2
                  : Math.abs(n.scrollLeft - x) > 2
              ),
            axis
          );
          assert.ok(scrolled, `${axis} native scroll offset must change`);
        }
      });
    }
  }
  await check('cancel does not open UI and next tap works', async () => {
    await send('touchStart', await point());
    await send('touchCancel');
    await expect(page.getByRole('menu')).toHaveCount(0);
    assert.equal(page.url(), url);
    await page.touchscreen.tap(...Object.values(await point()));
    await expect(page).toHaveURL(new RegExp(`/issues/${issues[0].id}`));
  });
  await check('stationary body tap opens details', async () => {
    await page.touchscreen.tap(...Object.values(await point()));
    await expect(page).toHaveURL(new RegExp(`/issues/${issues[0].id}`));
  });
  await check(
    'tag opens only on release and supports keyboard editing',
    async () => {
      await send('touchStart', await point(true));
      await page.waitForTimeout(100);
      await expect(page.getByRole('menu')).toHaveCount(0);
      await send('touchEnd');
      await expect(page.getByRole('menu')).toBeVisible();
      const input = page.getByRole('menu').getByRole('textbox');
      await expect(input).toBeFocused();
      await input.press('ArrowDown');
      await input.press('Enter');
      await input.press('Escape');
      await expect(page.getByRole('menu')).toHaveCount(0);
      assert.equal(page.url(), url);
      // Restore tag through real API if keyboard edit removed it.
      const rows = (
        await api(
          `/api/local/v1/fallback/issue_tags?project_id=${issues[0].project_id}`
        )
      ).issue_tags;
      assert.ok(
        !rows.some((r) => r.issue_id === issues[0].id && r.tag_id === tag.id),
        'keyboard removed selected tag'
      );
      await api('/api/local/v1/issue_tags', 'POST', {
        issue_id: issues[0].id,
        tag_id: tag.id,
      });
    }
  );
  // The tag-edit check is last among reset-dependent checks: it may remove the
  // label. Reload for DnD using only the title, not the tag prerequisite.
  await page.goto(url);
  await expect(card(issues[0].id)).toBeVisible({ timeout: 90000 });
  try {
    const startPoint = await point();
    const destinationBox = await page
      .locator(`[data-kanban-column="${statuses[1].id}"] [data-kanban-card]`)
      .first()
      .boundingBox();
    const destinationPoint = {
      x: Math.min(370, destinationBox.x + 35),
      y: startPoint.y,
    };
    const target = await page.evaluate(
      (p) => document.elementFromPoint(p.x, p.y)?.outerHTML,
      startPoint
    );
    await send('touchStart', startPoint);
    await page.waitForTimeout(300);
    await expect(page.locator('[data-kanban-ghost]')).toHaveCount(1);
    for (let i = 1; i <= 12; i++) {
      await send('touchMove', {
        x: startPoint.x + ((destinationPoint.x - startPoint.x) * i) / 12,
        y: startPoint.y,
      });
      await page.waitForTimeout(20);
    }
    await send('touchEnd');
    results.push({
      label: 'long press activation target',
      result: 'passed',
      target,
    });
    await expect(page.locator('[data-kanban-ghost]')).toHaveCount(0);
    assert.equal(page.url(), url);
    await expect
      .poll(async () => {
        const rows = (
          await api(
            `/api/local/v1/fallback/issues?project_id=${issues[0].project_id}`
          )
        ).issues;
        return rows.find((r) => r.id === issues[0].id).status_id;
      })
      .toBe(statuses[1].id);
    await page.reload();
    await expect(card(issues[0].id)).toBeVisible({ timeout: 90000 });
    results.push({
      label:
        'long press touch column move persists through real API and reload',
      result: 'passed',
    });
    await page.screenshot({ path: `${output}/mobile.png` });
  } catch (e) {
    results.push({
      label: 'long press touch column move',
      result: 'failed',
      error: e.message,
      trace: await page.evaluate(() => window.touchTrace),
    });
  }
  const desktop = await browser.newPage({
    viewport: { width: 1280, height: 900 },
  });
  try {
    await desktop.goto(url);
    const a = desktop.locator(`[data-kanban-card="${issues[0].id}"]`);
    await expect(a).toBeVisible({ timeout: 90000 });
    await a.getByText(issues[0].title, { exact: true }).click();
    await expect(desktop).toHaveURL(new RegExp(`/issues/${issues[0].id}`));
    await desktop.goto(url);
    await expect(a).toBeVisible({ timeout: 90000 });
    const b = desktop
      .locator(`[data-kanban-column="${statuses[2].id}"] [data-kanban-card]`)
      .first();
    const start = await a.boundingBox(),
      end = await b.boundingBox();
    await desktop.mouse.move(start.x + 100, start.y + 35);
    await desktop.mouse.down();
    await desktop.mouse.move(end.x + 100, end.y + 35, { steps: 16 });
    await desktop.mouse.up();
    await expect(
      desktop.locator(`[data-kanban-column="${statuses[2].id}"]`)
    ).toContainText(issues[0].title);
    await expect
      .poll(
        async () =>
          (
            await api(
              `/api/local/v1/fallback/issues?project_id=${issues[0].project_id}`
            )
          ).issues.find((r) => r.id === issues[0].id).status_id
      )
      .toBe(statuses[2].id);
    // Reload after the saved mouse move, as in isolated.mjs: do not depend on
    // the separate live DnD projection/persistence issue's transient layout.
    await desktop.reload();
    await expect(
      desktop.locator(
        `[data-kanban-column="${statuses[2].id}"] [data-kanban-card="${issues[0].id}"]`
      )
    ).toBeVisible();

    await a.focus();
    await desktop.keyboard.press('Space', { delay: 100 });
    await expect(desktop.locator('[data-kanban-ghost]')).toHaveCount(1);
    await desktop.keyboard.press('ArrowLeft', { delay: 100 });
    await desktop.keyboard.press('Space', { delay: 100 });
    await expect(desktop.locator('[data-kanban-ghost]')).toHaveCount(0);
    await expect
      .poll(async () => {
        const rows = (
          await api(
            `/api/local/v1/fallback/issues?project_id=${issues[0].project_id}`
          )
        ).issues;
        return rows.find((r) => r.id === issues[0].id).status_id;
      })
      .toBe(statuses[1].id);
    results.push({
      label: 'keyboard drop focus observation',
      result: 'passed',
      focusAfterDrop: await a.evaluate((n) => n === document.activeElement),
    });
    assert.equal(desktop.url(), url);
    await desktop.screenshot({ path: `${output}/desktop.png` });
    results.push({
      label: 'desktop click, mouse column move and keyboard DnD',
      result: 'passed',
    });
  } catch (e) {
    results.push({
      label: 'desktop interactions',
      result: 'failed',
      error: e.message,
    });
  } finally {
    await desktop.close();
  }
  assert.deepEqual(errors, []);
  await writeFile(`${output}/results.json`, JSON.stringify(results, null, 2));
  console.log(
    results.map(({ label, result }) => `${result}: ${label}`).join('\n')
  );
  assert.equal(
    results.filter((r) => r.result === 'failed').length,
    0,
    'see results.json for failures'
  );
}
