// Actual storage UI with intercepted APIs only. Never accesses a live LVK instance.
import assert from "node:assert/strict";
import { build } from "esbuild";
import { chromium, expect } from "@playwright/test";
import { createServer } from "node:http";
import { mkdtemp, readFile, rm, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { execFileSync } from "node:child_process";
const root = process.cwd();
const scratch = await mkdtemp(path.join(tmpdir(), "lvk-348-ui-"));
let browser, server;
try {
  const js = await build({
    entryPoints: ["tests/shared-storage/browser.fixture.tsx"],
    bundle: true,
    write: false,
    format: "iife",
    jsx: "automatic",
    define: { "import.meta.env.DEV": "false", "import.meta.hot": "undefined" },
    alias: {
      "@/shared/dialogs/settings/SettingsDialog": path.join(
        root,
        "tests/pets/settings-dialog.fixture.ts",
      ),
      "@": path.join(root, "packages/web-core/src"),
    },
    nodePaths: [path.join(root, "packages/web-core/node_modules")],
    loader: {
      ".woff2": "dataurl",
      ".woff": "dataurl",
      ".ttf": "dataurl",
      ".svg": "dataurl",
    },
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
    res.setHeader(
      "Content-Type",
      req.url === "/app.js"
        ? "text/javascript"
        : req.url === "/style.css"
          ? "text/css"
          : "text/html",
    );
    res.end(
      req.url === "/app.js"
        ? js.outputFiles[0].text
        : req.url === "/style.css"
          ? css
          : '<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/style.css"><div id="root"></div><script src="/app.js"></script>',
    );
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const base = `http://127.0.0.1:${server.address().port}`;
  browser = await chromium.launch({
    headless: true,
    executablePath: process.env.LVK_E2E_CHROMIUM,
  });
  const page = await browser.newPage({
    viewport: { width: 1024, height: 1000 },
  });
  const errors = [],
    requests = [],
    deletes = [];
  let releaseMeasurements;
  const measurementsReady = new Promise((resolve) => {
    releaseMeasurements = resolve;
  });
  let holdMeasurements = true;
  let failDelete = false,
    failRead = false,
    resourcePolls = 0;
  const sizes = {
    one: {
      cache: { bytes: 2097152, error: null },
      tmp: { bytes: 1024, error: null },
    },
    two: {
      cache: { bytes: 0, error: null },
      tmp: { bytes: null, error: "fixture denied" },
    },
  };
  page.on("pageerror", (e) => errors.push(e.message));
  await page.route("**/api/**", async (route) => {
    const url = new URL(route.request().url());
    const match =
      /^\/api\/repos\/(one|two)\/shared-storage(?:\/(cache|tmp))?$/.exec(
        url.pathname,
      );
    let data = {},
      status = 200;
    if (match) {
      const [, id, kind] = match;
      if (kind) {
        deletes.push([id, kind]);
        if (failDelete) status = 409;
        else sizes[id][kind] = { bytes: 0, error: null };
      } else {
        requests.push(id);
        if (failRead) status = 503;
      }
      if (!kind && holdMeasurements) await measurementsReady;
      data = sizes[id];
    } else if (/\/api\/repos\//.test(url.pathname)) {
      data = { display_name: `Repository ${url.pathname.split("/").at(-1)}` };
    } else if (url.pathname === "/api/resource-observation") {
      resourcePolls++;
    } else if (url.pathname === "/api/environment-info") {
      data = {
        hostname: "Fixture",
        observed_at: new Date().toISOString(),
        running_sessions: 0,
      };
    }
    await route.fulfill({
      status,
      contentType: "application/json",
      body: JSON.stringify({ success: status === 200, data }),
    });
  });
  await page.goto(base);
  await expect(page.getByTestId("environment-hostname")).toContainText(
    "Fixture",
  );
  assert.equal(requests.length, 0, "closed DISK must not measure storage");
  const disk = page.getByRole("button", { name: /DISK/ });
  await disk.click();
  await expect(page.getByText("計測中…")).toHaveCount(4);
  assert.equal(
    await page.getByText("0 B").count(),
    0,
    "unmeasured must not look empty",
  );
  holdMeasurements = false;
  releaseMeasurements();
  await expect(page.getByText("2.0 MiB")).toBeVisible();
  await expect(page.getByText("取得失敗")).toBeVisible();
  assert.deepEqual(
    requests.sort(),
    ["one", "two"],
    "duplicate repository IDs measured once",
  );
  assert.equal(
    await page
      .getByText("このレポジトリを使う他のプロジェクトにも影響します。")
      .count(),
    0,
  );
  await expect.poll(() => resourcePolls, { timeout: 15000 }).toBeGreaterThan(1);
  assert.equal(requests.length, 2, "resource polling must not rescan storage");
  await disk.click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.getByText("設定を切替").click();
  await expect(
    page.getByText("このレポジトリを使う他のプロジェクトにも影響します。"),
  ).toBeVisible();
  await page.getByRole("button", { name: "cacheを削除", exact: true }).click();
  assert.equal(deletes.length, 0, "first click is confirmation only");
  await page.getByRole("button", { name: "キャンセル", exact: true }).click();
  assert.equal(deletes.length, 0);
  await page.getByRole("button", { name: "tmpを削除", exact: true }).click();
  await page
    .getByRole("button", { name: "中身を削除する", exact: true })
    .click();
  await expect(
    page.getByText("削除が完了しました。", { exact: true }),
  ).toBeVisible();
  assert.deepEqual(deletes, [["one", "tmp"]]);
  await disk.click();
  await expect(page.getByText("0 B")).toHaveCount(3); // settings tmp + sidebar tmp and repo two cache
  await disk.click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  failDelete = true;
  await page.getByRole("button", { name: "cacheを削除", exact: true }).click();
  await page
    .getByRole("button", { name: "中身を削除する", exact: true })
    .click();
  await expect(page.getByRole("alert")).toContainText("削除に失敗");
  await expect(page.getByText("2.0 MiB")).toBeVisible();
  failRead = true;
  await page.getByRole("button", { name: "容量を更新", exact: true }).click();
  await expect(page.getByText("取得失敗")).toHaveCount(2);
  await page.getByText("Repositoryを切替").click();
  await expect(
    page.getByRole("button", { name: "中身を削除する", exact: true }),
  ).toHaveCount(0);
  failRead = false;
  await page.getByRole("button", { name: "容量を更新", exact: true }).click();
  await expect(page.getByText("0 B")).toBeVisible();
  const out = path.join(root, "test-results/shared-storage");
  await mkdir(out, { recursive: true });
  for (const [width, dark] of [
    [1024, false],
    [390, true],
  ]) {
    await page.setViewportSize({ width, height: 1000 });
    await page.evaluate(
      (dark) => document.documentElement.classList.toggle("dark", dark),
      dark,
    );
    assert(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
      "no horizontal overflow",
    );
    await page.screenshot({
      path: path.join(out, `${width}-${dark ? "dark" : "light"}.png`),
      fullPage: true,
      animations: "disabled",
    });
  }
  assert.deepEqual(errors, []);
  console.log(
    "PASS: on-demand/deduplicated sizes, polling isolation, settings-only impact, confirmation/cancel, fixed target, deletion refresh/failure, read failure, repo switch, light/dark desktop/mobile",
  );
} finally {
  await browser?.close();
  if (server) await new Promise((resolve) => server.close(resolve));
  await rm(scratch, { recursive: true, force: true });
}
