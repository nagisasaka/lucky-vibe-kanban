import { chromium } from "@playwright/test";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import assert from "node:assert/strict";

// A/B are immutable originals. The white variant changes only B's background.
// Apple/maskable outputs extend that background without moving the leaves.
const check = process.argv.includes("--check");
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
const white = svg.replace('fill="#18251E"', 'fill="#FFFFFF"');
assert.notEqual(white, svg);
async function emit(name, bytes) {
  const url = new URL(name, directory);
  if (check)
    assert.deepEqual(
      await readFile(url),
      Buffer.from(bytes),
      `Regeneration mismatch: ${name}`,
    );
  else await writeFile(url, bytes);
}
await emit("lvk-app-icon-white.svg", white);
const darkDirectory = new URL("icon-variants/dark-green/", directory);
if (!check) await mkdir(darkDirectory, { recursive: true });
const browser = await chromium.launch({
  ...(process.env.LVK_CHROMIUM_PATH
    ? { executablePath: process.env.LVK_CHROMIUM_PATH }
    : {}),
});
try {
  const page = await browser.newPage({ deviceScaleFactor: 1 });
  for (const [prefix, variant] of [
    ["", white],
    ["icon-variants/dark-green/", svg],
  ]) {
    for (const [name, size, maskable] of [
      ["favicon.png", 32, false],
      ["apple-touch-icon.png", 180, true],
      ["lvk-icon-192.png", 192, false],
      ["lvk-icon-512.png", 512, false],
      ["lvk-icon-maskable-512.png", 512, true],
    ]) {
      await page.setViewportSize({ width: size, height: size });
      const source = maskable
        ? variant.replace(
            'x="28" y="28" width="1198" height="1198" rx="245"',
            'x="0" y="0" width="1254" height="1254" rx="0"',
          )
        : variant;
      await page.setContent(
        `<style>html,body{margin:0}svg{display:block;width:100vw;height:100vh}</style>${source}`,
      );
      await emit(
        prefix + name,
        await page.screenshot({ omitBackground: true }),
      );
    }
  }
} finally {
  await browser.close();
}
console.log(
  check
    ? "Clover assets reproduce exactly"
    : "Generated white primary and dark-green variants",
);
