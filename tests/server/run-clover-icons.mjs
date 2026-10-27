import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, mkdtemp, readFile, writeFile, rm } from "node:fs/promises";
import { resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { chromium } from "@playwright/test";
import { isolatedEnvironment } from "./fixtures/issue-comments-env.mjs";

const root = process.cwd();
const report = resolve(".build/clover-review");
await mkdir(report, { recursive: true });
const instance = await mkdtemp(resolve(".build/clover-instance-"));
const env = isolatedEnvironment(process.env, instance, root);
const fixtureBin = resolve(instance, "bin");
await mkdir(fixtureBin);
for (const name of ["codex", "claude"])
  await writeFile(resolve(fixtureBin, name), "#!/bin/sh\nexit 1\n", { mode: 0o755 });
env.PATH = `${fixtureBin}:${env.PATH}`;
const server = spawn(resolve("target/debug/server"), [], {
  env,
  stdio: ["ignore", "pipe", "pipe"],
});
let logs = "";
server.stdout.on("data", (b) => {
  logs += b;
});
server.stderr.on("data", (b) => {
  logs += b;
});
const stopped = new Promise((resolve, reject) => {
  server.once("close", resolve);
  server.once("error", reject);
});
let browser;
try {
  let base;
  for (let i = 0; i < 120; i++) {
    if (server.exitCode !== null) throw Error(`Server exited: ${logs}`);
    try {
      const ports = JSON.parse(
        await readFile(`${instance}/ports/vibe-kanban.port`, "utf8"),
      );
      const url = `http://127.0.0.1:${ports.main_port}`;
      if ((await fetch(url + "/api/info")).ok) {
        base = url;
        break;
      }
    } catch {}
    await delay(500);
  }
  assert.ok(base, `Startup timeout: ${logs}`);
  browser = await chromium.launch({
    ...(process.env.LVK_CHROMIUM_PATH
      ? { executablePath: process.env.LVK_CHROMIUM_PATH }
      : {}),
  });
  const manifestResponse = await fetch(base + "/site.webmanifest");
  assert.match(
    manifestResponse.headers.get("content-type"),
    /application\/(manifest\+json|json)/,
  );
  const manifest = await manifestResponse.json();
  for (const [key, value] of Object.entries({
    name: "Lucky Vibe Kanban",
    short_name: "LVK",
    id: "/",
    start_url: "/",
    scope: "/",
    display: "standalone",
    theme_color: "#f2f2f2",
    background_color: "#f2f2f2",
  }))
    assert.equal(manifest[key], value);
  const mcp = await readFile("crates/executors/default_mcp.json", "utf8");
  assert.ok(mcp.includes('"icon": "lvk-app-icon.svg"'));
  assert.ok(!mcp.includes("favicon-vk-"));
  const checks = [];
  for (const scheme of ["light", "dark"]) {
    const page = await browser.newPage({ colorScheme: scheme });
    const response = await page.goto(base, { waitUntil: "networkidle" });
    assert.equal(response.status(), 200);
    assert.match(response.headers()["content-type"], /text\/html/);
    assert.equal(
      await page.locator('link[rel="icon"]').getAttribute("href"),
      "/lvk-app-icon.svg",
    );
    assert.equal(
      await page.locator('link[rel="manifest"]').getAttribute("href"),
      "/site.webmanifest",
    );
    const cdp = await page.context().newCDPSession(page);
    const parsed = await cdp.send("Page.getAppManifest");
    assert.deepEqual(parsed.errors, []);
    assert.equal(JSON.parse(parsed.data).name, manifest.name);
    const apple = await page
      .locator('link[rel="apple-touch-icon"]')
      .getAttribute("href");
    const assets = [
      "/lvk-clover.svg",
      "/lvk-app-icon.svg",
      "/favicon.png",
      apple,
      ...manifest.icons.map((i) => i.src),
    ];
    for (const asset of assets) {
      const res = await fetch(base + asset);
      assert.equal(res.status, 200, asset);
      assert.match(
        res.headers.get("content-type"),
        asset.includes(".svg") ? /image\/svg\+xml/ : /image\/png/,
      );
      const bytes = Buffer.from(await res.arrayBuffer());
      const local = await readFile(
        resolve("packages/public", asset.split("?")[0].slice(1)),
      );
      assert.deepEqual(bytes, local, `Served asset differs: ${asset}`);
    }
    // Render browser-decoded assets on actual light/dark tab backgrounds and
    // inspect pixel content at tab sizes, plus the maskable circular safe zone.
    const pixels = await page.evaluate(
      async ({ scheme, icons, apple }) => {
        async function render(src, size) {
          const img = new Image();
          img.src = src;
          await img.decode();
          const c = document.createElement("canvas");
          c.width = c.height = size;
          const ctx = c.getContext("2d");
          ctx.drawImage(img, 0, 0, size, size);
          return { img, c, data: ctx.getImageData(0, 0, size, size).data };
        }
        const small = [];
        for (const size of [16, 32]) {
          const { data } = await render("/lvk-app-icon.svg", size);
          const quadrants = [0, 0, 0, 0];
          for (let y = 0; y < size; y++)
            for (let x = 0; x < size; x++) {
              const i = (y * size + x) * 4;
              if (data[i + 1] > 150 && data[i] > 70 && data[i + 3] > 128)
                quadrants[(y >= size / 2 ? 2 : 0) + (x >= size / 2 ? 1 : 0)]++;
            }
          small.push({ size, quadrants });
        }
        const dimensions = [];
        for (const icon of [...icons, { src: apple, sizes: "180x180" }]) {
          const size = Number(icon.sizes.split("x")[0]);
          const { img, data } = await render(icon.src, size);
          let outside = 0,
            transparent = 0,
            green = 0,
            radius = 0;
          for (let y = 0; y < size; y++)
            for (let x = 0; x < size; x++) {
              const i = (y * size + x) * 4;
              if (data[i + 3] !== 255) transparent++;
              if (data[i + 1] > 150 && data[i] > 70 && data[i + 3] > 128) {
                green++;
                const r =
                  Math.hypot(x + 0.5 - size / 2, y + 0.5 - size / 2) / size;
                radius = Math.max(radius, r);
                if (r > 0.4) outside++;
              }
            }
          dimensions.push({
            src: icon.src,
            width: img.naturalWidth,
            height: img.naturalHeight,
            size,
            outside,
            transparent,
            green,
            radius,
          });
        }
        // Remove application styles from this visual fixture; the actual app
        // HTML/links and served bytes were checked above.
        document.head
          .querySelectorAll('style,link[rel="stylesheet"]')
          .forEach((el) => el.remove());
        document.body.innerHTML = "";
        document.body.style.cssText = `background:${scheme === "light" ? "#fff" : "#212121"};color:${scheme === "light" ? "#000" : "#fff"};font:16px sans-serif;padding:24px`;
        for (const size of [16, 32, 180, 192]) {
          const row = document.createElement("div");
          row.style.cssText =
            "display:flex;align-items:center;gap:20px;margin:20px";
          row.textContent = `${size}px`;
          const img = new Image(size, size);
          img.src =
            size === 180
              ? apple
              : size === 192
                ? "/lvk-icon-192.png"
                : "/lvk-app-icon.svg";
          await img.decode();
          row.append(img);
          document.body.append(row);
        }
        const mask = new Image(192, 192);
        mask.src = icons.find((i) => i.purpose === "maskable").src;
        await mask.decode();
        mask.style.borderRadius = "50%";
        document.body.append(mask);
        return { small, dimensions };
      },
      { scheme, icons: manifest.icons, apple },
    );
    for (const item of pixels.small)
      assert.ok(
        item.quadrants.every((n) => n >= 4),
        JSON.stringify(item),
      );
    for (const item of pixels.dimensions) {
      assert.equal(item.width, item.size);
      assert.equal(item.height, item.size);
      assert.ok(item.green > 100);
      if (item.src.includes("maskable") || item.src === apple) {
        assert.equal(item.transparent, 0);
        assert.equal(item.outside, 0);
      }
    }
    await page.evaluate(
      () =>
        new Promise((resolve) =>
          requestAnimationFrame(() => requestAnimationFrame(resolve)),
        ),
    );
    const screenshot = await page.screenshot({
      path: `${report}/${scheme}.png`,
      fullPage: true,
    });
    const painted = await page.evaluate(async (base64) => {
      const shot = new Image();
      shot.src = `data:image/png;base64,${base64}`;
      await shot.decode();
      const canvas = document.createElement("canvas");
      canvas.width = shot.width;
      canvas.height = shot.height;
      const ctx = canvas.getContext("2d");
      ctx.drawImage(shot, 0, 0);
      return [...document.images].map((img) => {
        const box = img.getBoundingClientRect();
        const data = ctx.getImageData(
          Math.floor(box.x),
          Math.floor(box.y),
          Math.floor(box.width),
          Math.floor(box.height),
        ).data;
        let green = 0;
        for (let i = 0; i < data.length; i += 4)
          if (
            data[i + 1] > 150 &&
            data[i + 1] > data[i] + 40 &&
            data[i + 1] > data[i + 2] + 40
          )
            green++;
        return { src: img.getAttribute("src"), size: box.width, green };
      });
    }, screenshot.toString("base64"));
    assert.ok(
      painted.every((item) => item.green >= 20),
      JSON.stringify(painted),
    );
    pixels.painted = painted;
    checks.push({ scheme, ...pixels });
    await page.close();
  }
  const hash = (path) =>
    readFile(path).then((b) => createHash("sha256").update(b).digest("hex"));
  const result = {
    checks,
    server_sha256: await hash("target/debug/server"),
    html_sha256: await hash("packages/local-web/dist/index.html"),
    browser: browser.version(),
    instance_isolated: true,
  };
  await writeFile(
    `${report}/results.json`,
    JSON.stringify(result, null, 2) + "\n",
  );
  console.log(JSON.stringify(result, null, 2));
} finally {
  await browser?.close();
  server.kill("SIGTERM");
  const timer = setTimeout(() => server.kill("SIGKILL"), 10000);
  await stopped;
  clearTimeout(timer);
  await rm(instance, { recursive: true, force: true });
}
