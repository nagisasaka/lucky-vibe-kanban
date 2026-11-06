import { chromium, expect } from "@playwright/test";
import { randomUUID } from "node:crypto";
import { writeFile, unlink } from "node:fs/promises";
const base = "https://preview2.luckyvibekanban.com";
const name = `hmr-proof-${randomUUID()}.js`;
const source = new URL(`../packages/local-web/src/${name}`, import.meta.url);
const browser = await chromium.launch({
  headless: true,
  args: ["--no-sandbox"],
});
let created = false;
try {
  const page = await browser.newPage();
  const frames = [];
  page.on("websocket", (socket) =>
    socket.on("framereceived", ({ payload }) => {
      try {
        frames.push(JSON.parse(String(payload)));
      } catch {
        /* Non-JSON transport frame. */
      }
    }),
  );
  await page.route(base + "/__lvk_hmr_proof", (route) =>
    route.fulfill({
      contentType: "text/html",
      body: '<!doctype html><title>Isolated HMR proof</title><script type="module" src="/@vite/client"></script>',
    }),
  );
  await writeFile(
    source,
    "window.__lvkHmrProof = 1; if (import.meta.hot) import.meta.hot.accept();",
    { flag: "wx" },
  );
  created = true;
  await page.goto(base + "/__lvk_hmr_proof");
  await expect
    .poll(() => frames.some((frame) => frame.type === "connected"))
    .toBe(true);
  await page.evaluate((name) => import(`/src/${name}`), name);
  const origin = await page.evaluate(() => performance.timeOrigin);
  await writeFile(
    source,
    "window.__lvkHmrProof = 2; if (import.meta.hot) import.meta.hot.accept();",
  );
  await page.waitForFunction(() => window.__lvkHmrProof === 2);
  expect(await page.evaluate(() => performance.timeOrigin)).toBe(origin);
  expect(frames.some((frame) => frame.type === "update")).toBe(true);
  console.log(
    "Existing public Vite WSS received a source-update frame and applied it without reloading; no API/data operations or server restart",
  );
} finally {
  await browser.close();
  if (created) await unlink(source);
}
