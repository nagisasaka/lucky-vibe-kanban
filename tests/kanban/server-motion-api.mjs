import { expect } from "@playwright/test";
import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import path from "node:path";

/** Real isolated API + production Container/collection, not a mocked stream. */
export async function runServerMotionApi({
  page,
  columns,
  card,
  api,
  project,
  statuses,
  issues,
  output,
  evidence,
  root,
}) {
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const collections =
    "/@fs" + root + "/packages/web-core/src/shared/lib/electric/collections.ts";
  const shape = "/@fs" + root + "/shared/remote-types.ts";
  const refresh = async (status = 200) => {
    const response = page.waitForResponse(
      (r) => r.url().includes("/fallback/issues?") && r.status() === status,
    );
    await page.evaluate(
      async ({ collections, shape, project }) => {
        const c = await import(collections),
          s = await import(shape);
        c.refreshShapeFallback(s.PROJECT_ISSUES_SHAPE, { project_id: project });
      },
      { collections, shape, project },
    );
    await response;
  };
  const rows = async () =>
    (await api(`/api/local/v1/fallback/issues?project_id=${project}`)).issues;
  const patch = async (id, changes) =>
    api(`/api/local/v1/issues/${id}`, "PATCH", changes);
  const ids = async (col) =>
    columns
      .nth(col)
      .locator("[data-kanban-card]")
      .evaluateAll((ns) => ns.map((n) => n.dataset.kanbanCard));
  const settled = async () => {
    await expect(page.locator("[data-kanban-animating]")).toHaveCount(0);
    await expect(page.locator("[data-kanban-server-ghost]")).toHaveCount(0);
  };
  const record = () =>
    page.evaluate(() => {
      cancelAnimationFrame(window.motionFrameId);
      window.motionFrames = [];
      window.recordMotion = true;
      const sample = () => {
        window.motionFrames.push({
          time: performance.now(),
          cards: [
            ...document.querySelectorAll(
              "[data-kanban-motion], [data-kanban-server-ghost]",
            ),
          ].map((n) => {
            const r = n.getBoundingClientRect();
            return {
              id: n.dataset.kanbanMotion ?? n.dataset.kanbanServerGhost,
              ghost: !!n.dataset.kanbanServerGhost,
              x: r.x,
              y: r.y,
              transform: getComputedStyle(n).transform,
            };
          }),
        });
        if (window.recordMotion)
          window.motionFrameId = requestAnimationFrame(sample);
      };
      window.motionFrameId = requestAnimationFrame(sample);
    });
  const saveFrames = async (label, moved, peer) => {
    await settled();
    const frames = await page.evaluate(() => {
      window.recordMotion = false;
      return window.motionFrames;
    });
    for (const id of [moved, peer].filter(Boolean)) {
      const points = frames.flatMap((f) =>
        f.cards
          .filter((c) => c.id === id)
          .map((c) => [Math.round(c.x), Math.round(c.y)].join(",")),
      );
      assert.ok(
        new Set(points).size >= 3,
        `${label}: intermediate positions for ${id}`,
      );
    }
    await writeFile(
      path.join(output, label + "-frames.json"),
      JSON.stringify(frames),
    );
    evidence.push({ label, frames: frames.length, moved, peer });
  };
  await settled();
  await refresh();
  await settled();
  let clientWrites = 0;
  page.on("request", (r) => {
    if (r.method() !== "GET" && /\/v1\/issues(?:\/|$)/.test(r.url()))
      clientWrites++;
  });
  const original = (await rows()).map((r) => [r.id, r.sort_order, r.status_id]);
  for (const width of [360, 600]) {
    await columns.evaluateAll(
      (ns, w) =>
        ns.forEach((n) => {
          n.closest(".kanban-grid-column").parentElement.style.width = w + "px";
          n.closest(".kanban-grid-column").parentElement.style.minWidth =
            w + "px";
        }),
      width,
    );
    await page.waitForTimeout(100);
    const current = await ids(0),
      moved = current[2],
      peer = current[1];
    await patch(moved, { sort_order: width === 360 ? -10 : -20 });
    await record();
    await refresh();
    await expect(
      page.locator("[data-kanban-animating]").first(),
    ).toBeAttached();
    const timing = await page
      .locator("[data-kanban-animating]")
      .first()
      .evaluate((n) => n.getAnimations()[0].effect.getTiming());
    assert.equal(timing.duration, 400);
    assert.equal(timing.easing, "cubic-bezier(0.2, 0, 0, 1)");
    evidence.push({
      label: "automatic motion timing",
      duration: timing.duration,
      easing: timing.easing,
    });
    await page.waitForTimeout(100);
    await page.screenshot({
      path: path.join(output, `same-column-${width}-mid.png`),
    });
    await expect
      .poll(() => ids(0))
      .toEqual([moved, ...current.filter((id) => id !== moved)]);
    await saveFrames(`same-column-${width}`, moved, peer);
  }
  await columns.nth(1).evaluate((n) => {
    n.closest(".kanban-grid-column").parentElement.style.width = "360px";
    n.closest(".kanban-grid-column").parentElement.style.minWidth = "360px";
  });
  await page.waitForTimeout(100);
  // Different-width columns, live focus moves with a reparented real card.
  const moved = (await ids(0))[1],
    peer = (await ids(1))[0];
  await card(moved).focus();
  await patch(moved, { status_id: statuses[1].id, sort_order: -30 });
  await record();
  await refresh();
  const ghost = page.locator(`[data-kanban-server-ghost="${moved}"]`);
  await expect(ghost).toHaveCount(1);
  await expect(ghost.locator("[data-kanban-card]")).toHaveCount(0);
  assert.equal(
    await ghost.evaluate(
      (n) =>
        n.parentElement.hasAttribute("inert") &&
        n.parentElement.getAttribute("aria-hidden") === "true",
    ),
    true,
  );
  await expect(card(moved)).toBeFocused();
  await page.waitForTimeout(100);
  await page.screenshot({ path: path.join(output, "cross-column-mid.png") });
  await saveFrames("cross-column", moved, peer);
  assert.equal(
    (await rows()).find((r) => r.id === moved).status_id,
    statuses[1].id,
  );
  // Empty destination has no fabricated siblings.
  await patch(moved, { status_id: statuses[3].id, sort_order: -40 });
  await record();
  await refresh();
  await expect(
    page.locator(`[data-kanban-server-ghost="${moved}"]`),
  ).toHaveCount(1);
  await saveFrames("empty-column", moved);
  assert.deepEqual(await ids(3), [moved]);
  await expect(card(moved)).toBeFocused();
  await expect(page.locator("[data-kanban-header]").nth(3)).toContainText(
    "(1)",
  );
  // Reduced motion updates final DOM immediately without a ghost.
  await page.emulateMedia({ reducedMotion: "reduce" });
  await patch(moved, { status_id: statuses[0].id, sort_order: -50 });
  await refresh();
  await expect.poll(() => ids(0)).toContain(moved);
  await settled();
  evidence.push({
    label: "reduced-motion: final status, no automatic animations",
  });
  await page.emulateMedia({ reducedMotion: "no-preference" });
  // A content-only edit on the already-first card may promote its rank, but
  // unchanged placement must not animate or send a client ordering write.
  await patch(moved, { description: "Server body saved on first card" });
  await refresh();
  await settled();
  assert.equal(
    clientWrites,
    0,
    "display animation must never persist ranks/status",
  );
  const current = await rows();
  for (const [id, rank, status] of original)
    if (id !== moved && !evidence.some((e) => e.moved === id)) {
      const row = current.find((r) => r.id === id);
      assert.equal(row.sort_order, rank);
      assert.equal(row.status_id, status);
    }
  // Real transport failures and visibility boundaries resume quietly, including
  // a successful recovery whose rows actually differ from the prior layout.
  await page.evaluate(() => {
    window.autoStarts = 0;
    window.motionObserver = new MutationObserver((rs) => {
      window.autoStarts += rs.filter(
        (r) =>
          r.attributeName === "data-kanban-animating" &&
          r.target.hasAttribute("data-kanban-animating"),
      ).length;
    });
    window.motionObserver.observe(document.body, {
      subtree: true,
      attributes: true,
      attributeFilter: ["data-kanban-animating"],
    });
    document.dispatchEvent(new Event("visibilitychange"));
  });
  const recoveryPeer = (await ids(0))[2];
  await patch(recoveryPeer, { sort_order: -80 });
  await refresh();
  await expect.poll(async () => (await ids(0))[0]).toBe(recoveryPeer);
  await settled();
  assert.equal(
    await page.evaluate(() => window.autoStarts),
    0,
    "visibility recovery baseline is quiet",
  );
  await page.route(
    "**/fallback/issues?*",
    (r) =>
      r.fulfill({
        status: 503,
        contentType: "application/json",
        body: '{"message":"isolated transport failure"}',
      }),
    { times: 1 },
  );
  await refresh(503);
  await patch(moved, { sort_order: -90 });
  await refresh();
  await expect.poll(async () => (await ids(0))[0]).toBe(moved);
  await settled();
  assert.equal(
    await page.evaluate(() => window.autoStarts),
    0,
    "communication recovery baseline is quiet",
  );
  await patch(recoveryPeer, { sort_order: -100 });
  await refresh();
  await expect(page.locator("[data-kanban-animating]").first()).toBeAttached();
  await settled();
  evidence.push({
    label:
      "real snapshot failure/recovery and visibility event boundary: no history replay, next change animates",
  });
  // Canonical updates during keyboard dragging remain deferred until cancel.
  const frozen = await ids(0),
    dragId = frozen[1],
    promoted = frozen[2];
  await card(dragId).focus();
  await page.keyboard.press("Space");
  await expect(card(dragId)).toHaveAttribute("aria-pressed", "true");
  await patch(promoted, { sort_order: -110 });
  await refresh();
  await page.waitForTimeout(80);
  assert.deepEqual(await ids(0), frozen);
  await page.keyboard.press("Escape");
  await expect.poll(async () => (await ids(0))[0]).toBe(promoted);
  await settled();
  await expect(card(dragId)).toBeFocused();
  evidence.push({
    label:
      "real API rank update during keyboard drag deferred until cancel; latest canonical order and focus retained",
  });
  // Actual explicit sort UI and reload establish quiet baselines.
  await page.getByRole("button", { name: "Filters", exact: true }).click();
  await page.getByRole("button", { name: "Sort: Manual", exact: true }).click();
  await page.getByRole("menuitem", { name: "Title", exact: true }).click();
  await page.keyboard.press("Escape");
  await settled();
  const before = (await rows()).find((r) => r.id === moved).sort_order;
  await patch(moved, { description: "Explicit sort keeps the stored rank" });
  await refresh();
  await settled();
  assert.equal((await rows()).find((r) => r.id === moved).sort_order, before);
  await page.reload();
  await expect(card(moved)).toBeVisible();
  await settled();
  await page.evaluate(() => {
    window.autoStarts = 0;
    const observer = new MutationObserver((records) => {
      window.autoStarts += records.filter((record) =>
        record.target.hasAttribute("data-kanban-animating"),
      ).length;
    });
    observer.observe(document.body, {
      subtree: true,
      attributes: true,
      attributeFilter: ["data-kanban-animating"],
    });
  });
  await page
    .getByPlaceholder("Search issues...", { exact: true })
    .fill("Card 0-");
  await expect(page.locator("[data-kanban-card]")).toHaveCount(10);
  await settled();
  await page.getByPlaceholder("Search issues...", { exact: true }).fill("");
  await expect(page.locator("[data-kanban-card]")).toHaveCount(30);
  await settled();
  await page.getByRole("button", { name: "Personal", exact: true }).click();
  await page.evaluate(
    () =>
      new Promise((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(resolve)),
      ),
  );
  await settled();
  await page.getByRole("button", { name: "Team", exact: true }).click();
  await expect(page.locator("[data-kanban-card]")).toHaveCount(30);
  await settled();
  assert.equal(
    await page.evaluate(() => window.autoStarts),
    0,
    "filter and view changes never replay placement history",
  );
  evidence.push({
    label:
      "real search filter exit/entry and Personal/Team switches: quiet placement baselines",
  });
  assert.deepEqual(errors, []);
  evidence.push({
    label:
      "real API rank/status, counts, explicit-sort promotion suppression, body-only placement, focus, reload, no client order writes verified",
  });
}
