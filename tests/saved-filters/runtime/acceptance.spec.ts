// Opt-in integration suite. Uses only the explicitly handed-off isolated instance.
// Never run against shared dev; never activate a natural-language definition.
import {
  test,
  expect,
  type Page,
  type APIRequestContext,
  type Request,
} from "@playwright/test";
import { readFileSync, mkdirSync } from "node:fs";
import { resolve } from "node:path";
import { randomUUID } from "node:crypto";
const handoffPath = process.env.LOCAL460_HANDOFF;
if (!handoffPath)
  throw new Error("Set LOCAL460_HANDOFF to the isolated runtime handoff");
const h = JSON.parse(readFileSync(handoffPath, "utf8"));
const base = h.backend_url;
if (!/^http:\/\/127\.0\.0\.1:\d+$/.test(base) || new URL(base).port === "4020")
  throw new Error("Isolated loopback runtime required");
const project = h.projects.machine.id;
const board = `${base}/projects/${project}`;
const filterBase = `/api/projects/${project}/saved-filters`;
const out = resolve(".build/saved-filters/runtime");
mkdirSync(out, { recursive: true });
const machine = (patch = {}) => ({
  search: null,
  simple_id: null,
  status_ids: [],
  priorities: [],
  assignee_ids: [],
  include_unassigned: false,
  tag_ids: [],
  tag_mode: "any",
  archived: "active",
  created_at: null,
  updated_at: null,
  relation: null,
  ...patch,
});
async function api(
  request: APIRequestContext,
  path: string,
  method = "GET",
  data?: unknown,
) {
  if (path.includes(h.projects.semantic.id))
    throw new Error("Semantic project is forbidden");
  if (path.endsWith("/evaluate")) {
    const definition = await api(request, path.slice(0, -9));
    if (definition.natural_language?.trim())
      throw new Error("Provider launch is forbidden");
  }
  const response = await request.fetch(base + path, {
    method,
    data,
    headers: { "x-lvk-csrf": "local" },
  });
  expect(response.ok(), `${method} ${path}: ${response.status()}`).toBeTruthy();
  if (response.status() === 204) return null;
  const body = await response.json();
  return body.data ?? body;
}
async function issues(request: APIRequestContext) {
  return (
    await api(request, `/api/local/v1/fallback/issues?project_id=${project}`)
  ).issues as any[];
}
async function createFilter(
  request: APIRequestContext,
  patch = {},
  context: string | null = null,
) {
  return api(request, filterBase, "POST", {
    request_id: randomUUID(),
    definition: {
      name: "UI runtime " + randomUUID().slice(0, 8),
      machine: machine(patch),
      natural_language: null,
      context,
    },
  });
}
async function updateFilter(
  request: APIRequestContext,
  filter: any,
  patch: any,
) {
  const current = await api(request, `${filterBase}/${filter.id}`);
  return api(request, `${filterBase}/${filter.id}`, "PUT", {
    request_id: randomUUID(),
    expected_revision: current.revision,
    definition: {
      name: current.name,
      machine: current.machine,
      natural_language: current.natural_language,
      context: current.context,
      ...patch,
    },
  });
}
async function createIssue(request: APIRequestContext, patch = {}) {
  const r = await api(request, "/api/local/v1/issues", "POST", {
    project_id: project,
    status_id: h.statuses.machine[0].id,
    title: "UI runtime " + randomUUID().slice(0, 8),
    description: "runtime fixture",
    priority: "medium",
    sort_order: 100,
    extension_metadata: {},
    ...patch,
  });
  return r.issue ?? r;
}
async function patchIssue(request: APIRequestContext, issue: any, patch: any) {
  const current = (await issues(request)).find((i) => i.id === issue.id);
  return api(request, `/api/local/v1/issues/${issue.id}`, "PATCH", {
    expected_updated_at: current.updated_at,
    ...patch,
  });
}
async function evaluated(request: APIRequestContext, id: string) {
  const value = await api(request, `${filterBase}/${id}/evaluate`, "POST", {
    offset: 0,
    limit: 500,
    retry_failed: false,
  });
  expect(value.state).toBe("complete");
  expect(value.next_offset).toBeNull();
  return value.issue_ids.sort();
}
const cards = (page: Page) =>
  page.locator("[data-kanban-card], [data-issue-list-row]");
async function cardIds(page: Page) {
  return cards(page).evaluateAll((nodes) =>
    nodes
      .map(
        (n) =>
          n.getAttribute("data-kanban-card") ??
          n.getAttribute("data-issue-list-row"),
      )
      .sort(),
  );
}
async function match(page: Page, request: APIRequestContext, id: string) {
  const expected = await evaluated(request, id);
  await expect.poll(() => cardIds(page)).toEqual(expected);
  await expect(
    page.getByRole("status").filter({ hasText: "評価完了" }),
  ).toContainText(`表示中 ${expected.length} 件`);
  return expected;
}
async function select(page: Page, id: string) {
  await page
    .getByRole("combobox", { name: "保存フィルター", exact: true })
    .selectOption(id);
}
async function capture(page: Page, name: string) {
  await page.screenshot({ path: `${out}/${name}.png`, fullPage: true });
}
async function verifyBulk(
  page: Page,
  request: APIRequestContext,
  selected: string[],
  priority: "low" | "medium" = "low",
) {
  const before = await issues(request);
  // The bulk UI currently sends one PATCH per Issue through updateIssue.
  // Observe either supported transport; assert the union, not a presumed batch shape.
  const sent: any[] = [];
  const observe = (r: Request) => {
    const path = new URL(r.url()).pathname;
    if (path === "/api/local/v1/issues/bulk" && r.method() === "POST")
      sent.push(...r.postDataJSON().updates);
    else if (
      /^\/api\/local\/v1\/issues\/[^/]+$/.test(path) &&
      r.method() === "PATCH"
    )
      sent.push({ id: path.split("/").pop(), ...r.postDataJSON() });
  };
  page.on("request", observe);
  try {
    await page
      .getByRole("button", { name: "優先度", exact: true })
      .last()
      .click();
    await page
      .getByRole("option", {
        name: priority === "low" ? "低" : "中",
        exact: true,
      })
      .click();
    await expect.poll(() => sent.map((i) => i.id).sort()).toEqual(selected);
    expect(sent.every((i) => i.priority === priority)).toBe(true);
    await expect
      .poll(async () =>
        (await issues(request))
          .filter((i) => selected.includes(i.id))
          .map((i) => i.priority),
      )
      .toEqual([priority, priority]);
    expect(
      (await issues(request)).find((i) => i.id === h.issues.other_assignee.id)
        .priority,
    ).toBe(before.find((i) => i.id === h.issues.other_assignee.id).priority);
  } finally {
    page.off("request", observe);
    await api(request, "/api/local/v1/issues/bulk", "POST", {
      updates: selected.map((id) => ({
        id,
        priority: before.find((i) => i.id === id).priority,
      })),
    });
  }
}
test.beforeEach(async ({ page }) => {
  await page.route("**/saved-filters/*/evaluate", async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (!path.startsWith(filterBase + "/"))
      throw new Error("Non-machine project activation blocked");
    const response = await page.request.get(base + path.slice(0, -9));
    // Deleted/foreign IDs must reach the backend's real 404, without a provider.
    if (response.status() === 404) return route.continue();
    expect(response.ok()).toBeTruthy();
    const definition = (await response.json()).data;
    if (definition.natural_language?.trim()) {
      await route.abort();
      throw new Error("Natural-language activation blocked");
    }
    await route.continue();
  });
});

test("actual settings CRUD, saved text/context, stable links and invalid links", async ({
  page,
  request,
}) => {
  await page.goto(board);
  await page
    .getByRole("button", { name: "フィルターを管理", exact: true })
    .click();
  await page
    .getByRole("button", { name: "フィルターを作成", exact: true })
    .click();
  const name = "UI CRUD " + randomUUID().slice(0, 8);
  await page.getByLabel("名前", { exact: true }).fill(name);
  await page.getByRole("textbox", { name: /^Issue ID/ }).fill("mech-1");
  await page
    .getByRole("textbox", { name: "自然文の条件", exact: true })
    .fill("保存だけを確認。評価は起動しない");
  await page
    .getByRole("textbox", { name: /^対象・補足文脈/ })
    .fill("context preserved v1");
  const saved = page.waitForResponse(
    (r) => r.url() === base + filterBase && r.request().method() === "POST",
  );
  await page
    .getByRole("button", { name: "フィルターを保存", exact: true })
    .click();
  const filter = (await (await saved).json()).data;
  expect(filter.natural_language).toBe("保存だけを確認。評価は起動しない");
  await page.getByRole("button", { name, exact: true }).click();
  await expect(
    page.getByRole("textbox", { name: /^対象・補足文脈/ }),
  ).toHaveValue("context preserved v1");
  await expect(
    page.getByRole("textbox", { name: "自然文の条件", exact: true }),
  ).toHaveValue(filter.natural_language);
  // Clear NL before any link/application; persisted context alone does not start AI.
  await page
    .getByRole("textbox", { name: "自然文の条件", exact: true })
    .fill("");
  await page.getByLabel("名前", { exact: true }).fill(name + " renamed");
  await page.getByRole("textbox", { name: /^Issue ID/ }).fill("mech-2");
  await page
    .getByRole("textbox", { name: /^対象・補足文脈/ })
    .fill("context preserved v2");
  const edited = page.waitForResponse(
    (r) =>
      r.url() === base + filterBase + "/" + filter.id &&
      r.request().method() === "PUT",
  );
  await page
    .getByRole("button", { name: "フィルターを保存", exact: true })
    .click();
  expect((await (await edited).json()).data.id).toBe(filter.id);
  await capture(page, "settings-desktop-light-bottom");
  await page
    .getByRole("textbox", { name: "名前", exact: true })
    .scrollIntoViewIfNeeded();
  await capture(page, "settings-desktop-light-top");
  await page.goto(board + "?savedFilter=" + filter.id);
  expect(await match(page, request, filter.id)).toEqual([
    h.issues.unassigned.id,
  ]);
  await expect(
    page.getByRole("combobox", { name: "保存フィルター", exact: true }),
  ).toHaveValue(filter.id);
  await page
    .getByRole("button", { name: "フィルターを管理", exact: true })
    .click();
  await page
    .getByRole("button", { name: name + " renamed", exact: true })
    .click();
  await page
    .getByRole("button", { name: "フィルターを削除", exact: true })
    .click();
  await page
    .getByRole("dialog", { name: "フィルターを削除", exact: true })
    .getByRole("button", { name: "フィルターを削除", exact: true })
    .click();
  const foreign = await api(
    request,
    `/api/projects/${h.projects.other.id}/saved-filters`,
    "POST",
    {
      request_id: randomUUID(),
      definition: {
        name: "foreign UI fixture",
        machine: machine(),
        natural_language: null,
        context: null,
      },
    },
  );
  for (const id of [filter.id, "invalid-id", foreign.id]) {
    await page.goto(board + "?savedFilter=" + id);
    await expect(
      page.getByRole("alert").filter({ hasText: "適用できません" }),
    ).toBeVisible();
    expect(await cardIds(page)).toEqual([]);
    await select(page, "__none__");
    await expect(
      page.getByRole("combobox", { name: "保存フィルター", exact: true }),
    ).toHaveValue("__none__");
    await expect(
      page.getByRole("textbox", { name: "Issueを検索…", exact: true }),
    ).toBeVisible();
  }
});

test("real board overrides transient filters with all evaluated IDs", async ({
  page,
  request,
}) => {
  const hidden = await createIssue(request, {
    status_id: h.statuses.machine.find((s: any) => s.hidden).id,
    title: "UI hidden match",
  });
  const broad = await createFilter(request, { archived: "all" });
  await page.goto(board);
  const clear = page.getByRole("button", {
    name: "絞り込みを解除",
    exact: true,
  });
  if (await clear.isVisible()) await clear.click();
  const kanban = page.getByRole("button", {
    name: "カンバン表示に切り替え",
    exact: true,
  });
  if (await kanban.isVisible()) await kanban.click();
  await page.getByRole("button", { name: "個人", exact: true }).click();
  await page
    .getByRole("textbox", { name: "Issueを検索…", exact: true })
    .fill("no-such-runtime-issue");
  await page.getByRole("button", { name: "アーカイブ済", exact: true }).click();
  await page.getByRole("button", { name: "絞り込み", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "絞り込み", exact: true });
  const sub = dialog
    .locator("div")
    .filter({ has: page.getByText("サブIssue", { exact: true }) })
    .filter({ has: page.getByRole("switch") })
    .last()
    .getByRole("switch");
  await sub.uncheck();
  await dialog
    .locator("div")
    .filter({ has: page.getByText("ブロック中を非表示", { exact: true }) })
    .filter({ has: page.getByRole("switch") })
    .last()
    .getByRole("switch")
    .check();
  await dialog.getByRole("button", { name: /^優先度/ }).click();
  await page
    .getByRole("menuitemcheckbox", { name: "Low", exact: true })
    .check();
  await page.keyboard.press("Escape");
  // Wait for the nested menu's exit animation before closing its parent dialog.
  await expect(page.getByRole("menu", { includeHidden: true })).toHaveCount(0);
  if (await dialog.isVisible())
    await dialog.getByRole("button", { name: "閉じる", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(page.locator("body")).not.toHaveCSS("pointer-events", "none");
  await select(page, broad.id);
  const ids = await match(page, request, broad.id);
  for (const id of [
    hidden.id,
    h.issues.child.id,
    h.issues.archived.id,
    h.issues.unassigned.id,
  ])
    expect(ids).toContain(id);
  await expect(page.getByText(/一時検索・Team/)).toBeVisible();
  await capture(page, "board-desktop-light");
});

test("real selection prune, reversible bulk updates and list view", async ({
  page,
  request,
}) => {
  const broad = await createFilter(request, { archived: "all" });
  const narrow = await createFilter(
    request,
    h.filters.assigned_or_none.machine,
  );
  await page.goto(board);
  const kanban = page.getByRole("button", {
    name: "カンバン表示に切り替え",
    exact: true,
  });
  if (await kanban.isVisible()) await kanban.click();
  await select(page, broad.id);
  const ids = await match(page, request, broad.id);
  // Dragging is disabled after the second selection, which adds aria-disabled
  // to the draggable card. Keyboard selection remains an actual supported UI flow.
  for (const issue of [
    h.issues.assigned,
    h.issues.unassigned,
    h.issues.other_assignee,
  ]) {
    const card = page.locator(`[data-kanban-card="${issue.id}"]`);
    await card.focus();
    await card.press("Control+Enter");
    await expect(card).toHaveClass(/ring-accent/);
  }
  await expect(page.getByText("3件を選択中", { exact: true })).toBeVisible();
  await select(page, narrow.id);
  await match(page, request, narrow.id);
  // Switching through an unknown result clears all selection, including visible survivors.
  await expect(page.getByText(/件を選択中$/)).toHaveCount(0);
  for (const issue of [h.issues.assigned, h.issues.unassigned]) {
    const card = page.locator(`[data-kanban-card="${issue.id}"]`);
    await card.focus();
    await card.press("Control+Enter");
    await expect(card).toHaveClass(/ring-accent/);
  }
  await expect(page.getByText("2件を選択中", { exact: true })).toBeVisible();
  await verifyBulk(
    page,
    request,
    [h.issues.assigned.id, h.issues.unassigned.id].sort(),
  );
  await select(page, "__none__");
  await page
    .getByRole("button", { name: "一覧表示に切り替え", exact: true })
    .click();
  await select(page, broad.id);
  await match(page, request, broad.id);
  await expect(page.locator("[data-issue-list-row]")).toHaveCount(ids.length);
  await capture(page, "board-list-light");
  await select(page, narrow.id);
  await match(page, request, narrow.id);
  for (const issue of [h.issues.assigned, h.issues.unassigned]) {
    const row = page.locator(`[data-issue-list-row="${issue.id}"]`);
    await row.focus();
    await row.press("Control+Enter");
  }
  await expect(page.getByText("2件を選択中", { exact: true })).toBeVisible();
  await capture(page, "board-list-bulk-light");
  await verifyBulk(
    page,
    request,
    [h.issues.assigned.id, h.issues.unassigned.id].sort(),
    "medium",
  );
});

test("active machine view follows new issues, body/status/tag/parent and definition/context changes, then time expiry", async ({
  page,
  request,
}) => {
  const marker = "evidence-" + randomUUID().slice(0, 8);
  const filter = await createFilter(request, {
    search: marker,
    status_ids: [h.statuses.machine[0].id],
    tag_ids: [h.tags[0].id],
    relation: { kind: "children", target_issue_id: h.issues.assigned.id },
  });
  await page.goto(board + "?savedFilter=" + filter.id);
  expect(await match(page, request, filter.id)).toEqual([]);
  const issue = await createIssue(request, {
    description: marker,
    parent_issue_id: h.issues.assigned.id,
  });
  const tag = await api(request, "/api/local/v1/issue_tags", "POST", {
    issue_id: issue.id,
    tag_id: h.tags[0].id,
  });
  await expect.poll(() => cardIds(page)).toEqual([issue.id]);
  await patchIssue(request, issue, { description: "not matching now" });
  await expect.poll(() => cardIds(page)).toEqual([]);
  await patchIssue(request, issue, { description: marker });
  await expect.poll(() => cardIds(page)).toEqual([issue.id]);
  await patchIssue(request, issue, { status_id: h.statuses.machine[1].id });
  await expect.poll(() => cardIds(page)).toEqual([]);
  await patchIssue(request, issue, { status_id: h.statuses.machine[0].id });
  await expect.poll(() => cardIds(page)).toEqual([issue.id]);
  await api(
    request,
    "/api/local/v1/issue_tags/" + (tag.issue_tag ?? tag).id,
    "DELETE",
  );
  await expect.poll(() => cardIds(page)).toEqual([]);
  await api(request, "/api/local/v1/issue_tags", "POST", {
    issue_id: issue.id,
    tag_id: h.tags[0].id,
  });
  await expect.poll(() => cardIds(page)).toEqual([issue.id]);
  await patchIssue(request, issue, { parent_issue_id: null });
  await expect.poll(() => cardIds(page)).toEqual([]);
  await patchIssue(request, issue, { parent_issue_id: h.issues.assigned.id });
  await expect.poll(() => cardIds(page)).toEqual([issue.id]);
  const changed = await updateFilter(request, filter, {
    context: "new shared context",
    name: "Updated active definition",
    machine: machine({ simple_id: issue.simple_id }),
  });
  await page.getByText(/条件を確認/).click();
  await expect(
    page.getByRole("textbox", { name: /^対象・補足文脈/ }),
  ).toHaveValue("new shared context");
  await expect
    .poll(
      async () =>
        (await api(request, `${filterBase}/${filter.id}/results`))
          .definition_revision,
    )
    .toBe(changed.revision);
  await updateFilter(request, filter, {
    machine: machine({ search: "absent-" + marker }),
  });
  await expect.poll(() => cardIds(page)).toEqual([]);
  await expect(
    page.getByRole("status").filter({ hasText: "評価完了" }),
  ).toContainText("表示中 0 件");
  const timeFilter = await createFilter(request, {
    created_at: { kind: "relative", seconds: 12 },
    search: marker,
  });
  const timed = await createIssue(request, { title: marker });
  await page.goto(board + "?savedFilter=" + timeFilter.id);
  await expect.poll(() => cardIds(page)).toEqual([timed.id]);
  await expect.poll(() => cardIds(page), { timeout: 35000 }).toEqual([]);
  expect(await evaluated(request, timeFilter.id)).toEqual([]);
});

test("full board injected network pending/failure/stale, and mobile dark visual", async ({
  page,
  request,
}) => {
  const filter = await createFilter(request, {
    simple_id: h.issues.assigned.simple_id,
  });
  let mode = "delay";
  let release: (() => void) | undefined;
  // Only network timing/failure is injected; successful responses come from real backend.
  await page.route(`**/saved-filters/${filter.id}/evaluate`, async (route) => {
    if (mode === "delay")
      await new Promise<void>((resolve) => {
        release = resolve;
      });
    if (mode === "fail") return route.abort("failed");
    await route.continue();
  });
  await page.goto(board + "?savedFilter=" + filter.id);
  await expect(
    page.getByText("評価結果はまだありません。該当0件ではありません。"),
  ).toBeVisible();
  expect(await cardIds(page)).toEqual([]);
  await capture(page, "board-network-pending");
  mode = "fail";
  release!();
  await expect(
    page.getByRole("alert").filter({ hasText: "評価失敗" }),
  ).toContainText("該当0件ではありません");
  mode = "ok";
  await page.getByRole("button", { name: "評価を更新", exact: true }).click();
  await match(page, request, filter.id);
  mode = "fail";
  await page.getByRole("button", { name: "評価を更新", exact: true }).click();
  await expect(
    page.getByRole("alert").filter({ hasText: "評価失敗" }),
  ).toContainText("古い結果");
  expect(await cardIds(page)).toEqual([h.issues.assigned.id]);
  await capture(page, "board-network-failure-stale");
  mode = "ok";
  await page.getByRole("button", { name: "評価を更新", exact: true }).click();
  await match(page, request, filter.id);
  if (
    await page
      .getByRole("button", { name: "右サイドバーを隠す", exact: true })
      .isVisible()
  )
    await page
      .getByRole("button", { name: "右サイドバーを隠す", exact: true })
      .click();
  await page.emulateMedia({ colorScheme: "dark" });
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.locator("html")).toHaveClass(/dark/);
  await capture(page, "board-mobile-dark");
  await page
    .getByRole("button", { name: "フィルターを管理", exact: true })
    .click();
  await page.getByRole("button", { name: filter.name, exact: true }).click();
  await page
    .getByRole("textbox", { name: "自然文の条件", exact: true })
    .scrollIntoViewIfNeeded();
  await capture(page, "settings-mobile-dark-bottom");
  await page
    .getByRole("textbox", { name: "名前", exact: true })
    .scrollIntoViewIfNeeded();
  await capture(page, "settings-mobile-dark-top");
});

test("ordinary title/simple ID and symmetric related search differ from saved literal body search", async ({
  page,
  request,
}) => {
  const key = randomUUID().slice(0, 8);
  const a = await createIssue(request, {
    title: "search-source-" + key,
    description: "body-only-" + key,
  });
  const b = await createIssue(request, { title: "search-target-" + key });
  await api(request, "/api/local/v1/issue_relationships", "POST", {
    issue_id: a.id,
    related_issue_id: b.id,
    relationship_type: "related",
  });
  await page.goto(board);
  await page.getByRole("button", { name: "チーム", exact: true }).click();
  const clear = page.getByRole("button", {
    name: "絞り込みを解除",
    exact: true,
  });
  if (await clear.isVisible()) await clear.click();
  await page.getByRole("button", { name: "アクティブ", exact: true }).click();
  const search = page.getByRole("textbox", {
    name: "Issueを検索…",
    exact: true,
  });
  const related = page.getByRole("checkbox", {
    name: "関連イシューを含む",
    exact: true,
  });
  await related.check();
  await search.fill(a.title);
  await expect.poll(() => cardIds(page)).toEqual([a.id, b.id].sort());
  await search.fill(b.title);
  await expect.poll(() => cardIds(page)).toEqual([a.id, b.id].sort());
  await related.uncheck();
  await search.fill(a.simple_id.toLowerCase());
  await expect.poll(() => cardIds(page)).toEqual([a.id]);
  await search.fill("body-only-" + key);
  await expect.poll(() => cardIds(page)).toEqual([]);
  const literal = await createFilter(request, { search: "body-only-" + key });
  await page.goto(board + "?savedFilter=" + literal.id);
  expect(await match(page, request, literal.id)).toEqual([a.id]);
  await page.getByText(/条件を確認/).click();
  await expect(
    page.getByText(/simple ID検索や関連Issueの展開は行いません/),
  ).toBeVisible();
  await updateFilter(request, literal, {
    machine: machine({ search: a.simple_id }),
  });
  await expect.poll(() => cardIds(page)).toEqual([]);
  await updateFilter(request, literal, {
    machine: machine({ simple_id: a.simple_id.toLowerCase() }),
  });
  await expect.poll(() => cardIds(page)).toEqual([a.id]);
});

test("real editor multi-choice, periods, relation and archive round trip with desktop/mobile visuals", async ({
  page,
  request,
}) => {
  await page.goto(board);
  await page
    .getByRole("button", { name: "フィルターを管理", exact: true })
    .click();
  await page
    .getByRole("button", { name: "フィルターを作成", exact: true })
    .click();
  const name = "UI editor " + randomUUID().slice(0, 8);
  await page.getByRole("textbox", { name: "名前", exact: true }).fill(name);
  for (const status of h.statuses.machine.slice(0, 2))
    await page
      .getByRole("checkbox", { name: status.name, exact: true })
      .check();
  for (const priority of ["高", "低"])
    await page
      .getByRole("group", { name: "優先度", exact: true })
      .getByRole("checkbox", { name: priority, exact: true })
      .check();
  const local = page.getByRole("checkbox", { name: /^Local \(/ });
  await local.check();
  await page
    .getByRole("checkbox", { name: "未割当のIssueも含める（OR）", exact: true })
    .check();
  for (const tag of h.tags)
    await page.getByRole("checkbox", { name: tag.name, exact: true }).check();
  await page
    .getByRole("combobox", { name: "タグの一致方法", exact: true })
    .selectOption("all");
  await page
    .getByRole("combobox", { name: "作成期間", exact: true })
    .selectOption("absolute");
  await page.getByLabel("開始", { exact: true }).fill("2026-01-01T00:00");
  await page.getByLabel("終了", { exact: true }).fill("2027-01-01T00:00");
  await page
    .getByRole("combobox", { name: "更新期間", exact: true })
    .selectOption("relative");
  await page
    .getByRole("spinbutton", { name: "直近の秒数", exact: true })
    .fill("86400");
  await page
    .getByRole("combobox", { name: "対象Issueとの関係", exact: true })
    .selectOption("blocking");
  await page
    .getByRole("combobox", { name: "対象Issue", exact: true })
    .selectOption(h.issues.unassigned.id);
  await page
    .getByRole("combobox", { name: "Issueのアーカイブ範囲", exact: true })
    .selectOption("all");
  const saved = page.waitForResponse(
    (r) => r.url() === base + filterBase && r.request().method() === "POST",
  );
  await page
    .getByRole("button", { name: "フィルターを保存", exact: true })
    .click();
  const definition = (await (await saved).json()).data;
  const current = await api(request, `${filterBase}/${definition.id}`);
  expect(current.machine).toMatchObject({
    status_ids: h.statuses.machine.slice(0, 2).map((s: any) => s.id),
    priorities: ["high", "low"],
    include_unassigned: true,
    tag_mode: "all",
    archived: "all",
    updated_at: { kind: "relative", seconds: 86400 },
    relation: { kind: "blocking", target_issue_id: h.issues.unassigned.id },
  });
  expect(current.machine.assignee_ids).toEqual([
    "00000000-0000-0000-0000-000000000001",
  ]);
  expect([...current.machine.tag_ids].sort()).toEqual(
    h.tags.map((t: any) => t.id).sort(),
  );
  expect(current.machine.created_at.kind).toBe("absolute");
  expect(new Date(current.machine.created_at.start).getUTCFullYear()).toBe(
    2026,
  );
  expect(new Date(current.machine.created_at.end).getUTCFullYear()).toBe(2027);
  await page.getByRole("button", { name, exact: true }).click();
  await local.scrollIntoViewIfNeeded();
  await capture(page, "settings-desktop-light-middle");
  await page.emulateMedia({ colorScheme: "dark" });
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.locator("html")).toHaveClass(/dark/);
  await local.scrollIntoViewIfNeeded();
  await capture(page, "settings-mobile-dark-middle");
  await page
    .getByRole("combobox", { name: "作成期間", exact: true })
    .scrollIntoViewIfNeeded();
  await capture(page, "settings-mobile-dark-periods");
});
