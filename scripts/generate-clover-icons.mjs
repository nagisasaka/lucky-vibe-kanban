import { chromium } from "@playwright/test";
import { readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";

// Preserve the approved SVGs byte for byte. Only extend the maskable background
// to the canvas edges; the four leaves keep B's original transform and paths.
const directory = new URL("../packages/public/", import.meta.url);
for (const [name, hash] of [
  [
    "lvk-clover.svg",
    "3b42c7b08d0a25af7533ab4511f3782fb318eccb05ccc474ddfae7fae8d43de7",
  ],
  [
    "lvk-app-icon.svg",
    "4cd6e93f81f1cb2da075ce2754f2c1f038c2d0338b2b2968adc2d414455a340c",
  ],
]) {
  const bytes = await readFile(new URL(name, directory));
  if (createHash("sha256").update(bytes).digest("hex") !== hash)
    throw new Error(`Approved source hash mismatch: ${name}`);
}
const svg = await readFile(new URL("lvk-app-icon.svg", directory), "utf8");
const browser = await chromium.launch({
  ...(process.env.LVK_CHROMIUM_PATH
    ? { executablePath: process.env.LVK_CHROMIUM_PATH }
    : {}),
});
try {
  const page = await browser.newPage({ deviceScaleFactor: 1 });
  for (const [name, size, maskable] of [
    ["favicon.png", 32, false],
    ["apple-touch-icon.png", 180, true],
    ["lvk-icon-192.png", 192, false],
    ["lvk-icon-512.png", 512, false],
    ["lvk-icon-maskable-512.png", 512, true],
  ]) {
    await page.setViewportSize({ width: size, height: size });
    const source = maskable
      ? svg.replace(
          'x="28" y="28" width="1198" height="1198" rx="245"',
          'x="0" y="0" width="1254" height="1254" rx="0"',
        )
      : svg;
    await page.setContent(
      `<style>html,body{margin:0}svg{display:block;width:100vw;height:100vh}</style>${source}`,
    );
    await writeFile(
      new URL(name, directory),
      await page.screenshot({ omitBackground: true }),
    );
  }
} finally {
  await browser.close();
}
