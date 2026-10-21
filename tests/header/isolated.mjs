// Real isolated backend + SQLite + built workspace frontend; only GitHub is mocked.
// Build server, agent-process-host and local-web before running.
import { chromium, expect } from "@playwright/test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createWriteStream } from "node:fs";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { once } from "node:events";
import path from "node:path";

const root = process.cwd();
const output = path.join(root, ".build/header-validation");
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
    // Own a process group so backend capability-probe children are cleaned up too.
    detached: true,
    env: { ...env, ...extra },
    stdio: ["pipe", "pipe", "pipe"],
  });
  children.push(child);
  child.stderr.pipe(
    createWriteStream(path.join(output, `${binary}.stderr.log`)),
  );
  return child;
}
function signalGroup(child, signal) {
  try {
    process.kill(-child.pid, signal);
  } catch (error) {
    if (error.code !== "ESRCH") throw error;
  }
}
async function stop(child) {
  signalGroup(child, "SIGTERM");
  if (child.exitCode !== null || child.signalCode !== null) return;
  const exited = once(child, "exit");
  const timer = setTimeout(() => signalGroup(child, "SIGKILL"), 10000);
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
      name: "Header regression",
      color: "#6366f1",
    })
  ).data.id;
  const workspace = (
    await api("/api/workspaces", "POST", { name: "Header workspace" })
  ).data.id;
  browser = await chromium.launch({
    headless: true,
    args: ["--no-sandbox"],
    ...(process.env.LVK_CHROMIUM_EXECUTABLE
      ? { executablePath: process.env.LVK_CHROMIUM_EXECUTABLE }
      : {}),
  });
  const results = [];
  let releaseStars;
  const pendingResponse = new Promise((resolve) => {
    releaseStars = resolve;
  });
  const loadingContext = await browser.newContext();
  const loadingPage = await loadingContext.newPage();
  page = loadingPage;
  await loadingPage.route(
    "https://api.github.com/repos/nagisasaka/lucky-vibe-kanban",
    async (route) => {
      await pendingResponse;
      await route.fulfill({ json: { stargazers_count: 0 } });
    },
  );
  await loadingPage.goto(`${base}/projects/${project}`);
  const loadingLink = loadingPage.getByRole("link", {
    name: "Star on GitHub",
    exact: true,
  });
  await expect(loadingLink).toBeVisible();
  await expect(loadingLink.locator("span")).toHaveCount(0);
  releaseStars();
  await expect(loadingLink.locator("span")).toHaveText("0");
  await loadingContext.close();
  results.push({ scenario: "loading then zero", badge: "absent then 0" });
  for (const [name, status, payload, badge] of [
    ["zero", 200, { stargazers_count: 0 }, "0"],
    ["positive", 200, { stargazers_count: 1234 }, "1.2k"],
    ["failed", 403, { message: "rate limit" }, null],
    ["invalid", 200, { stargazers_count: -1 }, null],
    ["missing", 200, {}, null],
    ["network", 0, {}, null],
  ]) {
    if (name === "positive") {
      const current = (await api("/api/info")).data.config;
      await api("/api/config", "PUT", { ...current, theme: "LIGHT" });
    }
    const context = await browser.newContext({
      viewport: { width: 1440, height: 900 },
    });
    page = await context.newPage();
    page.setDefaultTimeout(15000);
    let requests = 0;
    const obsolete = [];
    page.on("request", (request) => {
      if (/discord|api.github.com\/repos\/BloopAI/i.test(request.url()))
        obsolete.push(request.url());
    });
    await page.route(
      "https://api.github.com/repos/nagisasaka/lucky-vibe-kanban",
      async (route) => {
        requests++;
        if (!status) await route.abort();
        else await route.fulfill({ status, json: payload });
      },
    );
    await page.goto(`${base}/projects/${project}`);
    const github = page.getByRole("link", {
      name: "Star on GitHub",
      exact: true,
    });
    await expect(github).toHaveAttribute(
      "href",
      "https://github.com/nagisasaka/lucky-vibe-kanban",
    );
    await expect.poll(() => requests).toBe(1);
    // Wait until the query has settled before testing absence of a badge.
    await page.waitForTimeout(500);
    if (badge === null) await expect(github.locator("span")).toHaveCount(0);
    else await expect(github.locator("span")).toHaveText(badge);
    for (const route of [`/projects/${project}`, `/workspaces/${workspace}`]) {
      await page.goto(base + route);
      await expect(github).toBeVisible();
      await expect(
        page.getByRole("button", { name: "Give Feedback", exact: true }),
      ).toHaveCount(0);
      await expect(
        page.locator('a[href*="discord"], a[href*="BloopAI/vibe-kanban"]'),
      ).toHaveCount(0);
      await expect(page.getByText("Local", { exact: true })).toHaveCount(0);
    }
    await page.locator('button[aria-label="Header regression"]').click();
    await expect(page).toHaveURL(new RegExp(`/projects/${project}`));
    await page
      .getByRole("button", { name: "Local workspaces", exact: true })
      .click();
    await expect(page).toHaveURL(/\/workspaces/);
    await expect(
      page.getByRole("button", { name: "Create project", exact: true }),
    ).toBeVisible();
    if (name === "zero") {
      await page
        .getByRole("button", { name: "Open Command Bar", exact: true })
        .click();
      await expect(page.getByRole("dialog")).toBeVisible();
      await expect(
        page.getByText("Give Feedback", { exact: true }),
      ).toHaveCount(0);
      await page.keyboard.press("Escape");
    }
    assert.deepEqual(obsolete, []);
    await page.screenshot({
      animations: "disabled",
      path: path.join(output, `screenshots/${name}.png`),
    });
    results.push({
      scenario: name,
      badge,
      navigation: "passed",
      removedControls: "absent",
    });
    await context.close();
  }
  const mobileContext = await browser.newContext({
    viewport: { width: 390, height: 844 },
    isMobile: true,
  });
  await mobileContext.route(
    "https://api.github.com/repos/nagisasaka/lucky-vibe-kanban",
    (route) => route.fulfill({ json: { stargazers_count: 0 } }),
  );
  page = await mobileContext.newPage();
  page.setDefaultTimeout(15000);
  await page.goto(`${base}/projects/${project}`);
  await page.getByRole("button", { name: "Open menu", exact: true }).click();
  await expect(page.getByText("Local", { exact: true })).toHaveCount(0);
  await expect(page.getByText("Give Feedback", { exact: true })).toHaveCount(0);
  await expect(page.locator('a[href*="discord"]')).toHaveCount(0);
  await page.getByRole("button", { name: "Workspaces", exact: true }).click();
  // The fresh instance may redirect the workspace entry to its create screen.
  await expect(page).toHaveURL(/\/workspaces(?:\/create)?$/);
  // Test the return menu on the existing workspace, outside the automatic create dialog.
  await page.goto(`${base}/workspaces/${workspace}`);
  await page.getByRole("button", { name: "Projects", exact: true }).click();
  await page
    .getByRole("button", { name: "Header regression", exact: true })
    .click();
  await expect(page).toHaveURL(`${base}/projects/${project}`);
  await page.screenshot({
    animations: "disabled",
    path: path.join(output, "screenshots/mobile.png"),
  });
  await mobileContext.close();
  results.push({
    scenario: "mobile navigation",
    viewport: "390x844",
    navigation: "passed",
  });
  const currentConfig = (await api("/api/info")).data.config;
  await api("/api/config", "PUT", {
    ...currentConfig,
    onboarding_acknowledged: false,
  });
  const onboardingContext = await browser.newContext();
  page = await onboardingContext.newPage();
  page.setDefaultTimeout(15000);
  await onboardingContext.route(
    "https://api.github.com/repos/nagisasaka/lucky-vibe-kanban",
    (route) => route.fulfill({ json: { stargazers_count: 0 } }),
  );
  await page.goto(base + "/onboarding");
  // ConfigProvider bypasses onboarding; RootRedirectPage selects the only project.
  await expect(page).toHaveURL(`${base}/projects/${project}`);
  await expect(
    page.getByRole("link", { name: "Star on GitHub", exact: true }),
  ).toHaveAttribute("href", "https://github.com/nagisasaka/lucky-vibe-kanban");
  await onboardingContext.close();
  results.push({
    scenario: "local onboarding bypass",
    result:
      "redirected to the first project; landing component is not reachable in local mode",
  });
  await writeFile(
    path.join(output, "results.json"),
    JSON.stringify(
      { backend: "real isolated SQLite/server", github: "mocked", results },
      null,
      2,
    ),
  );
  console.log(JSON.stringify(results, null, 2));
} catch (error) {
  if (page && !page.isClosed()) {
    await page
      .screenshot({ path: path.join(output, "screenshots/failure.png") })
      .catch(() => {});
    await writeFile(
      path.join(output, "failure.html"),
      await page.content(),
    ).catch(() => {});
  }
  throw error;
} finally {
  await browser?.close();
  for (const child of children.reverse()) await stop(child);
  await rm(instance, { recursive: true, force: true });
  await writeFile(
    path.join(output, "cleanup.json"),
    JSON.stringify(
      {
        browserClosed: true,
        servers: children.map((child) => ({
          pid: child.pid,
          exitCode: child.exitCode,
          signal: child.signalCode,
        })),
        instanceRemoved: instance,
      },
      null,
      2,
    ),
  );
}
