import assert from "node:assert/strict";
import { expect } from "@playwright/test";
import path from "node:path";

// Prefix to the existing delayed-save/mouse/keyboard/column sequence. No reload
// or navigation between native scrolling, tapping, touch sorting and that sequence.
export async function continuousTouchPrefix({
  page,
  card,
  columns,
  issues,
  api,
  project,
  output,
  evidence,
}) {
  const id = issues[0].id;
  const cdp = await page.context().newCDPSession(page);
  const send = (type, p) =>
    cdp.send("Input.dispatchTouchEvent", {
      type,
      touchPoints: p ? [{ ...p, id: 1 }] : [],
    });
  const gesture = async (from, to, hold = 0) => {
    await send("touchStart", from);
    if (hold) await page.waitForTimeout(hold);
    for (let i = 1; i <= 16; i++) {
      await send("touchMove", {
        x: from.x + ((to.x - from.x) * i) / 16,
        y: from.y + ((to.y - from.y) * i) / 16,
      });
      await page.waitForTimeout(20);
    }
    await send("touchEnd");
    await page.waitForTimeout(400);
  };
  const url = page.url();
  await page.setViewportSize({ width: 1280, height: 700 });
  const scroller = await card(id).evaluateHandle((el) => {
    for (let p = el.parentElement; p; p = p.parentElement)
      if (
        p.scrollHeight > p.clientHeight &&
        /auto|scroll/.test(getComputedStyle(p).overflowY)
      )
        return p;
    throw Error("No native scroll container");
  });
  const box = await card(issues[3].id).boundingBox();
  assert.ok(box);
  const start = { x: box.x + 70, y: Math.min(550, box.y + 25) };
  await gesture(start, { x: start.x, y: start.y - 160 });
  assert.ok(
    await scroller.evaluate((el) => el.scrollTop > 0),
    "trusted touch must actually scroll",
  );
  assert.equal(page.url(), url);
  await expect(page.locator('[role="menu"]')).toHaveCount(0);
  await expect(page.locator("[data-kanban-ghost]")).toHaveCount(0);
  await scroller.evaluate((el) => {
    el.scrollTop = 0;
  });
  await page.waitForTimeout(300);
  const summary = card(id).locator("summary");
  const summaryBox = await summary.boundingBox();
  await page.touchscreen.tap(summaryBox.x + 50, summaryBox.y + 10);
  await expect(card(id).locator("details")).toHaveAttribute("open", "");
  await page.touchscreen.tap(summaryBox.x + 50, summaryBox.y + 10);
  await expect(card(id).locator("details")).not.toHaveAttribute("open", "");
  assert.equal(page.url(), url);
  const from = await card(id).boundingBox();
  const to = await card(issues[2].id).boundingBox();
  await gesture(
    { x: from.x + 55, y: from.y + 20 },
    { x: to.x + 55, y: to.y + 20 },
    300,
  );
  const saved = async () =>
    (
      await api(`/api/local/v1/fallback/issues?project_id=${project}`)
    ).issues.find((i) => i.id === id);
  // Bouncy sibling displacement can cross one additional collision target.
  // Verify an actual fractional midpoint and exact DOM/API agreement, rather
  // than hard-coding the hit-test result of an animated pointer trajectory.
  await expect.poll(async () => (await saved()).sort_order % 1).toBe(0.5);
  const savedOrder = async () => (await api(`/api/local/v1/fallback/issues?project_id=${project}`)).issues
    .filter(i => i.status_id === issues[0].status_id)
    .sort((a, b) => a.sort_order - b.sort_order).map(i => i.id);
  const displayedOrder = () => columns.nth(0).locator('[data-kanban-card]').evaluateAll(els => els.map(e => e.dataset.kanbanCard));
  await expect.poll(displayedOrder).toEqual(await savedOrder());
  const touchedIndex = (await displayedOrder()).indexOf(id);
  assert.ok(touchedIndex >= 1 && touchedIndex <= 3, 'touch moved within the nearby destination region');
  await page.screenshot({
    path: path.join(output, "continuous-touch-prefix.png"),
  });
  evidence.push({
    label:
      "NO RELOAD prefix: trusted touch native scroll -> summary tap twice -> long-press same-column fractional midpoint saved through real API; continues into delayed keyboard/mouse/cross-column sequence",
    saved: await saved(),
  });
  // Return the very same touched card to the first position using current
  // metadata, so the following regression continues operating on that card.
  for (let i = 0; i < touchedIndex; i++) {
    await card(id).focus();
    await page.keyboard.press('Space', { delay: 100 });
    await page.keyboard.press('ArrowUp', { delay: 100 });
    await page.keyboard.press('Space', { delay: 100 });
  }
  await expect.poll(() => columns.nth(0).locator('[data-kanban-card]').first().getAttribute('data-kanban-card')).toBe(id);
  await expect.poll(async () => (await saved()).sort_order).toBe(0);
  await expect(card(id)).toBeFocused();
  await page.setViewportSize({ width: 2560, height: 1440 });
  await cdp.detach();
}
