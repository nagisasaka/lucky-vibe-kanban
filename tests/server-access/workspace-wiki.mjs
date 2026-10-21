// Built application + real backend; disposable repository content is the only fixture.
// Run after building local-web, server and agent-process-host:
// node tests/server-access/workspace-wiki.mjs /absolute/evidence/directory
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { once } from "node:events";
import { mkdir, mkdtemp, rm, writeFile, open } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve, join } from "node:path";
import { createServer } from "node:net";
import { chromium, expect } from "@playwright/test";

const evidence = resolve(process.argv[2] || ".local-33-evidence/e2e");
await mkdir(evidence, { recursive: true });
const root = await mkdtemp(join(tmpdir(), "lvk-workspace-wiki-"));
const reservation = createServer();
await new Promise((done) => reservation.listen(0, "127.0.0.1", done));
const port = reservation.address().port;
await new Promise((done) => reservation.close(done));
const base = `http://127.0.0.1:${port}`;
const log = await open(join(evidence, "server.log"), "w");
const server = spawn(resolve("target/debug/server"), [], {
  detached: true,
  stdio: ["ignore", log.fd, log.fd],
  env: {
    ...Object.fromEntries(
      Object.entries(process.env).filter(([key]) => !key.startsWith("LVK_")),
    ),
    LVK_INSTANCE_DIR: join(root, "instance"),
    LVK_ACCESS_DEPLOYMENT: "local",
    LVK_AUTH_MODE: "local",
    LVK_PUBLIC_URL: "",
    HOST: "127.0.0.1",
    BACKEND_PORT: String(port),
    PORT: "",
    PREVIEW_PROXY_PORT: "0",
    RUST_LOG: "info",
    VK_ALLOWED_ORIGINS: base,
    VK_PREVIEW_DOMAIN: "",
    BROWSER: "true",
    DISABLE_WORKTREE_CLEANUP: "1",
    VIBE_KANBAN_AGENT_PROCESS_HOST: resolve("target/debug/agent-process-host"),
  },
});
const exited = once(server, "exit");
let csrf;
let browser;
let page;
const results = {
  base,
  fixture:
    "Temporary Git repository with two Wiki pages; no mocked HTTP/WebSocket responses and no agent execution",
  checks: [],
};
function descendants() {
  const rows = execFileSync("ps", ["-eo", "pid=,ppid="], { encoding: "utf8" })
    .trim()
    .split("\n")
    .map((line) => line.trim().split(/\s+/).map(Number));
  const found = new Set([process.pid]);
  for (let changed = true; changed; ) {
    changed = false;
    for (const [pid, parent] of rows)
      if (found.has(parent) && !found.has(pid)) {
        found.add(pid);
        changed = true;
      }
  }
  found.delete(process.pid);
  return [...found];
}
function alive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    if (error.code !== "ESRCH") throw error;
    return false;
  }
}
const check = (name) => {
  results.checks.push(name);
  console.log(`PASS ${name}`);
};
async function api(path, method = "GET", data) {
  const response = await fetch(`${base}/api${path}`, {
    method,
    signal: AbortSignal.timeout(30000),
    headers: {
      "Content-Type": "application/json",
      Origin: base,
      ...(csrf ? { "x-lvk-csrf": csrf } : {}),
    },
    body: data === undefined ? undefined : JSON.stringify(data),
  });
  const body = await response.json();
  assert.equal(
    response.ok && body.success,
    true,
    `${method} ${path}: ${JSON.stringify(body)}`,
  );
  return body.data;
}
try {
  await expect
    .poll(
      async () => {
        if (server.exitCode !== null)
          throw new Error("Server exited before readiness");
        return fetch(`${base}/api/health`)
          .then((r) => r.status)
          .catch(() => 0);
      },
      { timeout: 120000 },
    )
    .toBe(200);
  const access = await (await fetch(`${base}/api/access/status`)).json();
  assert.equal(access.mode, "local");
  csrf = access.csrf;
  const info = await api("/info");
  await api("/config", "PUT", {
    ...info.config,
    onboarding_acknowledged: true,
    disclaimer_acknowledged: true,
    telemetry_acknowledged: true,
    analytics_enabled: false,
    showcases: { seen_features: ["workspaces-guide"] },
    language: "EN",
  });
  const repoPath = join(root, "fixture-repo");
  await mkdir(join(repoPath, "openwiki"), { recursive: true });
  await writeFile(
    join(repoPath, "openwiki/index.md"),
    "# Wiki E2E Home\n\n[Second page](second.md)\n",
  );
  await writeFile(
    join(repoPath, "openwiki/second.md"),
    "# Wiki E2E Second\n\n[Home](index.md)\n",
  );
  const git = (...args) =>
    execFileSync("git", ["-C", repoPath, ...args], { stdio: "pipe" });
  git("init", "-b", "main");
  git("add", ".");
  git(
    "-c",
    "user.name=Fixture",
    "-c",
    "user.email=fixture@example.invalid",
    "commit",
    "-m",
    "Wiki fixture",
  );
  const repo = await api("/repos", "POST", {
    path: repoPath,
    display_name: "Wiki fixture",
  });
  await api(`/repos/${repo.id}/memory`, "PUT", {
    enabled: false,
    target_branch: "main",
    output_language: "ja",
  });
  const workspace = await api("/workspaces", "POST", { name: "Wiki pane E2E" });
  await api(`/workspaces/${workspace.id}/repos`, "POST", {
    repo_id: repo.id,
    target_branch: "main",
  });
  await api("/sessions", "POST", {
    workspace_id: workspace.id,
    executor: "CODEX",
    name: "Draft preservation",
  });
  const wiki = await api(`/workspaces/${workspace.id}/wiki?repo_id=${repo.id}`);
  assert.equal(wiki.wiki.exists, true);
  check(
    "Real backend: isolated DB, workspace, repository, session and Wiki snapshot",
  );
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    viewport: { width: 1600, height: 1000 },
  });
  await context.tracing.start({
    screenshots: true,
    snapshots: true,
    sources: false,
  });
  page = await context.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  const url = `${base}/workspaces/${workspace.id}`;
  await page.goto(url);
  const editor = page.locator('[contenteditable="true"]').first();
  await expect(editor).toBeVisible({ timeout: 60000 });
  const draft = "LOCAL-33 未送信の会話下書き";
  await editor.fill(draft);
  await page
    .getByRole("button", { name: "Toggle Wiki Reader", exact: true })
    .click();
  const article = page.getByRole("region", { name: "Wiki article" });
  await expect(article).toBeVisible();
  await expect(article).toContainText("Wiki E2E Home");
  await expect(editor).toBeVisible();
  await expect(editor).toContainText(draft);
  const left = await editor.boundingBox(),
    right = await article.boundingBox();
  assert.ok(left.x + left.width <= right.x + 2, "Chat is left of Wiki");
  await expect(
    page.getByRole("button", {
      name: /Return to workspace|Focus Wiki|Show chat alongside/,
    }),
  ).toHaveCount(0);
  assert.equal(page.url(), url);
  await page.screenshot({ path: join(evidence, "desktop-split.png") });
  check(
    "Desktop split, unchanged URL, visible chat draft, no dedicated return button",
  );
  await article.getByRole("link", { name: "Second page" }).click();
  await expect(article).toContainText("Wiki E2E Second");
  await page
    .getByRole("button", { name: "Toggle Wiki Reader", exact: true })
    .click();
  await expect(article).toHaveCount(0);
  await expect(editor).toContainText(draft);
  await page
    .getByRole("button", { name: "Toggle Wiki Reader", exact: true })
    .click();
  await expect(article).toContainText("Wiki E2E Second");
  check("Wiki page navigation and icon close/reopen preserve draft and page");
  await page
    .getByRole("button", { name: "Toggle Changes Panel", exact: true })
    .first()
    .click();
  await expect(article).toHaveCount(0);
  await expect(editor).toContainText(draft);
  await page
    .getByRole("button", { name: "Toggle Wiki Reader", exact: true })
    .click();
  await expect(article).toBeVisible();
  check("Other pane switch and return preserve draft");
  await expect
    .poll(async () => JSON.stringify(await api("/scratch")))
    .toContain(draft);
  await expect
    .poll(async () => JSON.stringify(await api("/scratch")))
    .toContain("wiki");
  await page.reload();
  await expect(article).toBeVisible({ timeout: 60000 });
  await expect(editor).toContainText(draft);
  check("Reload restores common pane state and backend scratch draft");
  await page.goto(`${base}/workspaces`);
  await page.goBack();
  await expect(article).toBeVisible();
  await expect(editor).toContainText(draft);
  assert.equal(page.url(), url);
  check("Workspace URL navigation and browser back preserve pane and draft");
  await page.setViewportSize({ width: 1000, height: 800 });
  await expect(editor).toBeVisible();
  await expect(article).toBeVisible();
  await page.screenshot({ path: join(evidence, "narrow-desktop.png") });
  check("Narrow desktop retains existing split rule");
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole("button", { name: "Wiki", exact: true }).click();
  await expect(
    page.getByRole("navigation", { name: "Wiki pages" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "index", exact: true }).click();
  await expect(article).toBeVisible();
  await expect(article).toContainText("Wiki E2E Home");
  await page.screenshot({ path: join(evidence, "mobile-wiki.png") });
  await page.getByRole("button", { name: "Chat", exact: true }).click();
  await expect(editor).toBeVisible();
  await expect(editor).toContainText(draft);
  await page.screenshot({ path: join(evidence, "mobile-chat.png") });
  check("Mobile Wiki tree/article/chat tab switching preserves draft");
  assert.deepEqual(errors, []);
  check("No browser page errors");
  await context.tracing.stop({ path: join(evidence, "trace.zip") });
} catch (error) {
  results.error = String(error.stack || error);
  if (page) {
    await page
      .screenshot({ path: join(evidence, "failure.png") })
      .catch(() => {});
    await writeFile(join(evidence, "failure.html"), await page.content()).catch(
      () => {},
    );
  }
  throw error;
} finally {
  const childPids = descendants();
  await browser?.close();
  if (server.exitCode === null) {
    process.kill(-server.pid, "SIGTERM");
    const timer = setTimeout(() => {
      try {
        process.kill(-server.pid, "SIGKILL");
      } catch {}
    }, 10000);
    await exited;
    clearTimeout(timer);
  }
  // Also settle owned descendants that started their own process group.
  for (const pid of childPids.filter(alive)) {
    try {
      process.kill(pid, "SIGTERM");
    } catch (error) {
      if (error.code !== "ESRCH") throw error;
    }
  }
  await log.close();
  await expect.poll(() => alive(-server.pid), { timeout: 15000 }).toBe(false);
  await expect
    .poll(() => childPids.filter(alive), { timeout: 15000 })
    .toEqual([]);
  await assert.rejects(
    fetch(`${base}/api/health`),
    "Temporary port must be closed",
  );
  await rm(root, { recursive: true, force: true });
  results.cleanup = {
    serverProcessGroupGone: true,
    portClosed: true,
    browserClosed: true,
    descendantsGone: childPids,
    temporaryDirectoryRemoved: true,
  };
  await writeFile(
    join(evidence, "result.json"),
    JSON.stringify(results, null, 2),
  );
  console.log(
    "Cleanup verified: server group gone, browser closed, port closed, temporary data removed",
  );
}
