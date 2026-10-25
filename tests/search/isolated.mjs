// Fresh SQLite and real API; source frontend via Vite. No production data/model calls.
import { chromium, expect } from "@playwright/test";
import { createServer } from "vite";
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { createWriteStream } from "node:fs";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { once } from "node:events";
import { createInterface } from "node:readline";
import { randomUUID } from "node:crypto";
import path from "node:path";
const root = process.cwd();
const output = path.join(root, ".build/search-validation");
await mkdir(output, { recursive: true });
const instance = await mkdtemp(path.join(output, "instance-"));
const binary = process.env.LVK_TEST_SERVER;
assert.ok(binary, "Set LVK_TEST_SERVER to a compatible prebuilt backend");
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
const server = spawn(binary, [], {
  cwd: root,
  env,
  stdio: ["ignore", "pipe", "pipe"],
});
server.stdout.pipe(createWriteStream(path.join(output, "server.stdout.log")));
server.stderr.pipe(createWriteStream(path.join(output, "server.stderr.log")));
let browser, page, vite, base, mcp;
async function api(url, method = "GET", body) {
  const response = await fetch(base + url, {
    method,
    redirect: "error",
    headers: { "content-type": "application/json", "x-lvk-csrf": "local" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const result = await response.json();
  assert.equal(response.status, 200, `${url}: ${JSON.stringify(result)}`);
  assert.notEqual(result.success, false, JSON.stringify(result));
  return result;
}
const evidence = [];
try {
  for (let i = 0; i < 120; i++) {
    assert.equal(server.exitCode, null, "backend exited");
    try {
      const ports = JSON.parse(
        await readFile(path.join(instance, "ports/vibe-kanban.port"), "utf8"),
      );
      base = `http://127.0.0.1:${ports.main_port}`;
      if ((await fetch(base + "/api/info")).ok) break;
    } catch {}
    await new Promise((r) => setTimeout(r, 500));
  }
  assert.ok(base, "backend did not start");
  const info = (await api("/api/info")).data;
  await writeFile(
    path.join(output, "revision.json"),
    JSON.stringify(
      {
        source: execFileSync("git", ["rev-parse", "HEAD"], {
          cwd: root,
          encoding: "utf8",
        }).trim(),
        dirty: execFileSync("git", ["status", "--porcelain"], {
          cwd: root,
          encoding: "utf8",
        }),
        binary,
        build: info.build_info,
      },
      null,
      2,
    ),
  );
  await api("/api/config", "PUT", {
    ...info.config,
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
      name: "LOCAL-180 isolated search",
      color: "#6366f1",
    })
  ).data.id;
  const statuses = (
    await api(`/api/board/project-statuses?project_id=${project}`)
  ).data.project_statuses;
  const issue = (
    await api("/api/local/v1/issues", "POST", {
      project_id: project,
      status_id: statuses[0].id,
      title: "Search fixture",
      description: "needle body",
      sort_order: 0,
      extension_metadata: {},
    })
  ).data;
  const tagA = (
    await api("/api/local/v1/tags", "POST", {
      project_id: project,
      name: "Tag A",
      color: "blue",
    })
  ).data;
  const tagB = (
    await api("/api/local/v1/tags", "POST", {
      project_id: project,
      name: "Tag B",
      color: "green",
    })
  ).data;
  for (const tag of [tagA, tagB])
    await api("/api/local/v1/issue_tags", "POST", {
      issue_id: issue.id,
      tag_id: tag.id,
    });
  const workspace = (
    await api("/api/workspaces", "POST", { name: "Search fixture workspace" })
  ).data.id;
  await api(`/api/workspaces/${workspace}/links`, "POST", {
    project_id: project,
    issue_id: issue.id,
  });
  const repoPath = path.join(instance, "repo");
  await mkdir(repoPath);
  execFileSync("git", ["init", "-b", "main", repoPath], { stdio: "ignore" });
  execFileSync(
    "git",
    [
      "-C",
      repoPath,
      "-c",
      "user.name=Search test",
      "-c",
      "user.email=test@example.invalid",
      "-c",
      "core.hooksPath=/dev/null",
      "commit",
      "--allow-empty",
      "-m",
      "fixture",
    ],
    { stdio: "ignore" },
  );
  const repo = (
    await api("/api/repos", "POST", {
      path: repoPath,
      display_name: "search fixture",
    })
  ).data.id;
  await api(`/api/workspaces/${workspace}/repos`, "POST", {
    repo_id: repo,
    target_branch: "main",
  });

  await api("/api/sessions", "POST", {
    workspace_id: workspace,
    executor: "CODEX",
    name: "Search UI fixture (no execution)",
  });
  const todoPath = `/api/projects/${project}/todos`;
  const todo = [];
  for (const [body, category] of [
    ["Alpha Needle", "one"],
    ["Other body", "Needle category"],
    ["Unrelated", "other"],
  ]) {
    todo.push(
      (
        await api(todoPath, "POST", {
          id: randomUUID(),
          body,
          category,
          issue_id: null,
        })
      ).data,
    );
  }
  const completed = (
    await api(todoPath, "POST", {
      id: randomUUID(),
      body: "Needle completed",
      category: "done",
      issue_id: null,
    })
  ).data;
  await api(`${todoPath}/${completed.id}`, "PATCH", {
    expected_revision: completed.revision,
    completed: true,
  });
  const notePath = `/api/projects/${project}/design-notes`;
  const notes = [];
  for (const [title, body] of [
    ["Needle title", "first body"],
    ["Second title", "Markdown **Needle**"],
    ["Other title", "unrelated"],
  ]) {
    notes.push(
      (
        await api(notePath, "POST", {
          request_id: randomUUID(),
          operation: { action: "create", title, body },
        })
      ).data.note,
    );
  }
  mcp = spawn(
    path.join(path.dirname(binary), "vibe-kanban-mcp"),
    ["--mode", "global"],
    {
      cwd: root,
      env: { ...env, VIBE_BACKEND_URL: base },
      stdio: ["pipe", "pipe", "pipe"],
    },
  );
  mcp.stderr.pipe(createWriteStream(path.join(output, "mcp.stderr.log")));
  const pending = new Map();
  let seq = 0;
  createInterface({ input: mcp.stdout }).on("line", (line) => {
    const msg = JSON.parse(line),
      p = pending.get(msg.id);
    if (p) {
      pending.delete(msg.id);
      clearTimeout(p.timer);
      msg.error
        ? p.reject(new Error(JSON.stringify(msg.error)))
        : p.resolve(msg.result);
    }
  });
  function rpc(method, params) {
    return new Promise((resolve, reject) => {
      const id = ++seq,
        timer = setTimeout(() => {
          pending.delete(id);
          reject(new Error(`MCP timeout ${method}`));
        }, 30000);
      pending.set(id, { resolve, reject, timer });
      mcp.stdin.write(
        JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n",
      );
    });
  }
  await rpc("initialize", {
    protocolVersion: "2025-03-26",
    capabilities: {},
    clientInfo: { name: "local180-search", version: "1" },
  });
  mcp.stdin.write(
    JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) +
      "\n",
  );
  async function tool(name, args) {
    const r = await rpc("tools/call", { name, arguments: args });
    assert.ok(!r.isError, JSON.stringify(r));
    return (
      r.structuredContent ??
      JSON.parse(r.content.find((c) => c.type === "text").text)
    );
  }
  const todoResult = await tool("list_todos", {
    project_id: project,
    search: "nEeDlE",
  });
  assert.equal(todoResult.items.length, 2);
  assert.equal(todoResult.next_offset, null);
  assert.equal(
    (await tool("list_todos", { project_id: project, search: " Needle " }))
      .items.length,
    0,
  );
  assert.equal(
    (await tool("list_todos", { project_id: project })).items.length,
    3,
  );
  assert.equal(
    (
      await tool("list_todos", {
        project_id: project,
        search: "NEEDLE",
        include_completed: true,
      })
    ).items.length,
    3,
  );
  const first = await tool("list_design_notes", {
    project_id: project,
    search: "NEEDLE",
    page_size: 1,
  });
  assert.equal(first.items.length, 1);
  assert.equal(first.has_more, true);
  assert.ok(!("body" in first.items[0]));
  const last = await tool("list_design_notes", {
    project_id: project,
    search: "NEEDLE",
    page_size: 1,
    cursor: first.next_cursor,
  });
  assert.equal(last.items.length, 1);
  assert.equal(last.has_more, false);
  assert.notEqual(first.items[0].id, last.items[0].id);
  const issues = await tool("list_issues", {
    project_id: project,
    search: "NEEDLE",
    status: statuses[0].name,
    limit: 1,
    offset: 0,
  });
  assert.equal(issues.total_count, 1);
  assert.equal(issues.issues[0].id, issue.id);
  assert.equal(issues.returned_count, 1);
  assert.equal(
    (await tool("list_issues", { project_id: project, search: " needle " }))
      .total_count,
    0,
  );
  assert.equal(
    (await tool("list_issues", { project_id: project, limit: 0 }))
      .returned_count,
    0,
  );
  assert.equal(
    (
      await tool("list_issues", {
        project_id: project,
        tag_id: tagA.id,
        tag_name: "TAG B",
      })
    ).total_count,
    0,
    "tag ID/name must identify the same tag even when the Issue has both tags",
  );
  assert.equal(
    (
      await tool("list_issues", {
        project_id: project,
        tag_id: tagA.id,
        tag_name: "TAG A",
      })
    ).total_count,
    1,
  );
  evidence.push(
    "Real stdio MCP: TODO body/category/completion/whitespace, design-note cursor metadata/body search, Issue filters/offset/count",
  );

  process.env.BACKEND_PORT = new URL(base).port;
  process.chdir(path.join(root, "packages/local-web"));
  vite = await createServer({
    configFile: path.join(root, "packages/local-web/vite.config.ts"),
    server: { port: 0, host: "127.0.0.1", open: false },
  });
  await vite.listen();
  const url = `http://127.0.0.1:${vite.httpServer.address().port}`;
  browser = await chromium.launch({ headless: true, args: ["--no-sandbox"] });
  page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  page.setDefaultTimeout(20000);
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(`${url}/projects/${project}`);
  const card = page.locator(`[data-kanban-card="${issue.id}"]`);
  await expect(card).toBeVisible({ timeout: 90000 });
  const kanbanSearch = page.getByPlaceholder("Search issues...");
  for (const query of [
    "  SEARCH FIXTURE  ",
    issue.simple_id.toLowerCase(),
    String(issue.issue_number),
  ]) {
    await kanbanSearch.fill(query);
    await expect(card).toBeVisible();
  }
  await kanbanSearch.fill("needle");
  await expect(card).toHaveCount(0);
  await kanbanSearch.fill("");
  await expect(card).toBeVisible();
  evidence.push(
    "Existing Kanban browser search still trims/case-folds title/simple_id/number and excludes body",
  );

  await page.getByRole("button", { name: "TODOリスト", exact: true }).click();
  const todoPanel = page.getByTestId("todo-grid");
  const search = todoPanel.getByRole("searchbox");
  const rows = todoPanel.locator("[data-todo-id]");
  await expect(rows).toHaveCount(3);
  const requests = [];
  page.on("request", (r) => {
    if (r.url().includes("/todos?")) requests.push(r.url());
  });
  await search.fill("  nEeDlE  ");
  await expect(rows).toHaveCount(2);
  await todoPanel.getByLabel("完了済みも表示").check();
  await expect(rows).toHaveCount(3);
  await search.fill("unrelated");
  await expect(rows).toHaveCount(1);
  await search.fill("");
  await expect(rows).toHaveCount(4);
  assert.ok(requests.every((u) => !/[?&](search|query)=/.test(u)));
  await page.screenshot({ path: path.join(output, "todo-project.png") });
  evidence.push(
    "Project TODO browser search, category OR body, trim/case, existing completion filter, no search API requests",
  );

  await page.getByRole("button", { name: "設計メモ", exact: true }).click();
  const panel = page.getByTestId("design-notes"),
    list = panel.locator("aside");
  await expect(list.getByRole("button")).toHaveCount(4);
  await panel.getByRole("searchbox").fill("  nEeDlE  ");
  await expect(list.getByRole("button")).toHaveCount(3);
  await list.getByRole("button", { name: /Needle title/ }).click();
  await expect(panel.getByTestId("note-body")).toContainText("first body");
  await panel.getByRole("searchbox").fill("Other title");
  await expect(list.getByRole("button", { name: /Needle title/ })).toHaveCount(
    0,
  );
  await expect(panel.getByTestId("note-body")).toContainText("first body");
  await panel.getByRole("button", { name: "本文を編集" }).click();
  const editor = panel
    .getByTestId("note-body-editor")
    .locator("[contenteditable=true]");
  await editor.fill("unsaved input retained");
  await panel.getByRole("searchbox").fill("no matches");
  await expect(editor).toContainText("unsaved input retained");
  await panel.getByRole("searchbox").fill("");
  await expect(editor).toContainText("unsaved input retained");
  await page.screenshot({ path: path.join(output, "design-note-draft.png") });
  evidence.push(
    "Design-note browser full Markdown/title search and selected detail/unsaved draft retained when filtered out",
  );

  await page.goto(`${url}/workspaces/${workspace}`);
  await page
    .getByRole("button", { name: "Toggle TODO list", exact: true })
    .click();
  const workspaceTodo = page.getByTestId("todo-list");
  const picker = page.getByLabel("プロジェクト", { exact: true });
  if (await picker.count()) await picker.selectOption(project);
  await workspaceTodo.getByRole("searchbox").fill("NEEDLE");
  await expect(workspaceTodo.locator("[data-todo-id]")).toHaveCount(2);
  await page.screenshot({ path: path.join(output, "todo-workspace.png") });
  await page.setViewportSize({ width: 390, height: 844 });
  assert.ok(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
    "mobile horizontal overflow",
  );
  await page.screenshot({
    path: path.join(output, "todo-workspace-mobile.png"),
  });
  evidence.push(
    "Workspace TODO browser search at desktop width; existing mobile layout has no horizontal overflow",
  );
  assert.deepEqual(errors, []);
  await writeFile(
    path.join(output, "results.json"),
    JSON.stringify(evidence, null, 2),
  );
  console.log(
    "PASS isolated LOCAL-180 API, stdio and browser search checks",
    evidence,
  );
} catch (error) {
  if (page)
    await page
      .screenshot({ path: path.join(output, "failure.png") })
      .catch(() => {});
  throw error;
} finally {
  await browser?.close();
  await vite?.close();
  if (server.exitCode === null && server.signalCode === null) {
    const exited = once(server, "exit");
    server.kill("SIGTERM");
    const timer = setTimeout(() => server.kill("SIGKILL"), 10000);
    await exited;
    clearTimeout(timer);
  }
  if (mcp && mcp.exitCode === null && mcp.signalCode === null) {
    const exited = once(mcp, "exit");
    mcp.kill("SIGTERM");
    const timer = setTimeout(() => mcp.kill("SIGKILL"), 10000);
    await exited;
    clearTimeout(timer);
  }
  await rm(instance, { recursive: true, force: true });
  process.chdir(root);
  console.log(
    "CLEANUP: browser, Vite, isolated backend stopped; temporary instance removed",
  );
}
