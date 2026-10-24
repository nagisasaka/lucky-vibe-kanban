// Real binary/router, built application, private DB and Basic browser login.
// Only resource-observation fault/value cases are intercepted explicitly.
import assert from "node:assert/strict";
import { chromium, expect } from "@playwright/test";
import { spawn, execFileSync } from "node:child_process";
import { createWriteStream } from "node:fs";
import {
  mkdtemp,
  mkdir,
  readFile,
  writeFile,
  rm,
  stat,
} from "node:fs/promises";
import { once } from "node:events";
import http from "node:http";
import https from "node:https";
import net from "node:net";
import { randomBytes } from "node:crypto";
import { tmpdir } from "node:os";
import path from "node:path";
const root = process.cwd();
const output = path.resolve(
  process.env.RESOURCE_APP_EVIDENCE ||
    "test-results/resource-observation/full-app",
);
await mkdir(output, { recursive: true });
const instance = await mkdtemp(path.join(tmpdir(), "lvk-resource-app-"));
const binary = path.join(root, "target/debug/server");
const results = {
  cases: [],
  errors: [],
  consoleErrors: [],
  networkFailures: [],
  scope:
    "real built application and router; disposable DB; loopback TLS and local Basic credentials",
};
let server, proxy, browser, page, backendPort, base, ports;
const sockets = new Set();
const log = createWriteStream(path.join(output, "server.log"));
const env = {
  ...process.env,
  LVK_INSTANCE_DIR: instance,
  HOST: "127.0.0.1",
  BACKEND_PORT: "0",
  PORT: "",
  PREVIEW_PROXY_PORT: "0",
  LVK_ACCESS_DEPLOYMENT: "public",
  LVK_AUTH_MODE: "",
  DISABLE_WORKTREE_CLEANUP: "1",
  BROWSER: "true",
  RUST_LOG: "warn",
  TOKIO_WORKER_THREADS: "1",
};
for (const key of [
  "LVK_MCP_CONFIG",
  "VIBE_BACKEND_URL",
  "VK_ALLOWED_ORIGINS",
  "VK_PREVIEW_DOMAIN",
])
  delete env[key];
const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
try {
  await mkdir(path.join(instance, "runtime/worktrees"), { recursive: true });
  // A private self-signed certificate preserves the real Secure-cookie boundary.
  execFileSync(
    "openssl",
    [
      "req",
      "-x509",
      "-newkey",
      "rsa:2048",
      "-nodes",
      "-keyout",
      path.join(instance, "tls.key"),
      "-out",
      path.join(instance, "tls.crt"),
      "-days",
      "1",
      "-subj",
      "/CN=localhost",
      "-addext",
      "subjectAltName=IP:127.0.0.1",
    ],
    { stdio: "ignore" },
  );
  proxy = https.createServer(
    {
      key: await readFile(path.join(instance, "tls.key")),
      cert: await readFile(path.join(instance, "tls.crt")),
    },
    (req, res) => {
      if (!backendPort) {
        res.writeHead(503).end();
        return;
      }
      const upstream = http.request(
        {
          agent: false,
          hostname: "127.0.0.1",
          port: backendPort,
          path: req.url,
          method: req.method,
          headers: { ...req.headers, "x-forwarded-proto": "https" },
        },
        (r) => {
          res.writeHead(r.statusCode, r.headers);
          r.pipe(res);
        },
      );
      upstream.on("error", () => {
        if (!res.headersSent) res.writeHead(502);
        res.end();
      });
      req.pipe(upstream);
    },
  );
  proxy.keepAliveTimeout = 65000;
  proxy.headersTimeout = 66000;
  proxy.on("connection", (s) => {
    sockets.add(s);
    s.on("close", () => sockets.delete(s));
  });
  proxy.on("upgrade", (req, socket, head) => {
    const upstream = net.connect(backendPort, "127.0.0.1", () => {
      upstream.write(
        `${req.method} ${req.url} HTTP/1.1\r\n${Object.entries(req.headers)
          .map(([k, v]) => `${k}: ${v}`)
          .join("\r\n")}\r\n\r\n`,
      );
      if (head.length) upstream.write(head);
      upstream.pipe(socket);
      socket.pipe(upstream);
    });
    socket.on("close", () => upstream.destroy());
    upstream.on("error", () => socket.destroy());
    socket.on("error", () => upstream.destroy());
  });
  await new Promise((resolve) => proxy.listen(0, "127.0.0.1", resolve));
  base = `https://127.0.0.1:${proxy.address().port}`;
  env.LVK_PUBLIC_URL = base;
  const key = execFileSync(binary, ["setup-token"], {
    env,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();
  server = spawn(binary, [], {
    cwd: root,
    env,
    stdio: ["ignore", "pipe", "pipe"],
  });
  server.stdout.pipe(log, { end: false });
  server.stderr.pipe(log, { end: false });
  for (let i = 0; i < 120; i++) {
    assert.equal(server.exitCode, null, "isolated server exited");
    try {
      ports = JSON.parse(
        await readFile(path.join(instance, "ports/vibe-kanban.port"), "utf8"),
      );
      backendPort = ports.main_port;
      break;
    } catch {
      await delay(500);
    }
  }
  assert.ok(backendPort, "no isolated port file");
  results.ports = ports;
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    ignoreHTTPSErrors: true,
    viewport: { width: 1280, height: 800 },
  });
  await context.route(
    (url) => url.origin !== base,
    (route) => route.abort(),
  );
  const req = context.request;
  async function call(url, method = "GET", body, csrf) {
    const response = await req.fetch(base + url, {
      method,
      headers: { origin: base, ...(csrf ? { "x-lvk-csrf": csrf } : {}) },
      data: body,
    });
    assert.equal(
      response.status(),
      200,
      `${method} ${url}: ${await response.text()}`,
    );
    return response.json();
  }
  const setup = await call("/api/access/setup", "POST", { key }, "setup");
  const username = "resource-test";
  const password = randomBytes(24).toString("hex");
  await call("/api/access/basic", "POST", { username, password }, setup.csrf);
  const anonymous = await req.get(base + "/api/resource-observation");
  assert.equal(anonymous.status(), 401);
  page = await context.newPage();
  page.on("requestfailed", (r) =>
    results.networkFailures.push({
      url: r.url(),
      error: r.failure()?.errorText,
    }),
  );
  page.on("pageerror", (e) => results.errors.push(e.message));
  page.on("console", (m) => {
    if (m.type() === "error") results.consoleErrors.push(m.text());
  });
  let mode = "real",
    actual,
    requests = 0,
    inflight = 0,
    maxInflight = 0;
  await page.route("**/api/resource-observation", async (route) => {
    requests++;
    inflight++;
    maxInflight = Math.max(maxInflight, inflight);
    try {
      if (mode === "failed")
        return await route.fulfill({
          status: 503,
          json: { success: false, message: "test unavailable" },
        });
      if (mode === "delayed") {
        await delay(6000);
        return await route
          .fulfill({ status: 503, json: { success: false } })
          .catch(() => {});
      }
      if (mode === "stale")
        return await route.fulfill({
          json: {
            success: true,
            data: {
              ...actual,
              timestamp: new Date(Date.now() - 44000).toISOString(),
            },
          },
        });
      if (mode === "hundred") {
        const value = structuredClone(actual);
        value.timestamp = new Date().toISOString();
        value.system_visible.cpu_usage.percent = {
          status: "known",
          value: 100,
        };
        value.system_visible.memory_available_bytes = {
          status: "known",
          value: 0,
        };
        for (const d of value.disks) {
          d.observation.value.used_percent = 100;
          d.observation.value.available_bytes = 0;
          d.observation.value.used_bytes = d.observation.value.total_bytes;
        }
        return await route.fulfill({ json: { success: true, data: value } });
      }
      // Let Chromium send the authenticated request through the real router.
      const received = page.waitForResponse(
        (r) => r.url() === route.request().url(),
      );
      await route.continue();
      const response = await received;
      assert.equal(response.status(), 200);
      assert.equal(response.headers()["cache-control"], "no-store");
      actual = (await response.json()).data;
      return;
    } catch (error) {
      results.errors.push(`resource request: ${error.message}`);
      await route.abort().catch(() => {});
    } finally {
      inflight--;
    }
  });
  await page.goto(base);
  await page.getByLabel("ユーザー名", { exact: true }).fill(username);
  await page.getByLabel("パスワード", { exact: true }).fill(password);
  await page.getByRole("button", { name: "ログイン", exact: true }).click();
  await expect
    .poll(async () => (await call("/api/access/status")).authenticated)
    .toBe(true);
  const status = await call("/api/access/status");
  const csrf = status.csrf;
  const read = await req.get(base + "/api/resource-observation");
  assert.equal(read.status(), 200);
  assert.equal(read.headers()["cache-control"], "no-store");
  actual = (await read.json()).data;
  const post = await req.post(base + "/api/resource-observation", {
    headers: { origin: base, "x-lvk-csrf": csrf },
    data: {},
  });
  assert.equal(post.status(), 405);
  results.auth = {
    anonymous: 401,
    login: "real Basic form and session cookie",
    get: 200,
    cacheControl: "no-store",
    post: 405,
  };
  const info = (await call("/api/info")).data;
  await call(
    "/api/config",
    "PUT",
    {
      ...info.config,
      language: "EN",
      theme: "LIGHT",
      disclaimer_acknowledged: true,
      onboarding_acknowledged: true,
      remote_onboarding_acknowledged: true,
      telemetry_acknowledged: true,
      analytics_enabled: false,
      showcases: { seen_features: ["workspaces-guide"] },
    },
    csrf,
  );
  const org = (await call("/api/local/v1/organizations")).organizations[0].id;
  const project = (
    await call(
      "/api/local/v1/projects",
      "POST",
      {
        organization_id: org,
        name: "Resource acceptance",
        color: "210 80% 52%",
      },
      csrf,
    )
  ).data;
  const statuses = (
    await call(
      `/api/local/v1/fallback/project_statuses?project_id=${project.id}`,
    )
  ).project_statuses;
  const issue = (
    await call(
      "/api/local/v1/issues",
      "POST",
      {
        project_id: project.id,
        status_id: statuses[0].id,
        title: "Resource layout",
        sort_order: 100,
        extension_metadata: {},
      },
      csrf,
    )
  ).data;
  const workspace = (
    await call("/api/workspaces", "POST", { name: "resource-acceptance" }, csrf)
  ).data;
  await call(
    "/api/sessions",
    "POST",
    {
      workspace_id: workspace.id,
      executor: "CODEX",
      name: "Idle acceptance session",
    },
    csrf,
  );
  results.fixture = {
    project: project.id,
    issue: issue.id,
    workspace: workspace.id,
  };
  const paths = {
    project: `/projects/${project.id}`,
    subroute: `/projects/${project.id}/issues/${issue.id}`,
    workspace: `/workspaces/${workspace.id}`,
  };
  async function open(url, width) {
    await page.setViewportSize({ width, height: 800 });
    await page.goto(base + url);
    const summary = page.getByTestId("resource-summary");
    await expect(summary).toBeVisible();
    const dialog = page
      .locator('[class*="z-[10000]"]')
      .filter({
        has: page.getByRole("button", {
          name: /Cancel|キャンセル/,
          exact: true,
        }),
      })
      .first();
    if (await dialog.isVisible()) await page.keyboard.press("Escape");
    await expect(page.getByTestId("resource-cpu")).toHaveText(/CPU \d+%/);
  }
  async function layout(name) {
    const nav = page
      .locator("nav")
      .filter({ has: page.getByTestId("resource-summary") });
    const bounds = await nav.evaluate((el) => ({
      width: innerWidth,
      scroll: document.documentElement.scrollWidth,
      navScroll: el.scrollWidth,
      navWidth: el.clientWidth,
      items: [...el.querySelectorAll('button,[data-testid="resource-summary"]')]
        .filter((e) => e.getClientRects().length)
        .map((e) => ({
          label: e.getAttribute("aria-label") || e.textContent,
          left: e.getBoundingClientRect().left,
          right: e.getBoundingClientRect().right,
          width: e.getBoundingClientRect().width,
        })),
    }));
    await page.screenshot({
      path: path.join(output, `${name}-header.png`),
      animations: "disabled",
    });
    await page.getByTestId("resource-disk").focus();
    const tooltip = page.getByRole("tooltip");
    await expect(tooltip).toBeVisible();
    await expect(tooltip).toHaveCSS("opacity", "1");
    const text = await tooltip.innerText();
    assert.doesNotMatch(text, /load|swap/i);
    const tip = await page
      .locator("[data-radix-popper-content-wrapper]")
      .last()
      .boundingBox();
    await page.screenshot({
      path: path.join(output, `${name}.png`),
      animations: "disabled",
    });
    results.cases.push({ name, bounds, tooltip: text, tooltipBounds: tip });
    assert.ok(
      bounds.scroll <= bounds.width && bounds.navScroll <= bounds.navWidth,
      `${name}: overflow ${JSON.stringify(bounds)}`,
    );
    assert.ok(
      bounds.items.every((i) => i.left >= 0 && i.right <= bounds.width),
      `${name}: clipped item ${JSON.stringify(bounds)}`,
    );
    assert.ok(
      tip.x >= 0 && tip.x + tip.width <= bounds.width,
      `${name}: tooltip overflow`,
    );
    await page.keyboard.press("Escape");
  }
  for (const width of [320, 767, 768, 1280])
    for (const [kind, url] of Object.entries(paths)) {
      await open(url, width);
      await layout(`${kind}-${width}`);
      const nav = page
        .locator("nav")
        .filter({ has: page.getByTestId("resource-summary") });
      if (width < 768)
        assert.ok(
          (await nav
            .getByRole("button", { name: "Settings", exact: true })
            .count()) >= 1,
          "real mobile user slot missing",
        );
      if (kind === "project") {
        const integration = nav.getByRole("button", {
          name: /Integrate|Integration/,
        });
        await expect(integration).toBeVisible();
        await integration.click();
        await expect(
          page.getByText("Auto Merge", { exact: true }).last(),
        ).toBeVisible();
        await page.screenshot({
          path: path.join(output, `integration-${width}.png`),
        });
        await page.keyboard.press("Escape");
      }
    }
  // API values from the private instance must reach the unmocked browser tooltip.
  await open(paths.workspace, 1280);
  assert.equal(actual.disks.length, 1);
  assert.equal(actual.disks[0].target, "worktree_base");
  await page.getByTestId("resource-disk").focus();
  const liveText = await page.getByRole("tooltip").innerText();
  for (const d of actual.disks) {
    assert.equal(
      d.observation.value.filesystem_id,
      String((await stat(path.join(instance, "runtime/worktrees"))).dev),
    );
    assert.equal(d.observation.status, "known");
    assert.ok(liveText.includes("Worktree filesystem"));
    await expect(page.getByTestId("resource-disk")).toHaveText(
      `DISK ${Math.floor(d.observation.value.used_percent)}% (${Number((d.observation.value.used_bytes / 1e9).toFixed(2))}GB / ${Number((d.observation.value.total_bytes / 1e9).toFixed(2))}GB)`,
    );
  }
  results.diskWiring = {
    targets: actual.disks.map((d) => ({
      target: d.target,
      filesystem: d.observation.value.filesystem_id,
    })),
    liveTooltip: liveText,
    instance: "private data/ and runtime/worktrees/",
  };
  mode = "hundred";
  await open(paths.project, 320);
  await layout("project-100-percent-320");
  await expect(page.getByTestId("resource-cpu")).toHaveText("CPU 100%");
  await expect(page.getByTestId("resource-cpu")).toHaveClass(/text-error/);
  for (const metric of ["cpu", "ram"]) {
    await expect(page.getByTestId(`resource-${metric}`)).toHaveClass(
      /text-error/,
    );
    await expect(page.getByTestId(`resource-${metric}`)).toContainText("100%");
  }
  await expect(page.getByTestId("resource-disk")).toHaveClass(/text-error/);
  await expect(page.getByTestId("resource-disk")).toContainText("DISK 100%");
  mode = "failed";
  await Promise.all([
    page.waitForResponse((r) => r.url().endsWith("/api/resource-observation")),
    page.reload(),
  ]);
  await expect(page.getByTestId("resource-cpu")).toHaveText("CPU —");
  await expect(page.getByTestId("resource-cpu")).toHaveClass(/text-low/);
  await layout("api-503");
  mode = "delayed";
  await page.reload();
  await delay(5500);
  await expect(page.getByTestId("resource-cpu")).toHaveText("CPU —");
  await expect(page.getByTestId("resource-cpu")).toHaveClass(/text-low/);
  await layout("api-delayed");
  await delay(1000);
  mode = "stale";
  await Promise.all([
    page.waitForResponse((r) => r.url().endsWith("/api/resource-observation")),
    page.reload(),
  ]);
  await expect(page.getByTestId("resource-cpu")).toHaveText(/CPU \d+%/);
  // The real hook's one-second clock must turn a 44-second-old value stale at 45s.
  await expect(page.getByTestId("resource-cpu")).toHaveText("CPU —", {
    timeout: 4000,
  });
  await expect(page.getByTestId("resource-cpu")).toHaveClass(/text-low/);
  await layout("api-stale-45-seconds");
  mode = "real";
  await open(paths.workspace, 1280);
  // Real elapsed polling and actual document visibility (CDP lifecycle alone does
  // not hide a headless tab; override visibility only, leaving all app logic real).
  const beforeHidden = requests;
  await page.evaluate(() => {
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "hidden",
    });
    document.dispatchEvent(new Event("visibilitychange"));
  });
  await delay(17000);
  assert.equal(requests, beforeHidden, "hidden page polled");
  await page.evaluate(() => {
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "visible",
    });
    document.dispatchEvent(new Event("visibilitychange"));
  });
  await expect.poll(() => requests, { timeout: 10000 }).toBe(beforeHidden + 1);
  await expect(page.getByTestId("resource-cpu")).toHaveText(/CPU \d+%/);
  const beforeTransition = requests;
  // Use rendered links for real SPA transitions, preserving the query client.
  await page
    .getByRole("button", { name: "Resource acceptance", exact: true })
    .first()
    .click();
  await page.waitForURL(base + paths.project);
  await page
    .getByRole("button", { name: "Local workspaces", exact: true })
    .click();
  await expect(page.getByTestId("resource-summary")).toBeVisible();
  await delay(1000);
  assert.equal(
    requests,
    beforeTransition,
    "SPA remount duplicated a fresh query",
  );
  assert.equal(maxInflight, 1);
  results.polling = {
    hiddenStopped: true,
    visibleResumed: true,
    spaTransitionNoDuplicate: true,
    maxInflight,
  };
  assert.deepEqual(results.errors, []);
  results.status = "passed";
} catch (error) {
  results.status = "failed";
  results.error = error.stack;
  if (page && !page.isClosed()) {
    const details = page.getByRole("button", {
      name: "Show details",
      exact: true,
    });
    if (await details.isVisible().catch(() => false)) await details.click();
    await page
      .screenshot({ path: path.join(output, "failure.png") })
      .catch(() => {});
    results.pageText = await page
      .locator("body")
      .innerText()
      .catch(() => "");
  }
  throw error;
} finally {
  await browser?.close();
  if (server && server.exitCode === null && server.signalCode === null) {
    const exited = once(server, "exit");
    server.kill("SIGTERM");
    const deadline = setTimeout(() => server.kill("SIGKILL"), 15000);
    await exited;
    clearTimeout(deadline);
  }
  for (const socket of sockets) socket.destroy();
  if (proxy) await new Promise((resolve) => proxy.close(resolve));
  log.end();
  await rm(instance, { recursive: true, force: true });
  results.cleanup = {
    browserClosed: !browser?.isConnected(),
    serverExited:
      !server || server.exitCode !== null || server.signalCode !== null,
    tlsClosed: !proxy?.listening,
    privateInstanceRemoved: true,
  };
  await writeFile(
    path.join(output, "results.json"),
    JSON.stringify(results, null, 2),
  );
  console.log(JSON.stringify(results, null, 2));
}
