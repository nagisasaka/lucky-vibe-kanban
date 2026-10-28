import { expect } from "@playwright/test";
import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import path from "node:path";

export async function runCrossColumnApi({
  page,
  columns,
  card,
  api,
  project,
  statuses,
  issues,
  output,
  evidence,
}) {
  const moved = issues[0].id;
  const ids = (col) =>
    columns
      .nth(col)
      .locator("[data-kanban-card]")
      .evaluateAll((els) => els.map((e) => e.dataset.kanbanCard));
  const rows = async () =>
    (await api(`/api/local/v1/fallback/issues?project_id=${project}`)).issues;
  const savedIds = async (col) =>
    (await rows())
      .filter((i) => i.status_id === statuses[col].id)
      .sort((a, b) => a.sort_order - b.sort_order)
      .map((i) => i.id);
  const writes = [];
  page.on("request", (r) => {
    if (r.url().includes("/issues/bulk")) writes.push(r.postDataJSON());
  });
  const hover = async (column, index) => {
    const from = await card(moved).boundingBox();
    const targetIds = (await ids(column)).filter((id) => id !== moved);
    const target =
      index < targetIds.length
        ? await card(targetIds[index]).boundingBox()
        : await columns.nth(column).boundingBox();
    const point = {
      x: target.x + 30,
      y: target.y + (index < targetIds.length ? 22 : 40),
    };
    await page.mouse.move(from.x + 30, from.y + 20);
    await page.mouse.down();
    await page.mouse.move(from.x + 45, from.y + 20, { steps: 5 });
    await page.mouse.move(point.x, point.y, { steps: 25 });
    const expected = [...targetIds];
    expected.splice(index, 0, moved);
    await expect.poll(() => ids(column)).toEqual(expected);
    await expect(card(moved)).toHaveCount(1);
    return expected;
  };
  const drop = async (column, index) => {
    const count = writes.length;
    const expected = await hover(column, index);
    assert.equal(writes.length, count, "hover must not save");
    await page.mouse.up();
    await expect
      .poll(() => savedIds(column), { timeout: 20000 })
      .toEqual(expected);
    await expect.poll(() => ids(column)).toEqual(expected);
    assert.equal(writes.length, count + 1, "one drop commits one card once");
    assert.equal(writes.at(-1).updates.length, 1);
    return expected;
  };
  for (const width of [360, 600]) {
    await columns.evaluateAll(
      (els, w) =>
        els.forEach((el) => {
          el.parentElement.style.width = `${w}px`;
        }),
      width,
    );
    const targetId = (await ids(1))[2];
    const before = await card(targetId).boundingBox();
    let expected = await drop(1, 2);
    const after = await card(targetId).boundingBox();
    assert.ok(Math.abs(after.y - before.y) + Math.abs(after.x - before.x) > 20);
    await expect(card(moved)).toHaveCount(1);
    await card(moved).focus();
    await page.keyboard.press("Space", { delay: 100 });
    await page.keyboard.press(width < 536 ? "ArrowDown" : "ArrowRight", {
      delay: 100,
    });
    await page.keyboard.press("Space", { delay: 100 });
    await expect.poll(() => ids(1)).not.toEqual(expected);
    expected = await ids(1);
    await expect.poll(() => savedIds(1)).toEqual(expected);
    await expect(card(moved)).toBeFocused();
    await drop(3, 0);
    const count = writes.length;
    await hover(1, 1);
    await expect
      .poll(async () => (await rows()).find((i) => i.id === moved).status_id)
      .toBe(statuses[3].id);
    await page.keyboard.press("Escape", { delay: 100 });
    await page.mouse.up();
    await expect.poll(() => ids(3)).toEqual([moved]);
    assert.equal(writes.length, count, "cancel must not save");
    await drop(0, 3);
    await page.screenshot({
      path: path.join(output, `continuous-cross-column-${width}.png`),
    });
    evidence.push({
      label: `${width}px no reload: hover slot -> middle drop -> destination reorder -> empty column -> hover/cancel -> move again`,
      writes: writes.length,
    });
  }
  await writeFile(
    path.join(output, "cross-column-results.json"),
    JSON.stringify({ evidence, writes }, null, 2),
  );
}
