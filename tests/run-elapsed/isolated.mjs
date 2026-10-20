// Own-source server + built app + real SQLite/API/WebSocket; provider records are synthetic.
import { chromium, expect } from "@playwright/test";
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { createWriteStream } from "node:fs";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { once } from "node:events";
import path from "node:path";

const root = process.cwd();
const output = path.join(root, ".build/run-elapsed-validation");
await mkdir(output, { recursive: true });
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
const server = spawn(path.join(root, "target/debug/server"), [], {
  cwd: root,
  env,
  stdio: ["ignore", "pipe", "pipe"],
});
server.stdout.pipe(createWriteStream(path.join(output, "server.stdout.log")));
server.stderr.pipe(createWriteStream(path.join(output, "server.stderr.log")));
let browser, page, base;
const evidence = [];
async function api(url, method = "GET", body) {
  const response = await fetch(base + url, {
    method,
    redirect: "error",
    headers: { "content-type": "application/json", "x-lvk-csrf": "local" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const data = await response.json();
  assert.equal(response.status, 200, `${url}: ${JSON.stringify(data)}`);
  assert.notEqual(data.success, false, JSON.stringify(data));
  return data;
}
try {
  for (let i = 0; i < 240; i++) {
    assert.equal(server.exitCode, null, "isolated server exited");
    try {
      const ports = JSON.parse(
        await readFile(path.join(instance, "ports/vibe-kanban.port"), "utf8"),
      );
      base = `http://127.0.0.1:${ports.main_port}`;
      if ((await fetch(base + "/api/info")).ok) break;
    } catch {}
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  assert.ok(base, "no isolated server port");
  const config = (await api("/api/info")).data.config;
  await api("/api/config", "PUT", {
    ...config,
    language: "EN",
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
      name: "Elapsed E2E",
      color: "#6366f1",
    })
  ).data.id;
  const statuses = (
    await api(`/api/local/v1/fallback/project_statuses?project_id=${project}`)
  ).project_statuses;
  const issue = (
    await api("/api/local/v1/issues", "POST", {
      project_id: project,
      status_id: statuses[0].id,
      title: "Elapsed regression",
      sort_order: 0,
      extension_metadata: {},
    })
  ).data.id;
  const workspace = (
    await api("/api/workspaces", "POST", { name: "Elapsed verification" })
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
      "user.name=Elapsed Test",
      "-c",
      "user.email=elapsed@example.invalid",
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
      display_name: "Elapsed fixture",
    })
  ).data.id;
  await api(`/api/workspaces/${workspace}/repos`, "POST", {
    repo_id: repo,
    target_branch: "main",
  });
  const session = (
    await api("/api/sessions", "POST", {
      workspace_id: workspace,
      executor: "CODEX",
      name: "Elapsed fixture",
    })
  ).data.id;
  const seed = (mode) =>
    JSON.parse(
      execFileSync(
        "python3",
        [
          "tests/run-elapsed/seed.py",
          instance,
          project,
          issue,
          mode,
          workspace,
          session,
        ],
        { encoding: "utf8" },
      ),
    );
  const fixture = seed("seed");
  const summaries = (
    await api("/api/workspaces/summaries", "POST", { archived: false })
  ).data;
  evidence.push({ step: "workspace summary", data: summaries });
  browser = await chromium.launch({ headless: true, args: ["--no-sandbox"] });
  page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  const errors = [];
  const serverErrors = [];
  const observeResponse = (response) => {
    if (response.status() >= 500)
      serverErrors.push(`${response.status()} ${response.url()}`);
  };
  page.on("response", observeResponse);
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(`${base}/workspaces/${fixture.workspace}`);
  const clocks = page.getByTestId("run-elapsed");
  await expect(clocks).toHaveCount(2, { timeout: 60000 });
  await expect(clocks.first()).toHaveText("12s");
  await expect(
    page.getByText("Elapsed Goal fixture", { exact: true }),
  ).toBeVisible();
  const before = await clocks.last().innerText();
  await expect
    .poll(() => clocks.last().innerText(), { timeout: 10000 })
    .not.toBe(before);
  evidence.push({
    step: "chat running / old completed",
    clocks: await clocks.allTextContents(),
  });
  await page.screenshot({ path: path.join(output, "chat-running.png") });
  const board = await browser.newPage({
    viewport: { width: 1440, height: 1000 },
  });
  board.on("response", observeResponse);
  await board.goto(`${base}/projects/${project}`);
  await expect(
    board.getByText("Elapsed verification", { exact: true }),
  ).toBeVisible({ timeout: 30000 });
  const runningCard = board
    .getByText("Elapsed verification", { exact: true })
    .locator("xpath=../../..");
  const runningClock = runningCard.locator(".tabular-nums");
  await expect(runningClock).toContainText("s");
  const boardBefore = await runningClock.innerText();
  await expect
    .poll(() => runningClock.innerText(), { timeout: 10000 })
    .not.toBe(boardBefore);
  evidence.push({ step: "board running", text: await runningCard.innerText() });
  await board.screenshot({ path: path.join(output, "board-running.png") });
  await board.close();
  seed("succeeded");
  await expect(clocks.last()).toHaveText("42s", { timeout: 15000 });
  await page.waitForTimeout(2200);
  await expect(clocks.last()).toHaveText("42s");
  await expect(page.getByText(/Recorded time:/)).toContainText("0m 7s");
  await page.reload();
  await expect(clocks.last()).toHaveText("42s", { timeout: 30000 });
  evidence.push({
    step: "chat completed / reconnect",
    clocks: await clocks.allTextContents(),
  });
  await page.screenshot({ path: path.join(output, "chat-completed.png") });
  await page.goto(`${base}/projects/${project}`);
  await expect(
    page.getByText("Elapsed verification", { exact: true }),
  ).toBeVisible({ timeout: 30000 });
  const card = page
    .getByText("Elapsed verification", { exact: true })
    .locator("xpath=../../..");
  await expect(card).toContainText("42s");
  await page.waitForTimeout(2200);
  await expect(card).toContainText("42s");
  evidence.push({ step: "board completed", text: await card.innerText() });
  await page.screenshot({ path: path.join(output, "board-completed.png") });
  assert.deepEqual(errors, []);
  assert.deepEqual(serverErrors, []);
  console.log(
    "PASS real backend app E2E (synthetic canonical provider records)",
  );
} catch (error) {
  evidence.push({ error: String(error) });
  if (page) {
    await writeFile(
      path.join(output, "failure.txt"),
      await page
        .locator("body")
        .innerText()
        .catch(() => "no page"),
    );
    await page
      .screenshot({ path: path.join(output, "failure.png") })
      .catch(() => {});
  }
  throw error;
} finally {
  await writeFile(
    path.join(output, "evidence.json"),
    JSON.stringify(evidence, null, 2),
  );
  await browser?.close();
  if (server.exitCode === null && server.signalCode === null) {
    const exited = once(server, "exit");
    server.kill("SIGTERM");
    const timeout = setTimeout(() => server.kill("SIGKILL"), 10000);
    await exited;
    clearTimeout(timeout);
  }
  await rm(instance, { recursive: true, force: true });
}
