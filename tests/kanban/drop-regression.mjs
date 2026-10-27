import { expect } from "@playwright/test";
import assert from "node:assert/strict";
import path from "node:path";
import { writeFile } from "node:fs/promises";
export async function runDropRegression({
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
  console.log("DnD regression: browser ready");
  page.on("request", (r) => {
    if (r.url().includes("/issues/bulk")) console.log("BULK", r.postData());
  });
  page.on("response", (r) => {
    if (r.url().includes("/issues/bulk"))
      console.log("BULK RESPONSE", r.status());
  });
  const pause = (ms) => new Promise((r) => setTimeout(r, ms));
  const ids = (column = 0) =>
    columns
      .nth(column)
      .locator("[data-kanban-card]")
      .evaluateAll((cards) => cards.map((c) => c.dataset.kanbanCard));
  const saved = async () =>
    (await api(`/api/local/v1/fallback/issues?project_id=${project}`)).issues;
  const savedIds = async (column = 0) =>
    (await saved())
      .filter((i) => i.status_id === statuses[column].id)
      .sort((a, b) => a.sort_order - b.sort_order)
      .map((i) => i.id);
  const keyDrop = async (id, key) => {
    await card(id).focus();
    await page.keyboard.press("Space", { delay: 100 });
    await expect(card(id)).toHaveAttribute("aria-pressed", "true");
    await page.keyboard.press(key, { delay: 100 });
    await page.keyboard.press("Space", { delay: 100 });
  };
  const settled = async (column = 0) => {
    const actual = await savedIds(column);
    await expect.poll(() => ids(column), { timeout: 20000 }).toEqual(actual);
    return actual;
  };
  if (process.env.LVK_DND_BASELINE !== "1") {
    await page.route("**/api/workspaces/summaries", async (route) => {
      const response = await route.fetch();
      const json = await response.json();
      json.data.summaries = json.data.summaries.map((w, i) => ({
        ...w,
        is_running: i === 0,
        latest_process_status: i === 0 ? "running" : w.latest_process_status,
      }));
      await route.fulfill({ response, json });
    });
    await page.reload();
    await expect(card(issues[0].id)).toBeVisible();
    await expect(
      card(issues[0].id)
        .getByRole("img", { name: "Running", exact: true })
        .first(),
    ).toBeVisible();
    evidence.push({
      label:
        "Running workspace summary fixture displayed during real browser drag; no provider execution",
    });
  }
  if (process.env.LVK_UI4_CONTINUOUS === "1") {
    const { continuousTouchPrefix } = await import('./ui4-continuous.mjs');
    await continuousTouchPrefix({ page, card, columns, issues, api, project, output, evidence });
  }
  let order = await ids();
  const moved = order[0];
  if (process.env.LVK_DND_BASELINE === "1") {
    const from = await card(moved).boundingBox(),
      to = await columns.nth(3).boundingBox();
    await page.mouse.move(from.x + 30, from.y + 20);
    await page.mouse.down();
    await page.mouse.move(from.x + 45, from.y + 20, { steps: 5 });
    await page.mouse.move(to.x + 40, to.y + 40, { steps: 20 });
    await page.mouse.up();
    await expect(
      columns.nth(3).locator(`[data-kanban-card="${moved}"]`),
    ).toHaveCount(1);
    await expect
      .poll(async () => (await saved()).find((i) => i.id === moved).status_id)
      .toBe(statuses[3].id);
    await expect.poll(() => ids()).toEqual(order);
    evidence.push({
      label:
        "BASELINE reproduction: successful save to Done, then fixed 500ms release restores stale original column",
      moved,
    });
    await page.screenshot({
      path: path.join(output, "baseline-stale-return.png"),
    });
    return;
  }

  // Slow both save and committed-state confirmation; observe every frame around the old 500ms release.
  let writes = 0;
  const samples = [];
  await page.route("**/api/local/v1/issues/bulk", async (route) => {
    writes++;
    await pause(900);
    await route.continue();
  });
  await page.route("**/api/local/v1/fallback/issues?*", async (route) => {
    await pause(1100);
    await route.continue();
  });
  await keyDrop(moved, "ArrowRight");
  const first = [order[1], order[0], ...order.slice(2)];
  await expect.poll(() => ids()).toEqual(first);
  for (let i = 0; i < 30; i++) {
    samples.push(await ids());
    assert.deepEqual(
      samples.at(-1),
      first,
      "optimism must survive old snapshot/500ms",
    );
    await pause(100);
  }
  await settled();
  console.log("DnD checkpoint", evidence.length);
  evidence.push({
    label: "slow real API save/confirmation: no return to original position",
    writes,
    samples,
  });
  // Two drops before first response. Second is based on the optimistic arrangement.
  await keyDrop(moved, "ArrowDown");
  const second = await ids();
  await keyDrop(moved, "ArrowLeft");
  const latest = await ids();
  assert.notDeepEqual(latest, second);
  for (let i = 0; i < 45; i++) {
    assert.deepEqual(
      await ids(),
      latest,
      "old response must not rewind newer drop",
    );
    await pause(100);
  }
  await expect.poll(() => savedIds(), { timeout: 20000 }).toEqual(latest);
  await settled();
  console.log("DnD checkpoint", evidence.length);
  evidence.push({
    label:
      "rapid same-card keyboard drops serialize and preserve newest position",
    latest,
  });
  await page.unroute("**/api/local/v1/issues/bulk");
  await page.unroute("**/api/local/v1/fallback/issues?*");
  // Pointer cross-column move, including counts and persistence.
  const beforeCount = (await ids(3)).length;
  const from = await card(moved).boundingBox(),
    to = await columns.nth(3).boundingBox();
  await page.mouse.move(from.x + 30, from.y + 20);
  await page.mouse.down();
  await page.mouse.move(from.x + 45, from.y + 20, { steps: 5 });
  await page.mouse.move(to.x + 40, to.y + 40, { steps: 20 });
  await page.mouse.up();
  await expect(
    columns.nth(3).locator(`[data-kanban-card="${moved}"]`),
  ).toHaveCount(1);
  await expect
    .poll(async () => (await saved()).find((i) => i.id === moved).status_id)
    .toBe(statuses[3].id);
  await settled(3);
  assert.equal((await ids(3)).length, beforeCount + 1);
  for (let column = 0; column < 4; column++) {
    const count = (await ids(column)).length;
    await expect(
      page
        .locator("span[title]")
        .filter({ hasText: statuses[column].name })
        .locator(".."),
    ).toContainText(`(${count})`);
  }
  // Do not reload: subsequent drag metadata must use the new container/rank.
  await keyDrop(moved, "ArrowLeft");
  await expect(
    columns.nth(2).locator(`[data-kanban-card="${moved}"]`),
  ).toHaveCount(1);
  await expect
    .poll(async () => (await saved()).find((i) => i.id === moved).status_id)
    .toBe(statuses[2].id);
  await settled(2);
  await expect(card(moved)).toBeFocused();
  const beforeInDestination = await ids(2);
  const destinationIndex = beforeInDestination.indexOf(moved);
  await keyDrop(moved, destinationIndex % 2 === 0 ? "ArrowRight" : "ArrowLeft");
  const afterInDestination = await ids(2);
  assert.notDeepEqual(afterInDestination, beforeInDestination);
  await expect.poll(() => savedIds(2)).toEqual(afterInDestination);
  await settled(2);
  await expect(card(moved)).toBeFocused();
  // Use a pointer for an unambiguous next column.
  const followFrom = await card(moved).boundingBox();
  const followTo = await columns.nth(3).boundingBox();
  await page.mouse.move(followFrom.x + 30, followFrom.y + 20);
  await page.mouse.down();
  await page.mouse.move(followFrom.x + 45, followFrom.y + 20, { steps: 4 });
  await page.mouse.move(followTo.x + 40, followTo.y + 40, { steps: 15 });
  await page.mouse.up();
  await expect(
    columns.nth(3).locator(`[data-kanban-card="${moved}"]`),
  ).toHaveCount(1);
  await expect
    .poll(async () => (await saved()).find((i) => i.id === moved).status_id)
    .toBe(statuses[3].id);
  await settled(3);
  evidence.push({
    label:
      "without reload: cross-column -> destination reorder -> cross-column again uses current container/rank and persists",
  });
  await page.screenshot({
    path: path.join(output, "continuous-no-reload.png"),
  });
  await page.reload();
  await expect(card(moved)).toBeVisible();
  await settled(3);
  console.log("DnD checkpoint", evidence.length);
  evidence.push({
    label:
      "pointer cross-column move persisted after reload; column membership/count updated",
  });
  // Pointer same-column reordering uses the production hit testing and real decimal ranks.
  order = await ids();
  const pointerSource = await card(order[0]).boundingBox();
  const pointerTarget = await card(order[3]).boundingBox();
  await page.mouse.move(pointerSource.x + 30, pointerSource.y + 20);
  await page.mouse.down();
  await page.mouse.move(pointerSource.x + 45, pointerSource.y + 20, {
    steps: 4,
  });
  await page.mouse.move(pointerTarget.x + 30, pointerTarget.y + 20, {
    steps: 15,
  });
  await page.mouse.up();
  const pointerOrder = [...order.slice(1, 4), order[0], ...order.slice(4)];
  await expect.poll(() => ids()).toEqual(pointerOrder);
  await expect.poll(() => savedIds()).toEqual(pointerOrder);
  await settled();
  evidence.push({
    label: "pointer same-column decimal reorder persisted through real API",
  });
  // Activity arriving during drag: real description PATCH promotes while frozen.
  order = await ids();
  const active = order[2];
  await card(active).focus();
  await page.keyboard.press("Space", { delay: 100 });
  await api(`/api/local/v1/issues/${active}`, "PATCH", {
    description: "Activity during drag",
  });
  await page.keyboard.press("ArrowRight", { delay: 100 });
  await page.keyboard.press("Space", { delay: 100 });
  await expect(page.getByRole("alert")).toContainText("Newer activity");
  await settled();
  assert.equal((await ids())[0], active);
  console.log("DnD checkpoint", evidence.length);
  evidence.push({
    label:
      "real activity promotion during drag: successful guarded bulk response, conflict notice, canonical head preserved",
  });
  await page.screenshot({ path: path.join(output, "activity-conflict.png") });
  // Save failure must notify and visibly reconcile, not silently disappear.
  await page.getByRole("button", { name: "Dismiss", exact: true }).click();
  await page.route(
    "**/api/local/v1/issues/bulk",
    (route) =>
      route.fulfill({
        status: 500,
        json: { message: "isolated save failure" },
      }),
    { times: 1 },
  );
  await keyDrop(active, "ArrowRight");
  await expect(page.getByRole("alert")).toContainText("Could not save");
  await settled();
  await page.screenshot({ path: path.join(output, "save-failure.png") });
  console.log("DnD checkpoint", evidence.length);
  evidence.push({
    label:
      "injected save HTTP 500: alert and resync; actual browser interaction",
  });
  await page.getByRole("button", { name: "Dismiss", exact: true }).click();
  // A saved drop with unavailable confirmation remains optimistic until explicit recovery.
  await page.route("**/api/local/v1/fallback/issues?*", (route) =>
    route.fulfill({ status: 503, json: { message: "isolated sync failure" } }),
  );
  await keyDrop(active, "ArrowRight");
  await expect(page.getByRole("alert")).toContainText("synchronization");
  const optimistic = await ids();
  await pause(1000);
  assert.deepEqual(await ids(), optimistic);
  await page.screenshot({ path: path.join(output, "sync-failure.png") });
  await page.unroute("**/api/local/v1/fallback/issues?*");
  await page
    .getByRole("button", { name: "Refresh board", exact: true })
    .click();
  await settled();
  console.log("DnD checkpoint", evidence.length);
  evidence.push({
    label:
      "injected sync HTTP 503 differs from save failure; preserves optimism; refresh recovers without repeat save",
  });
  // A confirmation request that exceeds the bounded wait gives a sync notice, then recovers.
  await page.getByRole("button", { name: "Dismiss", exact: true }).click();
  await page.route("**/api/local/v1/fallback/issues?*", async (route) => {
    await pause(17000);
    await route
      .fulfill({ status: 503, json: { message: "delayed confirmation" } })
      .catch(() => {});
  });
  await keyDrop(active, "ArrowDown");
  await expect(page.getByRole("alert")).toContainText("synchronization", {
    timeout: 20000,
  });
  await page.screenshot({ path: path.join(output, "sync-timeout.png") });
  await pause(2500);
  await page.unroute("**/api/local/v1/fallback/issues?*");
  await page
    .getByRole("button", { name: "Refresh board", exact: true })
    .click();
  await settled();
  evidence.push({
    label:
      "15s synchronization timeout: notice and explicit reconciliation without resubmitting move",
  });
  // Title sorting forbids within-column manual reorder but permits a status move.
  await page.getByRole("button", { name: "Dismiss", exact: true }).click();
  await page.getByRole("button", { name: "Filters", exact: true }).click();
  await page.getByRole("button", { name: "Sort: Manual", exact: true }).click();
  await page.getByRole("menuitem", { name: "Title", exact: true }).click();
  await page.keyboard.press("Escape");
  const titleSorted = (await saved())
    .filter((i) => i.status_id === statuses[0].id)
    .sort((a, b) => a.title.localeCompare(b.title))
    .map((i) => i.id);
  await expect.poll(() => ids(), { timeout: 20000 }).toEqual(titleSorted);
  const titleOrder = await ids();
  let nonManualWrites = 0;
  const countWrites = (r) => {
    if (r.url().includes("/issues/bulk")) nonManualWrites++;
  };
  page.on("request", countWrites);
  await keyDrop(titleOrder[0], "ArrowRight");
  await pause(500);
  assert.deepEqual(await ids(), titleOrder);
  assert.equal(nonManualWrites, 0);
  await keyDrop(moved, "ArrowLeft");
  await expect(
    columns.nth(2).locator(`[data-kanban-card="${moved}"]`),
  ).toHaveCount(1);
  await expect
    .poll(async () => (await saved()).find((i) => i.id === moved).status_id)
    .toBe(statuses[2].id);
  page.off("request", countWrites);
  evidence.push({
    label:
      "non-manual Title sort: intra-column reorder blocked, keyboard status move allowed",
  });
  await writeFile(
    path.join(output, "drop-results.json"),
    JSON.stringify(evidence, null, 2),
  );
}
