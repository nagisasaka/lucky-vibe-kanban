// Built workspace app + real isolated backend/SQLite. Only chat execution is mocked.
// Build server, agent-process-host and local-web first.
import { chromium, expect } from "@playwright/test";
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { createWriteStream } from "node:fs";
import { mkdir, mkdtemp, readFile, rm } from "node:fs/promises";
import { once } from "node:events";
import { randomUUID } from "node:crypto";
import path from "node:path";

const root = process.cwd();
const output = path.join(root, ".build/input-validation");
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
  const child = spawn(path.join(root, "target/debug", binary), args, {
    cwd: root,
    env: { ...env, ...extra },
    stdio: ["pipe", "pipe", "pipe"],
  });
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
  const server = start("server");
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
  console.log("Isolated app ready:", base);
  const config = (await api("/api/info")).data.config;
  await api("/api/config", "PUT", {
    ...config,
    language: "JA",
    send_message_shortcut: "ModifierEnter", // Old preference must not override chat.
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
  const session = (
    await api("/api/sessions", "POST", {
      workspace_id: workspace,
      executor: "CODEX",
      name: "UI検証（実行なし）",
    })
  ).data;

  browser = await chromium.launch({ headless: true, args: ["--no-sandbox"] });
  page = await browser.newPage({
    viewport: { width: 1600, height: 1000 },
    locale: "ja-JP",
  });
  page.setDefaultTimeout(20000);
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(`${base}/projects/${project}`);
  await page.getByRole("button", { name: "TODOリスト", exact: true }).click();
  const grid = page.getByTestId("todo-grid");
  const body = grid.getByLabel("気付いたことやTODOを追加…", { exact: true });
  const category = grid.getByLabel("カテゴリ", { exact: true });
  const add = grid.getByRole("button", { name: "追加", exact: true });
  const count = async () => (await api(todoPath)).data.items.length;
  const initial = await count();
  await body.fill("   ");
  await body.press("Enter");
  await expect(add).toBeDisabled();
  assert.equal(await count(), initial);
  await body.fill("分類が必要");
  await body.press("Enter");
  await expect(grid.getByRole("alert")).toContainText(
    "カテゴリを入力してください",
  );
  await category.fill("   ");
  await add.click();
  assert.equal(await count(), initial);
  await expect(body).toHaveValue("分類が必要");
  await category.fill("入力検証");
  await body.fill("日本語確定");
  for (const detail of [{ isComposing: true, keyCode: 13 }, { keyCode: 229 }]) {
    await body.dispatchEvent("keydown", { key: "Enter", ...detail });
    assert.equal(await count(), initial);
  }
  await body.fill("一行目");
  await body.press("Control+Enter");
  await page.keyboard.insertText("二行目");
  await expect(body).toHaveValue("一行目\n二行目");
  await body.dispatchEvent("keydown", {
    key: "Enter",
    keyCode: 13,
    repeat: true,
  });
  assert.equal(await count(), initial);
  let releaseTodo;
  const todoGate = new Promise((resolve) => {
    releaseTodo = resolve;
  });
  let posts = 0;
  await page.route(`**${todoPath}`, async (route) => {
    if (route.request().method() !== "POST") return route.continue();
    posts++;
    await todoGate;
    await route.continue(); // Real backend still performs the write; only timing is controlled.
  });
  await body.press("Enter");
  await expect(body).toBeDisabled();
  await expect(add).toBeDisabled();
  await body.evaluate((el) => {
    for (let i = 0; i < 5; i++) {
      el.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "Enter",
          keyCode: 13,
          bubbles: true,
        }),
      );
      el.form.requestSubmit();
    }
  });
  await expect.poll(() => posts).toBe(1);
  releaseTodo();
  await expect(body).toHaveValue("");
  assert.equal(await count(), initial + 1);
  const saved = (await api(todoPath)).data.items.find(
    (t) => t.body === "一行目\n二行目",
  );
  assert.ok(saved);
  assert.equal(saved.category, "入力検証");
  await page.unroute(`**${todoPath}`);
  await body.fill("ボタン追加\n複数行");
  await category.fill("入力検証");
  await add.click();
  await expect(body).toHaveValue("");
  assert.equal(await count(), initial + 2);
  await page.screenshot({
    path: path.join(output, "screenshots/todo-input.png"),
  });
  console.log(
    "PASS TODO: empty, category validation on Enter/button, IME flags, Ctrl+Enter, multiline persistence, repeat, pending duplicate submission; real backend/SQLite.",
  );

  await page.goto(`${base}/workspaces/${workspace}`);
  const editor = page.locator("[contenteditable]").first();
  await expect(editor).toBeVisible();
  const requests = [];
  let releaseChat;
  const chatGate = new Promise((resolve) => {
    releaseChat = resolve;
  });
  await page.route("**/api/sessions/*/follow-up", async (route) => {
    requests.push({
      ...route.request().postDataJSON(),
      path: new URL(route.request().url()).pathname,
    });
    await chatGate;
    await route.fulfill({
      status: 400,
      contentType: "application/json",
      body: JSON.stringify({
        success: false,
        data: null,
        message: "Controlled execution boundary: no agent started",
      }),
    });
  });
  await editor.fill("   ");
  await editor.press("Enter");
  assert.equal(requests.length, 0);
  await editor.fill("日本語確定");
  for (const detail of [{ isComposing: true, keyCode: 13 }, { keyCode: 229 }]) {
    await editor.dispatchEvent("keydown", { key: "Enter", ...detail });
    assert.equal(requests.length, 0);
  }
  await editor.fill("チャット一行目");
  await editor.press("Control+Enter");
  await page.keyboard.insertText("チャット二行目");
  assert.match(await editor.innerText(), /チャット一行目\nチャット二行目/);
  assert.equal(requests.length, 0);
  await editor.dispatchEvent("keydown", {
    key: "Enter",
    keyCode: 13,
    repeat: true,
  });
  assert.equal(requests.length, 0);
  await editor.press("Enter");
  await expect.poll(() => requests.length).toBe(1);
  assert.equal(requests[0].path, `/api/sessions/${session.id}/follow-up`);
  assert.match(requests[0].prompt, /チャット一行目\nチャット二行目/);
  await expect(editor).toHaveAttribute("contenteditable", "false");
  await editor.evaluate((el) => {
    for (let i = 0; i < 5; i++)
      el.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "Enter",
          keyCode: 13,
          bubbles: true,
        }),
      );
  });
  assert.equal(requests.length, 1);
  releaseChat();
  await expect(
    page.getByText(/Controlled execution boundary/).first(),
  ).toBeVisible();
  await expect(editor).toHaveAttribute("contenteditable", "true");
  await page.getByRole("button", { name: "送信", exact: true }).click();
  await expect.poll(() => requests.length).toBe(2);
  assert.equal(requests[1].prompt, requests[0].prompt);
  await page.screenshot({
    path: path.join(output, "screenshots/chat-input.png"),
  });
  assert.deepEqual(errors, []);
  console.log(
    "PASS CHAT: old ModifierEnter preference overridden, empty, IME flags, Ctrl+Enter multiline payload, Enter, repeat and pending guard. Actual workspace page/backend; follow-up response mocked as failure, no agent execution.",
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
      ).slice(0, 6000),
    );
  }
  throw error;
} finally {
  try {
    await browser?.close();
  } finally {
    for (const child of children.reverse()) await stop(child);
    await rm(instance, { recursive: true, force: true });
    console.log(
      "CLEANUP: browser closed, isolated server exited, dedicated instance removed.",
    );
  }
}
