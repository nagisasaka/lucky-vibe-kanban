// Isolated component/API fixtures only: never connects to a running LVK server.
import assert from "node:assert/strict";
import { build } from "esbuild";
import { chromium, expect } from "@playwright/test";
import { createServer } from "node:http";
import { mkdtemp, mkdir, readFile, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { execFileSync } from "node:child_process";
const root = process.cwd();
const output = path.resolve(
  process.env.RESOURCE_EVIDENCE_DIR || "test-results/resource-observation",
);
await mkdir(output, { recursive: true });
const scratch = await mkdtemp(path.join(tmpdir(), "lvk-resources-"));
let browser, server;
const results = { cases: [], errors: [] };
const known = (value) => ({ status: "known", value });
const fixture = (cpu = 42, ram = 42, disk = 42) => ({
  timestamp: new Date().toISOString(),
  system_visible: {
    memory_total_bytes: known(1000),
    memory_available_bytes: known(1000 - ram * 10),
    cpu_usage: { percent: known(cpu), sampling_duration_ms: 501 },
  },
  disks: [
    {
      target: "worktree_base",
      observation: known({
        used_percent: disk,
        total_bytes: 1000e9,
        used_bytes: disk * 10e9,
        available_bytes: (100 - disk) * 5e9,
        filesystem_id: "202",
      }),
    },
  ],
});
try {
  const js = await build({
    entryPoints: ["tests/resource-observation/browser.fixture.tsx"],
    bundle: true,
    write: false,
    format: "iife",
    jsx: "automatic",
    alias: { "@": path.join(root, "packages/web-core/src") },
    nodePaths: [path.join(root, "packages/web-core/node_modules")],
  });
  execFileSync(
    path.join(root, "packages/local-web/node_modules/.bin/tailwindcss"),
    [
      "-c",
      "tailwind.new.config.js",
      "-i",
      "../web-core/src/app/styles/new/index.css",
      "-o",
      path.join(scratch, "style.css"),
    ],
    { cwd: path.join(root, "packages/local-web"), stdio: "pipe" },
  );
  const css = await readFile(path.join(scratch, "style.css"));
  server = createServer((req, res) => {
    if (req.url === "/app.js") {
      res.setHeader("Content-Type", "text/javascript");
      res.end(js.outputFiles[0].text);
    } else if (req.url === "/style.css") {
      res.setHeader("Content-Type", "text/css");
      res.end(css);
    } else {
      res.setHeader("Content-Type", "text/html");
      res.end(
        '<!doctype html><html><head><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/style.css"></head><body><div id="root"></div><script src="/app.js"></script></body></html>',
      );
    }
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const base = `http://127.0.0.1:${server.address().port}`;
  browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({
    viewport: { width: 1024, height: 360 },
  });
  page.on("pageerror", (e) => results.errors.push(e.message));
  let data = fixture(),
    fail = false,
    requests = 0;
  await page.route("**/*", async (route) => {
    const url = new URL(route.request().url());
    if (url.origin !== base) return route.abort();
    if (url.pathname === "/api/resource-observation") {
      requests++;
      return route.fulfill({
        status: fail ? 503 : 200,
        contentType: "application/json",
        body: JSON.stringify({ success: !fail, data }),
      });
    }
    return route.continue();
  });
  async function check(
    name,
    values,
    color,
    { width = 1024, dark = false, project = false } = {},
  ) {
    data = values;
    fail = name === "failed";
    requests = 0;
    await page.setViewportSize({ width, height: 360 });
    await page.goto(base + (project ? "/?project=1" : ""));
    await page.evaluate(
      (dark) => document.documentElement.classList.toggle("dark", dark),
      dark,
    );
    const cpu = page.getByTestId("resource-cpu");
    await expect(cpu).toHaveClass(new RegExp(color));
    await expect(cpu).toHaveText(/CPU (\d+%|—)/);
    await expect.poll(() => requests).toBe(1);
    assert.equal(requests, 1, "two subscribers must share a single fetch");
    const summary = page.getByTestId("resource-summary");
    assert.doesNotMatch(await summary.innerText(), /load|swap/i);
    await page.screenshot({
      path: path.join(output, `${name}-header.png`),
      animations: "disabled",
    });
    const navItems = await page.locator("nav button").evaluateAll((items) =>
      items
        .filter((el) => el.getClientRects().length)
        .map((el) => ({
          left: el.getBoundingClientRect().left,
          right: el.getBoundingClientRect().right,
        })),
    );
    assert.ok(
      navItems.every((item) => item.left >= 0 && item.right <= width),
      `${name}: clipped navbar icon`,
    );
    await page.getByTestId("resource-disk").focus();
    const tooltip = page.getByRole("tooltip");
    await expect(tooltip).toBeVisible();
    await expect(tooltip).toHaveCSS("opacity", "1");
    assert.doesNotMatch(await tooltip.innerText(), /load|swap/i);
    if (!["failed", "stale", "missing"].includes(name)) {
      await expect(tooltip).toContainText(
        "Worktree filesystem: used / total capacity",
      );
      const disk = values.disks[0].observation.value;
      await expect(page.getByTestId("resource-disk")).toHaveText(
        `DISK ${Math.floor(disk.used_percent)}% (${Number((disk.used_bytes / 1e9).toFixed(2))}GB / 1000GB)`,
      );
    } else {
      await expect(page.getByTestId("resource-disk")).toHaveText(
        "DISK — (— / —)",
      );
      await expect(page.getByTestId("resource-disk")).toHaveClass(/text-low/);
    }
    await page.screenshot({
      path: path.join(output, `${name}.png`),
      animations: "disabled",
    });
    const layout = await summary.evaluate((el) => ({
      right: el.getBoundingClientRect().right,
      width: innerWidth,
      scroll: document.documentElement.scrollWidth,
    }));
    assert.ok(
      layout.right <= width && layout.scroll <= width,
      JSON.stringify(layout),
    );
    results.cases.push({ name, layout, requests });
  }
  await check("capacity-90", fixture(42, 42, 90), "text-success");
  await check("narrow-100", fixture(100, 100, 100), "text-error", {
    width: 320,
    project: true,
  });
  await check("normal", fixture(), "text-success");
  await check("warning-70", fixture(70, 70, 70), "text-orange-500");
  await check("high-80-dark", fixture(80, 80, 80), "text-error", {
    dark: true,
  });
  await check("below-70", fixture(69.99, 69.99, 69.99), "text-success");
  await expect(page.getByTestId("resource-cpu")).toHaveText("CPU 69%");
  await check("below-80", fixture(79.99, 79.99, 79.99), "text-orange-500");
  await check("failed", fixture(), "text-low");
  await check(
    "stale",
    { ...fixture(), timestamp: new Date(Date.now() - 46000).toISOString() },
    "text-low",
  );
  await check("missing", { timestamp: new Date().toISOString() }, "text-low");
  await check("narrow-320", fixture(80, 70, 42), "text-error", { width: 320 });
  await check("narrow-project-320", fixture(80, 70, 42), "text-error", {
    width: 320,
    project: true,
  });
  await check("desktop-boundary-768", fixture(100, 100, 100), "text-error", {
    width: 768,
  });
  // Deterministic wall-clock and visibility tests exercise the real hook.
  await page.clock.install();
  data = fixture();
  fail = false;
  requests = 0;
  await page.goto(base);
  await expect(page.getByTestId("resource-cpu")).toHaveText("CPU 42%");
  assert.equal(requests, 1);
  await page.clock.runFor(15000);
  await expect.poll(() => requests).toBe(2);
  await page.evaluate(() => {
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "hidden",
    });
    document.dispatchEvent(new Event("visibilitychange"));
  });
  await page.clock.runFor(60000);
  assert.equal(requests, 2, "hidden document must not poll");
  await page.evaluate(() => {
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "visible",
    });
    document.dispatchEvent(new Event("visibilitychange"));
  });
  await expect.poll(() => requests).toBe(3);
  await expect(page.getByTestId("resource-cpu")).toHaveText("CPU —");
  results.cases.push({ name: "visibility-and-clock-stale", requests });
  // A slow fetch remains shared between the two subscribers.
  let release;
  await page.unroute("**/*");
  let slowRequests = 0;
  await page.route("**/*", async (route) => {
    const url = new URL(route.request().url());
    if (url.origin !== base) return route.abort();
    if (url.pathname !== "/api/resource-observation") return route.continue();
    slowRequests++;
    await new Promise((resolve) => {
      release = resolve;
    });
    await route.fulfill({ json: { success: true, data: fixture() } });
  });
  await page.reload();
  await expect.poll(() => slowRequests).toBe(1);
  await page.clock.runFor(3000);
  assert.equal(slowRequests, 1);
  release();
  await expect(page.getByTestId("resource-cpu")).toHaveText("CPU —"); // fixture timestamp is real time, browser clock advanced
  results.cases.push({
    name: "inflight-deduplication",
    requests: slowRequests,
  });
  assert.deepEqual(results.errors, []);
  results.status = "passed";
} catch (error) {
  results.status = "failed";
  results.error = error.stack;
  throw error;
} finally {
  await browser?.close();
  if (server) await new Promise((resolve) => server.close(resolve));
  await rm(scratch, { recursive: true, force: true });
  results.cleanup = {
    browserClosed: !browser?.isConnected(),
    serverClosed: !server?.listening,
    scratchRemoved: true,
  };
  await writeFile(
    path.join(output, "results.json"),
    JSON.stringify(results, null, 2),
  );
  console.log(JSON.stringify(results, null, 2));
}
