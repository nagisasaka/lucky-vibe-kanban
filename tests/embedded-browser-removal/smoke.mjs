// Called only by lifecycle.py, against that script's newly created local instance.
import assert from "node:assert/strict";
import { chromium } from "@playwright/test";
const base = new URL(process.env.LVK_TEST_BASE_URL);
assert.equal(base.hostname, "127.0.0.1");
const browser = await chromium.launch();
try {
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  const response = await page.goto(base.href);
  assert.equal(response.status(), 200);
  await page.waitForFunction(() =>
    document.querySelector("#root")?.textContent?.trim(),
  );
  assert.match(await page.title(), /Lucky Vibe Kanban/);
  assert.equal(await page.locator("iframe").count(), 0);
  assert.deepEqual(errors, []);
  const cdp = await page.context().newCDPSession(page);
  assert.match((await cdp.send("Browser.getVersion")).product, /Chrome/);
  await page.screenshot({ path: ".build/local247/full-app-started.png" });
  console.log(
    "PASS built standalone frontend renders in bundled Chromium with CDP",
  );
} finally {
  await browser.close();
}
