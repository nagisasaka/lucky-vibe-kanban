// Browser acceptance for a disposable local-access instance only.
import { chromium } from "@playwright/test";
import { mkdir } from "node:fs/promises";
import path from "node:path";

const [base, output, frontendRevision] = process.argv.slice(2);
if (!/^http:\/\/127\.0\.0\.1:\d+$/.test(base) || !output || !frontendRevision) {
  throw new Error(
    "Usage: node verify-external-ui.mjs http://127.0.0.1:PORT OUTPUT FRONTEND_SHA",
  );
}
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
try {
  await page.goto(base, { waitUntil: "networkidle" });
  // Empty-instance workspace creation and first-use help are ordinary dialogs.
  const cancel = page.getByRole("button", { name: "Cancel", exact: true });
  if (await cancel.isVisible()) await cancel.click();
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  const label = page.getByTestId("app-build-info");
  await label.waitFor({ state: "visible", timeout: 20000 });
  await label.click();
  await page
    .getByText(`Frontend: ${frontendRevision}`, { exact: false })
    .waitFor();
  await page.getByRole("dialog").evaluate(async (element) => {
    await Promise.all(
      element
        .getAnimations({ subtree: true })
        .map((animation) => animation.finished),
    );
  });
  await page.screenshot({ path: path.join(output, "build-info-desktop.png") });
  await page.emulateMedia({ colorScheme: "dark" });
  await page.screenshot({
    path: path.join(output, "build-info-desktop-dark.png"),
  });
  await page.setViewportSize({ width: 390, height: 844 });
  await page.emulateMedia({ colorScheme: "dark" });
  await page.screenshot({ path: path.join(output, "build-info-mobile.png") });
  console.log("PASS: actual Vite UI shows separate frontend/backend revisions");
} catch (error) {
  await page.screenshot({ path: path.join(output, "failure.png") });
  throw error;
} finally {
  await browser.close();
}
