// Production asset smoke tests. APIs are explicit fixtures, not a real Auth0 E2E.
import { chromium } from "@playwright/test";
import assert from "node:assert/strict";
import http from "node:http";
import { readFile, stat } from "node:fs/promises";
import path from "node:path";
const root = path.resolve("packages/local-web/dist");
const contentType = {
  ".js": "application/javascript",
  ".css": "text/css",
  ".html": "text/html",
  ".svg": "image/svg+xml",
  ".woff2": "font/woff2",
};
const server = http.createServer(async (req, res) => {
  try {
    let file = path.resolve(
      root,
      `.${new URL(req.url, "http://localhost").pathname}`,
    );
    if (
      !file.startsWith(root + path.sep) ||
      !(await stat(file).catch(() => null))?.isFile()
    )
      file = path.join(root, "index.html");
    res.setHeader(
      "content-type",
      contentType[path.extname(file)] ?? "application/octet-stream",
    );
    res.end(await readFile(file));
  } catch {
    res.writeHead(500);
    res.end();
  }
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
let browser;
try {
  browser = await chromium.launch({ headless: true, args: ["--no-sandbox"] });
  const page = await browser.newPage({
    viewport: { width: 1100, height: 950 },
  });
  const origin = `http://127.0.0.1:${server.address().port}`;
  const status = {
    mode: "locked",
    deployment: "public",
    authenticated: false,
    management: false,
    limited: false,
    csrf: null,
    public_url: "https://203.0.113.7:8443",
    forced_mode: null,
    configuration_error: false,
    user: null,
  };
  const business = [];
  const calls = [];
  await page.route("**/api/**", async (route) => {
    const req = route.request();
    const url = new URL(req.url());
    calls.push({
      path: url.pathname,
      method: req.method(),
      headers: req.headers(),
      body: req.postData(),
    });
    if (!url.pathname.startsWith("/api/access/")) {
      business.push(url.pathname);
      await route.fulfill({ status: 423, json: { error: "locked" } });
      return;
    }
    let response = {};
    switch (url.pathname) {
      case "/api/access/status":
        response = status;
        break;
      case "/api/access/setup":
        assert.equal(req.headers()["x-lvk-csrf"], "setup");
        assert.equal(req.postDataJSON().key, "one-time-test-key");
        Object.assign(status, {
          management: true,
          limited: true,
          csrf: "test-browser-csrf",
        });
        response = { csrf: status.csrf };
        break;
      case "/api/access/configuration":
        response = {
          current: { mode: null },
          public_url: status.public_url,
          callback_url: `${status.public_url}/api/access/callback`,
          logout_url: `${status.public_url}/`,
          resource: `${status.public_url}/mcp`,
          scopes: ["lvk:read", "lvk:write", "lvk:merge"],
        };
        break;
      case "/api/access/confirmation":
        response = { ready: false };
        break;
      case "/api/access/auth0/test":
        assert.equal(req.headers()["x-lvk-csrf"], "test-browser-csrf");
        assert.equal(req.postDataJSON().client_secret, "web-only-test-secret");
        response = {
          authorize_url: "https://tenant.jp.auth0.com/authorize?state=fixture",
        };
        break;
      default:
        await route.fulfill({
          status: 404,
          json: { error: "unexpected fixture route" },
        });
        return;
    }
    await route.fulfill({ json: response });
  });
  await page.goto(origin);
  await page.getByRole("heading", { name: "LVK ログイン・初期設定" }).waitFor();
  await page
    .getByLabel("セットアップキー", { exact: true })
    .fill("one-time-test-key");
  await page.getByRole("button", { name: "設定を開始", exact: true }).click();
  await page.getByRole("combobox").waitFor();
  assert.deepEqual(
    await page.getByRole("combobox").locator("option").allTextContents(),
    ["Basic（単一所有者）", "Auth0（承認されたメンバー）"],
  );
  assert.equal(
    await page
      .getByLabel("WebアプリのAllowed Callback URL", { exact: true })
      .inputValue(),
    "https://203.0.113.7:8443/api/access/callback",
  );
  await page
    .getByLabel("Auth0 issuer（末尾 / を含む）", { exact: true })
    .fill("https://tenant.jp.auth0.com/");
  await page.getByLabel("Web用Client ID", { exact: true }).fill("web-client");
  await page
    .getByLabel("Web用Client Secret", { exact: true })
    .fill("web-only-test-secret");
  await page.screenshot({ path: ".build/access-settings.png", fullPage: true });
  await page.route("https://tenant.jp.auth0.com/**", (route) =>
    route.fulfill({
      contentType: "text/html",
      body: "Mock authorization screen",
    }),
  );
  await page
    .getByRole("button", { name: "設定をチェックしてログイン試験" })
    .click();
  await page.waitForURL("https://tenant.jp.auth0.com/**");
  assert.equal(
    business.length,
    0,
    "business providers must not mount before authentication",
  );
  assert(
    calls.every(
      (c) =>
        !c.path.includes("one-time-test-key") &&
        !c.path.includes("web-only-test-secret"),
    ),
  );
  await page.goto(`${origin}/?access=recovery`);
  await page.getByLabel("復旧キー", { exact: true }).waitFor();
  assert.equal(
    await page.evaluate(
      () =>
        Object.keys(localStorage).filter((k) => /token|secret|auth0/i.test(k))
          .length,
    ),
    0,
  );
  console.log(
    "PASS: locked setup, limited recovery, public mode restriction, exact IP callback, CSRF, no pre-auth business API, no credential storage; mock browser fixtures only",
  );
} finally {
  await browser?.close();
  await new Promise((resolve) => server.close(resolve));
}
