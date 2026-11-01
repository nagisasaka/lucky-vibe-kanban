import test from "node:test";
import assert from "node:assert/strict";
import { build } from "esbuild";
import { chromium, expect } from "@playwright/test";
import { createServer } from "node:http";
import { mkdtemp, readFile, rm, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve, join } from "node:path";
import { spawnSync } from "node:child_process";

test("event settings persist, isolate registrations and render at mobile/desktop widths", async () => {
  const scratch = await mkdtemp(join(tmpdir(), "lvk-push-settings-"));
  let browser;
  let server;
  try {
    const script = join(scratch, "fixture.js");
    await build({
      stdin: {
        contents: `import React, {useState} from 'react';
          import {createRoot} from 'react-dom/client';
          import {WebPushSettings} from './src/shared/components/web-push/WebPushSettings';
          import {OsNotificationEvents} from './src/shared/dialogs/settings/settings/OsNotificationEvents';
          function App() { const [mask,setMask] = useState(15);
            return <main className="p-base space-y-double" style={{maxWidth:768,marginInline:"auto"}}>
              <WebPushSettings/><section aria-label="OS通知"><OsNotificationEvents mask={mask} onChange={setMask}/></section>
            </main> }
          createRoot(document.getElementById('root')).render(<App/>);`,
        resolveDir: resolve("packages/web-core"),
        loader: "tsx",
      },
      outfile: script,
      bundle: true,
      jsx: "automatic",
      alias: { "@": resolve("packages/web-core/src") },
    });
    const cssPath = join(scratch, "fixture.css");
    const css = spawnSync(
      "pnpm",
      [
        "--dir",
        "packages/web-core",
        "exec",
        "tailwindcss",
        "-c",
        resolve("packages/local-web/tailwind.new.config.js"),
        "-i",
        resolve("packages/web-core/src/app/styles/new/index.css"),
        "-o",
        cssPath,
        "--content",
        [
          resolve("packages/web-core/src/**/*.{ts,tsx}"),
          resolve("packages/ui/src/**/*.{ts,tsx}"),
          script,
        ].join(","),
      ],
      { encoding: "utf8" },
    );
    assert.equal(css.status, 0, css.stderr);
    const files = {
      "/fixture.js": await readFile(script),
      "/fixture.css": await readFile(cssPath),
    };
    server = createServer((req, res) => {
      if (files[req.url]) {
        res.setHeader(
          "Content-Type",
          req.url.endsWith(".js") ? "text/javascript" : "text/css",
        );
        res.end(files[req.url]);
      } else {
        res.setHeader("Content-Type", "text/html");
        res.end(
          '<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/fixture.css"><div id="root"></div><script src="/fixture.js"></script>',
        );
      }
    });
    await new Promise((r) => server.listen(0, "127.0.0.1", r));
    const origin = `http://127.0.0.1:${server.address().port}`;
    browser = await chromium.launch({
      ...(process.env.LVK_CHROMIUM_PATH
        ? { executablePath: process.env.LVK_CHROMIUM_PATH }
        : {}),
    });
    const page = await browser.newPage();
    const errors = [];
    page.on("pageerror", (e) => errors.push(e.message));
    const registrations = [
      { id: "one", project_id: "p1", endpoint_hash: "fixture", event_mask: 31 },
      { id: "two", project_id: "p2", endpoint_hash: "other", event_mask: 31 },
    ];
    let failSave = false;
    let subscribeCalls = 0;
    await page.route("**/*", async (route) => {
      const url = new URL(route.request().url());
      if (url.origin !== origin) return route.abort();
      if (!url.pathname.startsWith("/api/web-push")) return route.continue();
      const request = route.request();
      if (url.pathname.endsWith("/events")) {
        if (failSave) return route.fulfill({ status: 500 });
        const id = url.pathname.split("/")[3];
        registrations.find((r) => r.id === id).event_mask =
          request.postDataJSON().event_mask;
        return route.fulfill({ status: 204 });
      }
      if (request.method() === "POST") subscribeCalls++;
      return route.fulfill({
        json: {
          public_key: "BA",
          registrations,
          projects: [
            { id: "p1", name: "Project One" },
            { id: "p2", name: "Project Two" },
          ],
        },
      });
    });
    await page.addInitScript(() => {
      Object.defineProperty(navigator.serviceWorker, "getRegistration", {
        value: async () => null,
      });
    });
    await page.goto(origin);
    const inputs = page.locator("fieldset").first().getByRole("checkbox");
    await inputs.first().waitFor();
    assert.equal(await inputs.count(), 5);
    for (let i = 0; i < 5; i++) {
      await inputs.nth(i).click();
      await expect(inputs.nth(i)).not.toBeChecked();
      await page.waitForFunction(
        () => !document.querySelector("fieldset").disabled,
      );
    }
    assert.equal(registrations[0].event_mask, 0);
    assert.equal(registrations[1].event_mask, 31);
    await page.reload();
    await inputs.first().waitFor();
    for (let i = 0; i < 5; i++)
      assert.equal(await inputs.nth(i).isChecked(), false);
    await inputs.nth(2).click();
    await expect(inputs.nth(2)).toBeChecked();
    await page.waitForFunction(
      () => !document.querySelector("fieldset").disabled,
    );
    assert.equal(registrations[0].event_mask, 4);
    failSave = true;
    await inputs.nth(0).click();
    await page.getByRole("alert").waitFor();
    assert.equal(await inputs.nth(0).isChecked(), false);
    assert.equal(registrations[0].event_mask, 4);
    await page.evaluate(() => {
      Object.defineProperty(Notification, "requestPermission", {
        value: async () => "denied",
      });
    });
    await page.getByRole("button", { name: "この端末でON / 再登録" }).click();
    await expect(page.getByRole("alert")).toContainText(
      "通知が許可されていません",
    );
    assert.equal(subscribeCalls, 0);
    failSave = false;
    await inputs.nth(1).click();
    await expect(inputs.nth(1)).toBeChecked();
    await expect(page.getByRole("alert")).toHaveCount(0);
    const os = page
      .getByRole("region", { name: "OS通知" })
      .getByRole("checkbox");
    for (let i = 0; i < 4; i++) {
      await os.nth(i).uncheck();
      assert.equal(await os.nth(i).isChecked(), false);
    }
    const output = resolve(".build/web-push-settings");
    await mkdir(output, { recursive: true });
    for (const [width, theme] of [
      [390, "light"],
      [1280, "dark"],
    ]) {
      await page.setViewportSize({ width, height: 1000 });
      await page.evaluate(
        (theme) =>
          document.documentElement.classList.toggle("dark", theme === "dark"),
        theme,
      );
      assert.equal(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= innerWidth,
        ),
        true,
      );
      await page.screenshot({
        path: join(output, `${width}-${theme}.png`),
        fullPage: true,
      });
    }
    await page.addInitScript(() => {
      delete window.PushManager;
    });
    await page.reload();
    await expect(
      page.getByText(
        "この環境では利用できません。HTTPSの対応ブラウザを使用してください。",
      ),
    ).toBeVisible();
    assert.deepEqual(errors, []);
    console.log(
      "UI fixture: 390px light / 1280px dark; API calls mocked, no Push sent.",
    );
  } finally {
    await browser?.close();
    if (server) await new Promise((r) => server.close(r));
    await rm(scratch, { recursive: true, force: true });
  }
});
