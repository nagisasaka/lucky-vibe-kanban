// Real local server + SQLite + stdio MCP + Chromium. Never connects to a running
// LVK instance. Build server, agent-process-host, vibe-kanban-mcp and web first.
import { chromium, expect } from "@playwright/test";
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { createWriteStream } from "node:fs";
import { mkdir, mkdtemp, readFile, rm } from "node:fs/promises";
import { once } from "node:events";
import { createInterface } from "node:readline";
import { randomUUID } from "node:crypto";
import path from "node:path";
import { checkBodyExpansion } from "./body-expansion.mjs";
import { checkCategoryLayout } from "./layout.mjs";

const root = process.cwd();
const output = path.join(root, ".build/todo-validation");
await mkdir(path.join(output, "screenshots"), { recursive: true });
const instance = await mkdtemp(path.join(output, "instance-"));
const env = {
  ...process.env,
  LVK_INSTANCE_DIR: instance,
  HOST: "127.0.0.1",
  BACKEND_PORT: "0",
  PREVIEW_PROXY_PORT: "0",
  LVK_ACCESS_DEPLOYMENT: "local",
  LVK_AUTH_MODE: "local",
  DISABLE_WORKTREE_CLEANUP: "1",
  RUST_LOG: "warn",
};
for (const key of [
  "LVK_PUBLIC_URL",
  "LVK_MCP_CONFIG",
  "VIBE_BACKEND_URL",
  "VK_ALLOWED_ORIGINS",
])
  delete env[key];
const children = [];
function start(binary, args = [], extra = {}) {
  const child = spawn(
    path.join(
      process.env.LVK_TODO_BIN_DIR || path.join(root, "target/debug"),
      binary,
    ),
    args,
    {
      cwd: root,
      env: { ...env, ...extra },
      stdio: ["pipe", "pipe", "pipe"],
    },
  );
  children.push(child);
  child.stderr.pipe(
    createWriteStream(path.join(output, `${binary}.stderr.log`)),
  );
  return child;
}
async function stop(child) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  const exited = once(child, "exit");
  child.kill("SIGTERM");
  const timer = setTimeout(() => child.kill("SIGKILL"), 10000);
  await exited;
  clearTimeout(timer);
}
let vite;
let webBase;
let browser;
let page;
let base;
async function api(url, method = "GET", body, expected = 200) {
  const response = await fetch(base + url, {
    method,
    headers: { "content-type": "application/json", "x-lvk-csrf": "local" },
    body: body === undefined ? undefined : JSON.stringify(body),
    redirect: "error",
  });
  const result = await response.json();
  assert.equal(
    response.status,
    expected,
    `${method} ${url}: ${JSON.stringify(result)}`,
  );
  if (expected === 200)
    assert.notEqual(result.success, false, JSON.stringify(result));
  return result;
}
try {
  let server = start("server");
  server.stdout.pipe(createWriteStream(path.join(output, "server.stdout.log")));
  for (let i = 0; i < 240; i++) {
    assert.equal(server.exitCode, null, "isolated server exited");
    try {
      const ports = JSON.parse(
        await readFile(path.join(instance, "ports/vibe-kanban.port"), "utf8"),
      );
      base = `http://127.0.0.1:${ports.main_port}`;
      if ((await fetch(base + "/api/info")).ok) break;
    } catch {}
    await new Promise((r) => setTimeout(r, 500));
  }
  assert.ok(base, "server did not start");
  const config = (await api("/api/info")).data.config;
  await api("/api/config", "PUT", {
    ...config,
    language: "JA",
    theme: "DARK",
    disclaimer_acknowledged: true,
    onboarding_acknowledged: true,
    remote_onboarding_acknowledged: true,
    telemetry_acknowledged: true,
    analytics_enabled: false,
    showcases: { seen_features: ["workspaces-guide"] },
  });
  const project = (
    await api("/api/local/v1/projects", "POST", {
      organization_id: "00000000-0000-0000-0000-000000000002",
      name: "LVK TODO 検証",
      color: "#6366f1",
    })
  ).data.id;
  const todoPath = `/api/projects/${project}/todos`;
  const samples = [
    ["Codex 関連", "Codex CLI の更新"],
    ["Codex 関連", "Planモードの動作確認"],
    [
      "チャット画面UI",
      "スクロールが最新結果に追従しない\nワークスペースを切り替えたときに発生？",
    ],
    ["チャット画面UI", "各実行の時刻と累積時間を表示したい"],
    [
      "自動マージ機能",
      "テストの重複実行について検討する\n省略オプションだけでよいかは要議論",
    ],
    ["LLM Wiki 機能", "Wiki更新の表示を確認する（修正済み？）"],
    ["コンテナ整備", "コンテナに less を同梱"],
    ["設計確認・再検討", "ワークスペースの長寿命化について議論"],
  ];
  for (const [category, body] of samples)
    await api(todoPath, "POST", { id: randomUUID(), body, category });
  // Recreate the pre-feature schema only in this stopped, task-owned fixture.
  // Restart exercises the production migrator against populated legacy TODOs.
  await stop(server);
  const legacyId = execFileSync(
    "python3",
    [
      "-c",
      `
import sqlite3,sys
c=sqlite3.connect(sys.argv[1])
c.execute("DROP TRIGGER project_todos_creation_source_immutable")
c.execute("ALTER TABLE project_todos DROP COLUMN creation_source")
c.execute("DELETE FROM _sqlx_migrations WHERE version=20261024000000")
print(c.execute("SELECT lower(hex(id)) FROM project_todos ORDER BY created_at,id LIMIT 1").fetchone()[0])
c.commit()
`,
      path.join(instance, "data/db.v2.sqlite"),
    ],
    { encoding: "utf8" },
  )
    .trim()
    .replace(/(.{8})(.{4})(.{4})(.{4})(.{12})/, "$1-$2-$3-$4-$5");
  server = start("server");
  server.stdout.pipe(
    createWriteStream(path.join(output, "server-restart.stdout.log")),
  );
  base = undefined;
  for (let i = 0; i < 240; i++) {
    assert.equal(server.exitCode, null, "restarted server exited");
    try {
      const ports = JSON.parse(
        await readFile(path.join(instance, "ports/vibe-kanban.port"), "utf8"),
      );
      const candidate = `http://127.0.0.1:${ports.main_port}`;
      if ((await fetch(candidate + "/api/info")).ok) {
        base = candidate;
        break;
      }
    } catch {}
    await new Promise((r) => setTimeout(r, 500));
  }
  assert.ok(base, "restarted server did not start");
  assert.equal(
    (await api(`${todoPath}/${legacyId}`)).data.creation_source,
    "unknown",
  );
  const internalToken = (
    await readFile(path.join(instance, "data/access-internal.token"), "utf8")
  ).trim();
  const unclassifiedId = randomUUID();
  const unclassified = await fetch(base + todoPath, {
    method: "POST",
    redirect: "error",
    headers: {
      "content-type": "application/json",
      "x-lvk-internal-token": internalToken,
    },
    body: JSON.stringify({
      id: unclassifiedId,
      body: "unknown internal route",
    }),
  });
  assert.equal((await unclassified.json()).data.creation_source, "unknown");
  await api(`${todoPath}/${unclassifiedId}?expected_revision=1`, "DELETE");
  await api(
    `/api/internal/projects/${project}/todos`,
    "POST",
    { id: randomUUID(), body: "web cannot claim MCP" },
    403,
  );
  // A header is not accepted as provenance, and input DTOs reject arbitrary labels.
  const spoof = await fetch(base + todoPath, {
    method: "POST",
    redirect: "error",
    headers: {
      "content-type": "application/json",
      "x-lvk-csrf": "local",
      "x-lvk-creation-source": "mcp",
    },
    body: JSON.stringify({
      id: randomUUID(),
      body: "spoof",
      creation_source: "mcp",
    }),
  });
  assert.equal(spoof.status, 422);
  const headerId = randomUUID();
  const headerOnly = await fetch(base + todoPath, {
    method: "POST",
    redirect: "error",
    headers: {
      "content-type": "application/json",
      "x-lvk-csrf": "local",
      "x-lvk-creation-source": "mcp",
    },
    body: JSON.stringify({ id: headerId, body: "web route" }),
  });
  assert.equal((await headerOnly.json()).data.creation_source, "ui");
  const crossRouteRetry = await fetch(
    base + `/api/internal/projects/${project}/todos`,
    {
      method: "POST",
      redirect: "error",
      headers: {
        "content-type": "application/json",
        "x-lvk-internal-token": internalToken,
      },
      body: JSON.stringify({ id: headerId, body: "web route" }),
    },
  );
  assert.equal((await crossRouteRetry.json()).data.creation_source, "ui");
  await api(`${todoPath}/${headerId}?expected_revision=1`, "DELETE");
  const workspace = (
    await api("/api/workspaces", "POST", { name: "TODO 動作確認" })
  ).data.id;
  const repositoryPath = path.join(instance, "fixture-repository");
  await mkdir(repositoryPath);
  execFileSync("git", ["init", "-b", "main", repositoryPath], {
    stdio: "ignore",
  });
  execFileSync(
    "git",
    [
      "-C",
      repositoryPath,
      "-c",
      "user.name=TODO Test",
      "-c",
      "user.email=todo@example.invalid",
      "-c",
      "core.hooksPath=/dev/null",
      "commit",
      "--allow-empty",
      "-m",
      "Fixture",
    ],
    { stdio: "ignore" },
  );
  const repo = (
    await api("/api/repos", "POST", {
      path: repositoryPath,
      display_name: "TODO fixture",
    })
  ).data.id;
  await api(`/api/workspaces/${workspace}/repos`, "POST", {
    repo_id: repo,
    target_branch: "main",
  });
  await api("/api/sessions", "POST", {
    workspace_id: workspace,
    executor: "CODEX",
    name: "UI検証（実行なし）",
  });

  const mcp = start("vibe-kanban-mcp", ["--mode", "global"], {
    VIBE_BACKEND_URL: base,
  });
  let sequence = 0;
  const pending = new Map();
  createInterface({ input: mcp.stdout }).on("line", (line) => {
    const msg = JSON.parse(line);
    const entry = pending.get(msg.id);
    if (entry) {
      pending.delete(msg.id);
      clearTimeout(entry.timer);
      msg.error
        ? entry.reject(new Error(JSON.stringify(msg.error)))
        : entry.resolve(msg.result);
    }
  });
  const rpc = (method, params) =>
    new Promise((resolve, reject) => {
      const id = ++sequence;
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new Error(`MCP timeout: ${method}`));
      }, 30000);
      pending.set(id, { resolve, reject, timer });
      mcp.stdin.write(
        JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n",
      );
    });
  await rpc("initialize", {
    protocolVersion: "2025-03-26",
    capabilities: {},
    clientInfo: { name: "todo-regression", version: "1" },
  });
  mcp.stdin.write(
    JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) +
      "\n",
  );
  const catalog = await rpc("tools/list", {});
  for (const name of [
    "list_todos",
    "get_todo",
    "create_todo",
    "update_todo",
    "delete_todo",
  ])
    assert.ok(
      catalog.tools.some((t) => t.name === name),
      name,
    );
  const tool = async (name, args) => {
    const result = await rpc("tools/call", { name, arguments: args });
    assert.ok(!result.isError, JSON.stringify(result));
    return (
      result.structuredContent ??
      JSON.parse(result.content.find((c) => c.type === "text").text)
    );
  };
  const mcpId = randomUUID();
  await tool("create_todo", {
    project_id: project,
    input: { id: mcpId, body: "MCPから追加したメモ", category: "MCP" },
  });
  assert.equal(
    (await api(`${todoPath}/${mcpId}`)).data.body,
    "MCPから追加したメモ",
  );
  assert.equal(
    (await tool("get_todo", { project_id: project, todo_id: mcpId }))
      .creation_source,
    "mcp",
  );
  const list = await tool("list_todos", { project_id: project });
  assert.equal(list.items.find((t) => t.id === mcpId).creation_source, "mcp");
  assert.equal(list.items.length, samples.length + 1);

  webBase = base;
  if (process.env.LVK_TODO_VITE === "1") {
    process.env.BACKEND_PORT = new URL(base).port;
    process.chdir(path.join(root, "packages/local-web"));
    const { createServer } = await import("vite");
    vite = await createServer({
      root: path.join(root, "packages/local-web"),
      configFile: path.join(root, "packages/local-web/vite.config.ts"),
      server: { host: "127.0.0.1", port: 0, open: false },
    });
    await vite.listen();
    webBase = `http://127.0.0.1:${vite.httpServer.address().port}`;
  }
  browser = await chromium.launch({
    headless: true,
    args: ["--no-sandbox"],
    ...(process.env.LVK_CHROMIUM_EXECUTABLE
      ? { executablePath: process.env.LVK_CHROMIUM_EXECUTABLE }
      : {}),
  });
  page = await browser.newPage({
    viewport: { width: 1600, height: 1000 },
    locale: "ja-JP",
  });
  page.setDefaultTimeout(process.env.LVK_TODO_VITE === "1" ? 60000 : 15000);
  const errors = [];
  page.on("pageerror", (e) => {
    errors.push(e.message);
    console.error("PAGE ERROR", e.message);
  });
  await page.goto(`${webBase}/projects/${project}`);
  await page.getByRole("button", { name: "TODOリスト", exact: true }).click();
  const grid = page.getByTestId("todo-grid");
  await expect(grid).toBeVisible();
  await expect(
    grid.getByRole("button", { name: "MCPから追加したメモ", exact: true }),
  ).toBeVisible();
  await grid
    .getByLabel("気付いたことやTODOを追加…", { exact: true })
    .fill("UIから追加\n原因はまだ不明？");
  await grid.getByLabel("カテゴリ", { exact: true }).fill("チャット画面UI");
  await grid.getByRole("button", { name: "追加", exact: true }).click();
  await expect(
    grid.getByRole("button", {
      name: "UIから追加\n原因はまだ不明？",
      exact: true,
    }),
  ).toBeVisible();
  const created = (
    await tool("list_todos", { project_id: project })
  ).items.find((t) => t.body === "UIから追加\n原因はまだ不明？");
  assert.ok(created, "UI addition must be readable via MCP");
  assert.equal(created.creation_source, "ui");
  for (const [id, source] of [
    [created.id, "ui"],
    [mcpId, "mcp"],
    [legacyId, "unknown"],
  ]) {
    await expect(
      page.locator(`[data-todo-id="${id}"] [data-creation-source="${source}"]`),
    ).toBeVisible();
  }
  let row = page.locator(`[data-todo-id="${created.id}"]`);
  await row
    .getByRole("button", { name: "UIから追加\n原因はまだ不明？", exact: true })
    .click();
  await row
    .getByLabel("TODOを編集", { exact: true })
    .fill("編集したメモ\n不確実性をそのまま保存");
  await row.getByRole("button", { name: "保存", exact: true }).click();
  await expect(
    row.getByRole("button", {
      name: "編集したメモ\n不確実性をそのまま保存",
      exact: true,
    }),
  ).toBeVisible();
  await row.getByRole("checkbox").click();
  await expect(row).toHaveCount(0);
  await grid.getByLabel("完了済みも表示", { exact: true }).check();
  await expect(row.getByRole("checkbox")).toBeChecked();
  await row.getByRole("checkbox").click();
  await expect(row.getByRole("checkbox")).not.toBeChecked();
  const latest = (await api(`${todoPath}/${created.id}`)).data;
  await api(
    `${todoPath}/${created.id}`,
    "PATCH",
    { expected_revision: 1, body: "stale overwrite" },
    409,
  );
  assert.equal((await api(`${todoPath}/${created.id}`)).data.body, latest.body);
  await grid
    .getByLabel("気付いたことやTODOを追加…", { exact: true })
    .fill("未送信の思い付き");
  await page
    .getByRole("button", { name: "カンバンボード", exact: true })
    .click();
  await page.getByRole("button", { name: "TODOリスト", exact: true }).click();
  await expect(
    grid.getByLabel("気付いたことやTODOを追加…", { exact: true }),
  ).toHaveValue("未送信の思い付き");
  await grid.getByLabel("気付いたことやTODOを追加…", { exact: true }).fill("");
  await page
    .getByRole("button", { name: "統合 / 自動マージ", exact: true })
    .click();
  const integrationTitle = page.getByRole("heading", {
    name: "自動マージ",
    exact: true,
  });
  await expect(integrationTitle).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(integrationTitle).toHaveCount(0);
  await page.screenshot({
    path: path.join(output, "screenshots/project-dark.png"),
    fullPage: true,
  });
  const columns = await grid
    .locator("div.grid")
    .evaluate(
      (el) => getComputedStyle(el).gridTemplateColumns.split(" ").length,
    );
  assert.ok(columns > 1, "desktop TODO grid must use multiple columns");

  await checkBodyExpansion({
    page,
    browser,
    webBase,
    project,
    grid,
    api,
    todoPath,
    output,
  });
  await checkCategoryLayout({ page, grid, api, todoPath, output });

  // Both interfaces edit every source; provenance remains the initial route.
  await grid.getByLabel("完了済みも表示", { exact: true }).check();
  for (const [id, source] of [
    [created.id, "ui"],
    [mcpId, "mcp"],
    [legacyId, "unknown"],
  ]) {
    const original = (await api(`${todoPath}/${id}`)).data;
    const changed = await tool("update_todo", {
      project_id: project,
      todo_id: id,
      changes: {
        expected_revision: original.revision,
        body: original.body + " edited via MCP",
        category: "経路検証",
        completed: true,
      },
    });
    assert.equal(changed.creation_source, source);
    const sourceRow = page.locator(`[data-todo-id="${id}"]`);
    await expect(
      sourceRow.getByRole("button", {
        name: original.body + " edited via MCP",
        exact: true,
      }),
    ).toBeVisible({ timeout: 15000 });
    await sourceRow
      .getByRole("button", {
        name: original.body + " edited via MCP",
        exact: true,
      })
      .click();
    await sourceRow
      .getByLabel("TODOを編集", { exact: true })
      .fill(original.body);
    await sourceRow
      .getByLabel("カテゴリ", { exact: true })
      .fill(original.category);
    await sourceRow.getByRole("button", { name: "保存", exact: true }).click();
    await expect(
      sourceRow.getByRole("button", { name: original.body, exact: true }),
    ).toBeVisible();
    await expect(sourceRow.getByRole("checkbox")).toBeChecked();
    await sourceRow.getByRole("checkbox").click();
    await expect(sourceRow.getByRole("checkbox")).not.toBeChecked();
    const read = await tool("get_todo", { project_id: project, todo_id: id });
    assert.equal(read.creation_source, source);
    assert.equal(read.body, original.body);
    assert.equal(read.category, original.category);
    assert.equal(read.completed, false);
  }
  await page.reload();
  if (!(await grid.isVisible()))
    await page.getByRole("button", { name: "TODOリスト", exact: true }).click();
  for (const [id, source] of [
    [created.id, "ui"],
    [mcpId, "mcp"],
    [legacyId, "unknown"],
  ]) {
    await expect(
      page.locator(`[data-todo-id="${id}"] [data-creation-source="${source}"]`),
    ).toBeVisible();
    assert.equal(
      (await tool("list_todos", { project_id: project })).items.find(
        (t) => t.id === id,
      ).creation_source,
      source,
    );
  }
  await page.screenshot({
    path: path.join(output, "screenshots/creation-sources.png"),
    fullPage: true,
  });

  await page.goto(`${webBase}/workspaces/${workspace}`);
  const editor = page.locator('[contenteditable="true"]').first();
  await editor.fill("未送信のチャットを保持");
  await page
    .getByRole("button", { name: "Toggle TODO list", exact: true })
    .click();
  await page.getByLabel("プロジェクト", { exact: true }).selectOption(project);
  const panel = page.getByTestId("todo-list");
  await expect(editor).toContainText("未送信のチャットを保持");
  assert.ok(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
    "desktop workspace overflow",
  );
  await expect(
    panel.getByRole("button", { name: "MCPから追加したメモ", exact: true }),
  ).toBeVisible();
  await tool("update_todo", {
    project_id: project,
    todo_id: mcpId,
    changes: {
      expected_revision: (await api(`${todoPath}/${mcpId}`)).data.revision,
      body: "MCPで更新 → UIにも反映",
    },
  });
  await expect(
    panel.getByRole("button", { name: "MCPで更新 → UIにも反映", exact: true }),
  ).toBeVisible({ timeout: 15000 });
  const statuses = (
    await api(`/api/local/v1/fallback/project_statuses?project_id=${project}`)
  ).project_statuses;
  const issue = (
    await api("/api/local/v1/issues", "POST", {
      project_id: project,
      status_id: statuses[0].id,
      title: "TODOから検討するカード",
      sort_order: 0,
      extension_metadata: {},
    })
  ).data.id;
  await api(`/api/workspaces/${workspace}/links`, "POST", {
    project_id: project,
    issue_id: issue,
  });
  await expect(page.getByLabel("プロジェクト", { exact: true })).toHaveCount(
    0,
    { timeout: 15000 },
  );
  await expect(panel).toBeVisible();
  await tool("update_todo", {
    project_id: project,
    todo_id: mcpId,
    changes: {
      expected_revision: (await api(`${todoPath}/${mcpId}`)).data.revision,
      issue_id: issue,
    },
  });
  await expect(
    panel.getByRole("button", { name: "関連カード", exact: true }),
  ).toBeVisible({ timeout: 15000 });
  await page.screenshot({
    path: path.join(output, "screenshots/workspace-dark.png"),
    fullPage: true,
  });

  await page
    .getByRole("button", { name: "Toggle TODO list", exact: true })
    .click();
  await expect(editor).toContainText("未送信のチャットを保持");
  await page
    .getByRole("button", { name: "Toggle TODO list", exact: true })
    .click();
  await expect(panel).toBeVisible();
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole("button", { name: "TODO", exact: true }).click();
  await expect(page.getByTestId("todo-list")).toBeVisible();
  await page.screenshot({
    path: path.join(output, "screenshots/workspace-mobile.png"),
    fullPage: true,
  });
  assert.ok(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
    "workspace mobile overflow",
  );
  await page.getByRole("button", { name: "Chat", exact: true }).click();
  await expect(page.locator('[contenteditable="true"]').first()).toContainText(
    "未送信のチャットを保持",
  );
  await page.setViewportSize({ width: 1600, height: 1000 });
  await panel.getByRole("button", { name: "関連カード", exact: true }).click();
  await expect(page).toHaveURL(
    new RegExp(`/projects/${project}/issues/${issue}`),
  );
  await expect(page.getByTestId("todo-grid")).toHaveCount(0);

  await page.goto(`${webBase}/projects/${project}`);
  if (!(await grid.isVisible()))
    await page.getByRole("button", { name: "TODOリスト", exact: true }).click();
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({
    path: path.join(output, "screenshots/project-mobile.png"),
    fullPage: true,
  });
  assert.ok(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
    "mobile page must not overflow",
  );
  await page.setViewportSize({ width: 1600, height: 1000 });
  const currentConfig = (await api("/api/info")).data.config;
  await api("/api/config", "PUT", { ...currentConfig, theme: "LIGHT" });
  await page.reload();
  await page.getByRole("button", { name: "TODOリスト", exact: true }).click();
  await expect(page.locator(`[data-todo-id="${created.id}"]`)).toBeVisible();
  await page.screenshot({
    path: path.join(output, "screenshots/project-light.png"),
    fullPage: true,
  });
  row = page.locator(`[data-todo-id="${created.id}"]`);
  await row.getByRole("button", { name: "TODOの操作", exact: true }).click();
  await page.getByRole("menuitem", { name: "削除", exact: true }).click();
  await row.getByRole("button", { name: "削除", exact: true }).click();
  await expect(row).toHaveCount(0);
  await tool("delete_todo", {
    project_id: project,
    todo_id: mcpId,
    expected_revision: (await api(`${todoPath}/${mcpId}`)).data.revision,
  });
  await expect(page.locator(`[data-todo-id="${mcpId}"]`)).toHaveCount(0, {
    timeout: 15000,
  });
  assert.deepEqual(errors, [], "browser runtime errors");
  console.log(
    "PASS: real API + stdio MCP + Chromium CRUD, revisions, pane navigation, Integration dialog, responsive grid, dark/light, reload persistence. No agents executed.",
  );
} catch (error) {
  if (page) {
    await page
      .screenshot({
        path: path.join(output, "screenshots/failure.png"),
        fullPage: true,
      })
      .catch(() => {});
    console.error(
      (
        await page
          .locator("body")
          .innerText()
          .catch(() => "")
      ).slice(0, 5000),
    );
  }
  throw error;
} finally {
  await browser?.close();
  await vite?.close();
  process.chdir(root);
  for (const child of children.reverse()) await stop(child);
  await rm(instance, { recursive: true, force: true });
}
