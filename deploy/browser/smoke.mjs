// Also used by CI/the host updater against a disposable app database.
import assert from "node:assert/strict";
import { readFile, mkdir } from "node:fs/promises";
import { chromium } from "playwright";

const url = process.env.LVK_E2E_BASE_URL;
if (!url) throw new Error("Set LVK_E2E_BASE_URL to the application to verify");
const credentials = process.env.LVK_E2E_CREDENTIALS_FILE
  ? JSON.parse(await readFile(process.env.LVK_E2E_CREDENTIALS_FILE, "utf8"))
  : undefined;
const internalFile = process.env.LVK_TEST_INTERNAL_TOKEN_FILE;
const base = new URL(url);
if (
  internalFile &&
  !(
    base.protocol === "http:" &&
    ["127.0.0.1", "localhost", "[::1]"].includes(base.hostname)
  )
)
  throw new Error("Internal fixture capability is loopback-only");
const internalToken = internalFile
  ? (await readFile(internalFile, "utf8")).trim()
  : undefined;
const browser = await chromium.launch();
// A cold Vite checkout can take longer than a bundled release. Bound the whole
// probe while still closing Chromium before the host command's timeout.
const watchdog = setTimeout(
  () => void browser.close().catch(() => {}),
  150_000,
);
try {
  const context = await browser.newContext({
    httpCredentials: credentials
      ? {
          username: credentials.username,
          password: credentials.password,
          origin: new URL(url).origin,
        }
      : undefined,
  });
  if (internalToken) {
    // A trusted local fixture transport, not an end-user Web-login E2E claim.
    await context.route("**/*", async (route) => {
      if (new URL(route.request().url()).origin !== base.origin)
        return route.continue();
      const headers = {
        ...route.request().headers(),
        "x-lvk-internal-token": internalToken,
      };
      delete headers.origin;
      // Chromium can restore Origin on module/CORS requests after continue().
      // Fetch this loopback-only fixture request outside the browser instead;
      // never follow a redirect with the internal capability attached.
      const response = await route.fetch({ headers, maxRedirects: 0 });
      await route.fulfill({ response });
    });
  }
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(message.text());
  });
  const response = await page.goto(url, {
    waitUntil: "commit",
    timeout: 30_000,
  });
  assert.equal(response?.status(), 200);
  assert.match(await page.title(), /Lucky Vibe Kanban/i);
  try {
    await page.waitForFunction(
      () => document.querySelector("#root")?.textContent?.trim(),
      undefined,
      { timeout: 120_000 },
    );
  } catch (error) {
    throw new Error(`Application did not render: ${JSON.stringify(errors)}`, {
      cause: error,
    });
  }
  await page
    .getByRole("button")
    .first()
    .waitFor({ state: "visible", timeout: 120_000 });
  if (credentials && !internalToken) {
    const login = await context.request.post(
      new URL("/api/access/basic-login", url).href,
      {
        headers: {
          "x-lvk-csrf": "basic",
          Authorization: `Basic ${Buffer.from(`${credentials.username}:${credentials.password}`).toString("base64")}`,
        },
      },
    );
    assert.equal(login.status(), 200);
    await page.reload();
  }
  const info = await context.request.get(new URL("/api/info", url).href, {
    headers: internalToken
      ? { "x-lvk-internal-token": internalToken }
      : undefined,
  });
  assert.equal(info.status(), 200);
  assert.equal((await info.json()).success, true);
  assert.deepEqual(errors, [], "Browser JavaScript errors");
  if (process.env.LVK_E2E_OUTPUT_DIR) {
    await mkdir(process.env.LVK_E2E_OUTPUT_DIR, { recursive: true });
    await page.screenshot({
      path: `${process.env.LVK_E2E_OUTPUT_DIR}/application.png`,
      fullPage: true,
    });
  }
  console.log(
    JSON.stringify({
      browser: browser.version(),
      title: await page.title(),
      url,
      errors,
    }),
  );
} finally {
  clearTimeout(watchdog);
  await browser.close();
}
