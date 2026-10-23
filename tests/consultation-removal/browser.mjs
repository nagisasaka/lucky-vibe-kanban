// Changed Vite frontend + explicitly selected prebuilt backend + disposable DB.
// Canonical input/approval/run records are fixtures; no provider is launched.
import { chromium, expect } from "@playwright/test";
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { createWriteStream, constants } from "node:fs";
import {
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
  copyFile,
} from "node:fs/promises";
import { once } from "node:events";
import net from "node:net";
import path from "node:path";

const root = process.cwd();
assert.ok(
  process.env.LVK_TEST_BACKEND,
  "Set LVK_TEST_BACKEND to the authorised prebuilt server",
);
const output = path.join(root, ".build/consultation-removal");
await mkdir(output, { recursive: true });
const instance = await mkdtemp(path.join(output, "instance-"));
const binary = path.join(instance, "server");
const result = {
  frontendSha: execFileSync("git", ["rev-parse", "HEAD"], {
    encoding: "utf8",
  }).trim(),
  backendSource: process.env.LVK_TEST_BACKEND,
  cases: [],
  cleanup: {},
  limits: [
    "Prebuilt backend: removed context API is NOT validated",
    "Synthetic provider records; no real model, MCP answers or live OS push delivery",
  ],
};
const children = [];
const streams = [];
let browser, page, base, frontend, ports;
const errors = [],
  serverErrors = [],
  contextRequests = [];
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
function launch(name, command, args, env, cwd = root) {
  const child = spawn(command, args, {
    cwd,
    env,
    stdio: ["ignore", "pipe", "pipe"],
  });
  children.push(child);
  for (const [suffix, pipe] of [
    ["stdout", child.stdout],
    ["stderr", child.stderr],
  ]) {
    const stream = createWriteStream(
      path.join(output, `${name}.${suffix}.log`),
    );
    streams.push(stream);
    pipe.pipe(stream);
  }
  return child;
}
async function isListening(port) {
  return new Promise((resolve) => {
    const socket = net.connect({ host: "127.0.0.1", port });
    socket.once("connect", () => {
      socket.destroy();
      resolve(true);
    });
    socket.once("error", () => resolve(false));
  });
}
async function checkAbsent() {
  await expect(
    page.getByRole("button", { name: /ChatGPTに相談|依頼文をコピー/ }),
  ).toHaveCount(0);
  await expect(page.getByRole("link", { name: "ChatGPTを開く" })).toHaveCount(
    0,
  );
  await expect(
    page.getByRole("textbox", { name: "ChatGPTへの相談文" }),
  ).toHaveCount(0);
}
try {
  await copyFile(
    process.env.LVK_TEST_BACKEND,
    binary,
    constants.COPYFILE_FICLONE,
  );
  const env = {
    ...process.env,
    LVK_INSTANCE_DIR: instance,
    HOST: "127.0.0.1",
    BACKEND_PORT: "0",
    PORT: "",
    PREVIEW_PROXY_PORT: "0",
    LVK_ACCESS_DEPLOYMENT: "local",
    LVK_AUTH_MODE: "",
    DISABLE_WORKTREE_CLEANUP: "1",
    BROWSER: "true",
    RUST_LOG: "warn",
    VIBE_KANBAN_AGENT_PROCESS_HOST: path.join(instance, "no-provider-host"),
  };
  for (const key of [
    "LVK_PUBLIC_URL",
    "LVK_MCP_CONFIG",
    "VIBE_BACKEND_URL",
    "VK_ALLOWED_ORIGINS",
    "VK_PREVIEW_DOMAIN",
  ])
    delete env[key];
  const server = launch("server", binary, [], env, instance);
  for (let i = 0; i < 120; i++) {
    assert.equal(server.exitCode, null, "isolated backend exited");
    try {
      ports = JSON.parse(
        await readFile(path.join(instance, "ports/vibe-kanban.port"), "utf8"),
      );
      base = `http://127.0.0.1:${ports.main_port}`;
      if ((await fetch(base + "/api/info")).ok) break;
    } catch {
      /* Wait for this instance only. */
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  assert.ok(base);
  const info = (await api("/api/info")).data;
  result.backendBuildInfo = info.build_info;
  assert.equal(
    info.build_info.revision,
    "aa59ab5855ae90b0fe3cf65a2017ff60be52df61",
  );
  await api("/api/config", "PUT", {
    ...info.config,
    language: "EN",
    theme: "DARK",
    disclaimer_acknowledged: true,
    onboarding_acknowledged: true,
    remote_onboarding_acknowledged: true,
    telemetry_acknowledged: true,
    analytics_enabled: false,
    showcases: { seen_features: ["workspaces-guide"] },
  });
  // Vite port 0 chooses an unused loopback port; no existing listener is touched.
  await writeFile(path.join(output, "vite.stdout.log"), "");
  const vite = launch(
    "vite",
    process.execPath,
    ["node_modules/vite/bin/vite.js", "--host", "127.0.0.1", "--port", "0"],
    {
      ...env,
      BACKEND_PORT: String(ports.main_port),
      VITE_OPEN: "false",
      NO_COLOR: "1",
    },
    path.join(root, "packages/local-web"),
  );
  for (let i = 0; i < 120; i++) {
    assert.equal(vite.exitCode, null, "Vite exited");
    const log = await readFile(
      path.join(output, "vite.stdout.log"),
      "utf8",
    ).catch((error) => {
      if (error.code === "ENOENT") return "";
      throw error;
    });
    const match = log.match(/http:\/\/127\.0\.0\.1:(\d+)/);
    if (match) {
      frontend = match[0];
      break;
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  assert.ok(frontend, "Vite readiness");
  result.ports = { ...ports, frontend: Number(new URL(frontend).port) };
  const project = (
    await api("/api/local/v1/projects", "POST", {
      organization_id: "00000000-0000-0000-0000-000000000002",
      name: "Consultation removal",
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
      title: "Consultation removal fixture",
      sort_order: 0,
      extension_metadata: {},
    })
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
      "user.name=Fixture",
      "-c",
      "user.email=fixture@example.invalid",
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
      display_name: "Fixture",
    })
  ).data.id;
  browser = await chromium.launch({ headless: true, args: ["--no-sandbox"] });
  const context = await browser.newContext({
    viewport: { width: 1440, height: 1000 },
    permissions: ["clipboard-read", "clipboard-write"],
  });
  page = await context.newPage();
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("response", (response) => {
    if (response.status() >= 500)
      serverErrors.push(`${response.status()} ${response.url()}`);
  });
  page.on("request", (request) => {
    if (request.url().includes("/web-push/context/"))
      contextRequests.push(request.url());
  });
  await page.route("**/*", (route) =>
    new URL(route.request().url()).origin === frontend
      ? route.continue()
      : route.abort(),
  );
  for (const mode of ["awaiting_input", "awaiting_approval", "succeeded"]) {
    const workspace = (
      await api("/api/workspaces", "POST", { name: `Fixture ${mode}` })
    ).data.id;
    await api(`/api/workspaces/${workspace}/repos`, "POST", {
      repo_id: repo,
      target_branch: "main",
    });
    const session = (
      await api("/api/sessions", "POST", {
        workspace_id: workspace,
        executor: "CODEX",
        name: "Fixture session",
      })
    ).data.id;
    const fixture = JSON.parse(
      execFileSync(
        "python3",
        [
          "tests/consultation-removal/seed.py",
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
    const run = fixture.runs.at(-1).run;
    const route = `/workspaces/${workspace}?session_id=${session}&agent_run_id=${run}`;
    await page.goto(frontend + route);
    await expect(
      page.getByText(`通知対象Run: ${run}`, { exact: false }),
    ).toBeVisible({ timeout: 90000 });
    await expect(
      page.getByText(`通知対象Run: ${run}`, { exact: false }),
    ).toContainText(mode);
    if (mode === "awaiting_input") {
      await expect(
        page.getByText("Which branch should we use?", { exact: false }).last(),
      ).toBeVisible();
      const editor = page.getByRole("textbox", {
        name: "Markdown editor",
        exact: true,
      });
      await editor.fill("Use the fixture branch");
      await expect(
        page.getByRole("button", { name: "Send", exact: true }).last(),
      ).toBeEnabled();
    } else if (mode === "awaiting_approval") {
      await expect(
        page.getByRole("button", { name: "Approve", exact: true }).last(),
      ).toBeEnabled();
      await expect(
        page.getByRole("button", { name: "Stop", exact: true }).last(),
      ).toBeEnabled();
    } else {
      const message = page
        .getByText("Consultation fixture 1", { exact: true })
        .last();
      await expect(message).toBeVisible();
      await message.hover();
      const copy = message
        .locator(
          'xpath=ancestor::div[contains(concat(" ", normalize-space(@class), " "), " group ")][1]',
        )
        .getByRole("button", { name: "Copy as Markdown", exact: true });
      await copy.click();
      assert.equal(
        await page.evaluate(() => navigator.clipboard.readText()),
        "Consultation fixture 1",
      );
      result.cases.push({
        mode,
        view: "existing-copy-as-markdown",
        clipboard: "Consultation fixture 1",
      });
    }
    await checkAbsent();
    await page.screenshot({
      path: path.join(output, `${mode}-desktop-dark.png`),
    });
    result.cases.push({
      mode,
      workspace,
      session,
      run,
      view: "desktop-dark",
      text: await page.locator("body").innerText(),
    });
    await page.setViewportSize({ width: 390, height: 844 });
    await page.evaluate(() => {
      document.documentElement.classList.remove("dark");
      document.documentElement.classList.add("light");
    });
    await expect(
      page.getByText("Consultation fixture 1", { exact: true }).last(),
    ).toBeVisible({ timeout: 30000 });
    await page
      .getByText(`通知対象Run: ${run}`, { exact: false })
      .scrollIntoViewIfNeeded();
    await expect(
      page.getByText(`通知対象Run: ${run}`, { exact: false }),
    ).toBeVisible();
    if (mode === "awaiting_input") {
      await expect(
        page.getByText("Which branch should we use?", { exact: false }).last(),
      ).toBeVisible({ timeout: 30000 });
      await page
        .getByRole("textbox", { name: "Markdown editor", exact: true })
        .fill("Use the fixture branch");
      await expect(
        page.getByRole("button", { name: "Send", exact: true }).last(),
      ).toBeEnabled();
    } else if (mode === "awaiting_approval") {
      await expect(
        page.getByRole("button", { name: "Approve", exact: true }).last(),
      ).toBeEnabled({ timeout: 30000 });
      await expect(
        page.getByRole("button", { name: "Deny", exact: true }).last(),
      ).toBeEnabled();
    } else {
      await expect(
        page.getByText("Consultation fixture 1", { exact: true }).last(),
      ).toBeVisible({ timeout: 30000 });
    }
    await checkAbsent();
    await page.screenshot({
      path: path.join(output, `${mode}-mobile-light.png`),
    });
    result.cases.push({
      mode,
      view: "mobile-light",
      text: await page.locator("body").innerText(),
    });
    await page.setViewportSize({ width: 1440, height: 1000 });
    // Notification may point to an older completed run while the active one waits.
    const old = fixture.runs[0].run;
    await page.goto(
      `${frontend}/projects/${project}/issues/${issue}/workspaces/${workspace}?session_id=${session}&agent_run_id=${old}`,
    );
    await expect(
      page.getByText(`通知対象Run: ${old}`, { exact: false }),
    ).toContainText("succeeded", { timeout: 30000 });
    await checkAbsent();
    result.cases.push({ mode, view: "project-linked-old-notification", old });
    await page.goto(
      `${frontend}/workspaces/${workspace}?session_id=${session}`,
    );
    await expect(
      page.getByText("Consultation fixture 1", { exact: true }).last(),
    ).toBeVisible({ timeout: 30000 });
    await expect(page.getByText("通知対象Run:", { exact: false })).toHaveCount(
      0,
    );
    await checkAbsent();
    result.cases.push({ mode, view: "workspace-without-notification" });
  }
  assert.deepEqual(contextRequests, [], "removed frontend context request");
  assert.deepEqual(errors, [], "browser errors");
  assert.deepEqual(serverErrors, [], "server errors");
  result.status = "passed";
} catch (error) {
  result.status = "failed";
  result.error = error.stack;
  if (page) {
    result.failureText = await page
      .locator("body")
      .innerText()
      .catch(() => "");
    await page
      .screenshot({ path: path.join(output, "failure.png") })
      .catch(() => {});
  }
  throw error;
} finally {
  await browser?.close();
  result.cleanup.browserClosed = !browser?.isConnected();
  for (const child of children.reverse()) {
    if (child.exitCode === null && child.signalCode === null) {
      const exited = once(child, "exit");
      child.kill("SIGTERM");
      const timeout = setTimeout(() => child.kill("SIGKILL"), 10000);
      await exited;
      clearTimeout(timeout);
    }
  }
  result.cleanup.childrenExited = children.every(
    (child) => child.exitCode !== null || child.signalCode !== null,
  );
  result.cleanup.portsClosed = {};
  for (const [name, port] of Object.entries(result.ports || ports || {})) {
    if (typeof port === "number" && port > 0)
      result.cleanup.portsClosed[name] = !(await isListening(port));
  }
  await rm(instance, { recursive: true, force: true });
  result.cleanup.instanceRemoved = true;
  result.errors = errors;
  result.serverErrors = serverErrors;
  result.contextRequests = contextRequests;
  await writeFile(
    path.join(output, "results.json"),
    JSON.stringify(result, null, 2),
  );
  assert.ok(result.cleanup.childrenExited);
  assert.ok(Object.values(result.cleanup.portsClosed).every(Boolean));
  console.log(
    JSON.stringify(
      {
        status: result.status,
        cases: result.cases.length,
        cleanup: result.cleanup,
      },
      null,
      2,
    ),
  );
}
