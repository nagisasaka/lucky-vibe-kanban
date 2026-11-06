import { chromium } from "@playwright/test";
import { readFile, writeFile, unlink } from "node:fs/promises";
const base = "https://preview2.luckyvibekanban.com";
const credentials = JSON.parse(
  await readFile(
    new URL("../.build/design-preview/credentials.json", import.meta.url),
  ),
);
const response = await fetch(base + "/api/access/status");
const status = await response.json();
if (
  status.mode !== "basic" ||
  status.public_url !== base ||
  status.authenticated
)
  throw Error("Unexpected preview authentication");
const foreign = await fetch(base + "/api/access/status", {
  headers: { Origin: "https://untrusted.invalid" },
});
if (foreign.status !== 403) throw Error("Foreign origin accepted");
const browser = await chromium.launch({
  headless: true,
  args: ["--no-sandbox"],
});
try {
  const context = await browser.newContext();
  const page = await context.newPage();
  const initialRequests = [];
  const trackInitial = (request) =>
    initialRequests.push(new URL(request.url()).pathname);
  page.on("request", trackInitial);
  const connection = await context.newCDPSession(page);
  await connection.send("Network.enable");
  await connection.send("Network.emulateNetworkConditions", {
    offline: false,
    latency: 75,
    downloadThroughput: 1250000,
    uploadThroughput: 1250000,
  });
  const loginStarted = Date.now();
  await page.goto(base, { waitUntil: "domcontentloaded" });
  await page
    .locator('input[autocomplete="username"]')
    .waitFor({ state: "visible", timeout: 30000 });
  page.off("request", trackInitial);
  if (
    initialRequests.some((path) =>
      /\/app\/(?:entry\/App\.tsx|router\/)/.test(path),
    )
  ) {
    throw Error("Application/router loaded before authentication");
  }
  console.log(
    `Cold login visible in ${Date.now() - loginStarted}ms with 75ms latency; application/router deferred`,
  );
  await connection.send("Network.emulateNetworkConditions", {
    offline: false,
    latency: 0,
    downloadThroughput: -1,
    uploadThroughput: -1,
  });
  await page
    .locator('input[autocomplete="username"]')
    .fill(credentials.username);
  await page
    .locator('input[autocomplete="current-password"]')
    .fill(credentials.password);
  await page
    .locator("form")
    .filter({ has: page.locator('input[autocomplete="username"]') })
    .locator('button[type="submit"], button')
    .click();
  await page.waitForFunction(
    async () => {
      const status = await (await fetch("/api/access/status")).json();
      return status.mode === "basic" && status.authenticated && !status.limited;
    },
    undefined,
    { timeout: 30000 },
  );
  await page
    .locator('input[autocomplete="username"]')
    .waitFor({ state: "detached" });
  await page.reload({ waitUntil: "domcontentloaded" });
  const auth = await page.evaluate(async () =>
    (await fetch("/api/access/status")).json(),
  );
  if (!auth.authenticated) {
    console.log({
      mode: auth.mode,
      authenticated: auth.authenticated,
      limited: auth.limited,
    });
    throw Error("Browser session did not survive reload");
  }
  const cookies = await context.cookies();
  if (
    !cookies.some(
      (cookie) =>
        cookie.name === "lvk_session" &&
        cookie.domain === "preview2.luckyvibekanban.com" &&
        cookie.secure &&
        cookie.httpOnly,
    )
  )
    throw Error("Session cookie boundary invalid");
  await page.waitForFunction(
    () => {
      const text = document.body.innerText.trim();
      return text.length > 30 && !/^Loading[.…]*$/.test(text);
    },
    undefined,
    { timeout: 60000 },
  );
  console.log("Authenticated application rendered");
  const canaryName = `preview-hmr-canary-${process.pid}.js`;
  const canary = new URL(
    `../packages/local-web/src/${canaryName}`,
    import.meta.url,
  );
  const source = (value) =>
    `window.__previewHmrCanary = ${value}; if (import.meta.hot) import.meta.hot.accept();`;
  try {
    await writeFile(canary, source(1), { flag: "wx" });
    await page.evaluate((name) => import(`/src/${name}`), canaryName);
    const timeOrigin = await page.evaluate(() => performance.timeOrigin);
    await writeFile(canary, source(2));
    await page.waitForFunction(() => window.__previewHmrCanary === 2);
    if ((await page.evaluate(() => performance.timeOrigin)) !== timeOrigin)
      throw Error("HMR reloaded page");
    console.log("Public source update applied by HMR without page reload");
  } finally {
    await unlink(canary);
  }
  await page.screenshot({
    path: ".build/design-preview/logged-in.png",
    fullPage: true,
  });
  console.log(
    "Public browser Basic login, session reload, preview-only Secure/HttpOnly cookie and foreign-origin rejection passed",
  );
} finally {
  await browser.close();
}
const client = await (await fetch(base + "/@vite/client")).text();
const token = client.match(/const wsToken = "([^"]+)"/)?.[1];
if (!token) throw Error("HMR token missing");
await new Promise((resolve, reject) => {
  const ws = new WebSocket(
    base.replace("https:", "wss:") + "/?token=" + encodeURIComponent(token),
    "vite-hmr",
  );
  const timer = setTimeout(() => {
    ws.close();
    reject(Error("HMR timeout"));
  }, 10000);
  ws.onmessage = ({ data }) => {
    if (JSON.parse(data).type === "connected") {
      clearTimeout(timer);
      ws.close();
      resolve();
    }
  };
  ws.onerror = () => {
    clearTimeout(timer);
    reject(Error("HMR connection failed"));
  };
});
console.log("Public HMR WebSocket connected");
