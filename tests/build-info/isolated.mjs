// Built application + real, disposable SQLite. Only the explicitly named
// /api/info cases below replace metadata; all other app requests use the server.
import { chromium, expect } from "@playwright/test";
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { createWriteStream } from "node:fs";
import {
  access,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import { once } from "node:events";
import net from "node:net";
import path from "node:path";
import os from "node:os";

const root = process.cwd();
const output = path.resolve(
  process.env.LVK_BUILD_INFO_EVIDENCE || "test-results/build-info",
);
await mkdir(output, { recursive: true });
const instance = await mkdtemp(path.join(os.tmpdir(), "lvk-build-info-"));
const launchDirectory = path.join(instance, "unrelated-checkout");
await mkdir(launchDirectory);
execFileSync("git", ["init", "--quiet", launchDirectory]);
execFileSync("git", [
  "-C",
  launchDirectory,
  "-c",
  "user.name=Test",
  "-c",
  "user.email=test@example.invalid",
  "commit",
  "--allow-empty",
  "-m",
  "unrelated launch checkout",
]);
const launchSha = execFileSync(
  "git",
  ["-C", launchDirectory, "rev-parse", "HEAD"],
  { encoding: "utf8" },
).trim();
const expectedSha = execFileSync("git", ["rev-parse", "HEAD"], {
  encoding: "utf8",
}).trim();
const expectedDirty =
  execFileSync("git", ["status", "--porcelain"], { encoding: "utf8" }).trim()
    .length > 0;
assert.notEqual(launchSha, expectedSha);
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
  VIBE_KANBAN_AGENT_PROCESS_HOST: path.join(
    root,
    "target/debug/agent-process-host",
  ),
  // These must not override the identity embedded in the running executable.
  LVK_BUILD_REVISION: launchSha,
  LVK_BUILD_KIND: "release",
  LVK_BUILD_RELEASE_TAG: "v999.0.0",
  LVK_BUILD_DIRTY: "false",
};
for (const key of [
  "LVK_PUBLIC_URL",
  "LVK_MCP_CONFIG",
  "VIBE_BACKEND_URL",
  "VK_ALLOWED_ORIGINS",
  "VK_PREVIEW_DOMAIN",
])
  delete env[key];
let server;
let browser;
let page;
let ports;
let base;
const results = {
  expectedSha,
  expectedDirty,
  launchSha,
  cases: [],
  cleanup: {},
};
const streams = [];
async function api(url, method = "GET", body) {
  const response = await fetch(base + url, {
    method,
    redirect: "error",
    headers: { "content-type": "application/json", "x-lvk-csrf": "local" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  assert.equal(response.status, 200, `${method} ${url}`);
  const value = await response.json();
  assert.equal(value.success, true);
  return value;
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
try {
  server = spawn(path.join(root, "target/debug/server"), [], {
    cwd: launchDirectory,
    env,
    stdio: ["ignore", "pipe", "pipe"],
  });
  results.serverPid = server.pid;
  for (const [name, pipe] of [
    ["stdout", server.stdout],
    ["stderr", server.stderr],
  ]) {
    const stream = createWriteStream(path.join(output, `server.${name}.log`));
    streams.push(stream);
    pipe.pipe(stream);
  }
  for (let i = 0; i < 240; i++) {
    assert.equal(
      server.exitCode,
      null,
      "isolated server exited before readiness",
    );
    try {
      ports = JSON.parse(
        await readFile(path.join(instance, "ports/vibe-kanban.port"), "utf8"),
      );
      base = `http://127.0.0.1:${ports.main_port}`;
      if ((await fetch(base + "/api/info")).ok) break;
    } catch {
      /* Wait for this instance's port file and migrations. */
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  assert.ok(base, "server did not start");
  results.ports = ports;
  const info = await api("/api/info");
  assert.deepEqual(info.data.build_info, {
    revision: expectedSha,
    kind: "development",
    release_tag: null,
    dirty: expectedDirty,
  });
  results.actualBuildInfo = info.data.build_info;
  await api("/api/config", "PUT", {
    ...info.data.config,
    language: "JA",
    theme: "DARK",
    disclaimer_acknowledged: true,
    onboarding_acknowledged: true,
    remote_onboarding_acknowledged: true,
    telemetry_acknowledged: true,
    analytics_enabled: false,
    showcases: { seen_features: ["workspaces-guide"] },
  });
  browser = await chromium.launch({ headless: true });
  page = await browser.newPage({
    viewport: { width: 1024, height: 768 },
  });
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  // Block only external social counters/telemetry, never local application APIs.
  await page.route("**/*", (route) =>
    new URL(route.request().url()).origin === base
      ? route.continue()
      : route.abort(),
  );
  async function check(name, value, expectedDetails, fixture = true) {
    await page.unroute("**/api/info");
    if (fixture)
      await page.route("**/api/info", async (route) => {
        const response = await route.fetch();
        const json = await response.json();
        if (value === undefined) delete json.data.build_info;
        else json.data.build_info = value;
        await route.fulfill({ response, json });
      });
    await page.goto(`${base}/workspaces/create`, { waitUntil: "networkidle" });
    const initialDialog = page.locator('[class*="z-[10000]"]').first();
    if (await initialDialog.isVisible()) {
      results.dismissedDialogs ??= [];
      results.dismissedDialogs.push(await initialDialog.innerText());
      await page.keyboard.press("Escape");
      await expect(initialDialog).toBeHidden();
    }
    const label = page.getByTestId("app-build-info");
    if (value?.revision) {
      await expect(label).toBeVisible();
      await expect(label).toHaveText(value.revision.slice(0, 7));
      const title = await label.getAttribute("title");
      for (const detail of expectedDetails)
        assert.ok(title.includes(detail), title);
      await label.hover();
      const layout = await label.evaluate((el) => ({
        width: el.getBoundingClientRect().width,
        scroll: el.scrollWidth,
        client: el.clientWidth,
        font: getComputedStyle(el).fontSize,
      }));
      assert.ok(layout.width <= 40.5, JSON.stringify(layout));
      assert.ok(
        layout.scroll <= layout.client,
        "seven-character SHA is clipped",
      );
      results.cases.push({ name, fixture, title, layout });
    } else {
      // Wait for the application shell, so absence cannot pass before loading.
      await expect(page.getByRole("button").first()).toBeVisible();
      await expect(label).toHaveCount(0);
      await expect(page.getByText("v0.1.44", { exact: true })).toHaveCount(0);
      results.cases.push({ name, fixture, hidden: true });
    }
    await page.screenshot({ path: path.join(output, `${name}.png`) });
  }
  await check(
    "actual-development-dirty",
    info.data.build_info,
    [
      expectedSha,
      "開発版",
      expectedDirty ? "未コミット変更: あり" : "未コミット変更: なし",
    ],
    false,
  );
  await page
    .getByRole("button", { name: "Local workspaces", exact: true })
    .click();
  await expect(page.getByTestId("app-build-info")).toHaveText(
    expectedSha.slice(0, 7),
  );
  await page.reload({ waitUntil: "networkidle" });
  await expect(page.getByTestId("app-build-info")).toHaveText(
    expectedSha.slice(0, 7),
  );
  const release = {
    revision: expectedSha,
    kind: "release",
    release_tag: "v0.1.50-lvk.1-server.9",
    dirty: false,
  };
  await check("fixture-release-clean", release, [
    expectedSha,
    "リリース版",
    release.release_tag,
    "未コミット変更: なし",
  ]);
  await check("fixture-release-dirty", { ...release, dirty: true }, [
    "未コミット変更: あり",
  ]);
  await check(
    "fixture-partial-unknown",
    { revision: expectedSha, kind: null, release_tag: null, dirty: null },
    ["ビルド種別: 不明", "未コミット変更: 不明"],
  );
  await check("fixture-identity-unknown", null, []);
  await check("fixture-legacy-no-build-info", undefined, []);
  await page.setViewportSize({ width: 820, height: 700 });
  await check(
    "actual-narrow-sidebar",
    info.data.build_info,
    [expectedSha, "開発版"],
    false,
  );
  const latestConfig = (await api("/api/info")).data.config;
  await api("/api/config", "PUT", { ...latestConfig, theme: "LIGHT" });
  await check(
    "actual-light-sidebar",
    info.data.build_info,
    [expectedSha, "開発版"],
    false,
  );
  assert.deepEqual(errors, []);
  results.status = "passed";
} catch (error) {
  results.status = "failed";
  if (page && !page.isClosed()) {
    await page
      .screenshot({ path: path.join(output, "failure.png") })
      .catch(() => {});
    results.failurePageText = await page
      .locator("body")
      .innerText()
      .catch(() => "");
  }
  results.error = error.stack;
  throw error;
} finally {
  if (browser) await browser.close();
  results.cleanup.browserClosed = !browser || !browser.isConnected();
  if (server && server.exitCode === null && server.signalCode === null) {
    const exited = once(server, "exit");
    server.kill("SIGTERM");
    const timeout = setTimeout(() => server.kill("SIGKILL"), 15000);
    await exited;
    clearTimeout(timeout);
  }
  results.cleanup.serverExited =
    !server || server.exitCode !== null || server.signalCode !== null;
  results.cleanup.serverPidGone =
    !server?.pid ||
    (() => {
      try {
        process.kill(server.pid, 0);
        return false;
      } catch (error) {
        return error.code === "ESRCH";
      }
    })();
  results.cleanup.portClosed = {};
  for (const [name, port] of Object.entries(ports || {})) {
    if (typeof port === "number" && port > 0)
      results.cleanup.portClosed[name] = !(await isListening(port));
  }
  await rm(instance, { recursive: true, force: true });
  results.cleanup.temporaryInstanceRemoved = await access(instance).then(
    () => false,
    () => true,
  );
  await writeFile(
    path.join(output, "results.json"),
    JSON.stringify(results, null, 2),
  );
  assert.ok(results.cleanup.serverExited);
  assert.ok(results.cleanup.serverPidGone);
  assert.ok(results.cleanup.browserClosed);
  assert.ok(Object.values(results.cleanup.portClosed).every(Boolean));
  console.log(JSON.stringify(results, null, 2));
}
