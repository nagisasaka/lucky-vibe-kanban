import { readFile, readdir, mkdir } from "node:fs/promises";
import { test } from "node:test";
import assert from "node:assert/strict";
import { build } from "esbuild";
import { chromium } from "@playwright/test";
import { fileURLToPath } from "node:url";
const root = fileURLToPath(new URL("../../", import.meta.url));
const cssDirectory = `${root}packages/local-web/dist/assets`;
const styles = (
  await Promise.all(
    (await readdir(cssDirectory))
      .filter((name) => name.endsWith(".css"))
      .map((name) => readFile(`${cssDirectory}/${name}`, "utf8")),
  )
).join("\n");
await mkdir(`${root}.build/local247`, { recursive: true });
const mock = `
const api = () => window;
const processes = () => ['one','two'].map((id) => ({id, session_id:'session', status:api().running ? 'running' : 'completed', exit_code:!api().running && id==='two' ? 1n : null, executor_action:{typ:{type:'ScriptRequest',working_dir:id}}}));
export const useWorkspaceContext = () => ({workspaceId:'fixture', repos:[{id:'repo-one'},{id:'repo-two'}]});
export const useDevServer = () => ({start:() => {api().calls.push('start');api().running=true;api().refresh();}, stop:() => {api().calls.push('stop');api().running=false;api().refresh();}, isStarting:api().mode === 'starting', isStopping:false, runningDevServers:api().running ? processes() : [], devServerProcesses:processes()});
export const useLogsPanel = () => ({logsPanelContent:{type:'process',processId:api().selected}, viewProcessInPanel:(id)=>{api().selected=id;api().refresh();}});
export const useLogStream = (id) => ({logs:api().fixtureLogs[id] ?? []});
export const ScriptFixerDialog = {show:(args)=>{api().calls.push(args.scriptType);}};
`;
const bundle = await build({
  entryPoints: [`${root}tests/embedded-browser-removal/controls.fixture.tsx`],
  bundle: true,
  write: false,
  format: "iife",
  jsx: "automatic",
  alias: { "@": `${root}packages/web-core/src` },
  nodePaths: [`${root}packages/web-core/node_modules`],
  plugins: [
    {
      name: "isolated-manager-observations",
      setup(b) {
        b.onResolve(
          {
            filter:
              /useWorkspaceContext$|useDevServer$|useLogsPanel$|useLogStream$|ScriptFixerDialog$/,
          },
          (a) => ({ path: a.path, namespace: "fixture" }),
        );
        b.onLoad({ filter: /.*/, namespace: "fixture" }, () => ({
          contents: mock,
          loader: "js",
        }));
      },
    },
  ],
});

test("managed controls, per-repo logs and external URLs work without iframe; old component context renders; Chromium CDP remains available", async () => {
  const browser = await chromium.launch({ headless: true });
  try {
    const session = await browser.newBrowserCDPSession();
    assert.match((await session.send("Browser.getVersion")).product, /Chrome/);
    for (const viewport of [
      { width: 1280, height: 900 },
      { width: 390, height: 844 },
    ]) {
      const context = await browser.newContext({ viewport });
      await context.grantPermissions(["clipboard-read", "clipboard-write"]);
      const page = await context.newPage();
      const errors = [];
      page.on("pageerror", (e) => errors.push(e.message));
      // A secure loopback origin is required for the real clipboard API.
      await page.route("http://127.0.0.1:19173/**", (route) =>
        route.fulfill({
          contentType: "text/html",
          body: '<!doctype html><div id="root"></div>',
        }),
      );
      await page.goto("http://127.0.0.1:19173/");
      await page.addStyleTag({ content: styles });
      await page.addScriptTag({ content: bundle.outputFiles[0].text });
      await page.getByRole("button", { name: "Start", exact: true }).click();
      await page.getByRole("status").filter({ hasText: "Running" }).waitFor();
      assert.equal(
        await page
          .getByRole("textbox", { name: "Application URL" })
          .inputValue(),
        "http://localhost:5173/",
      );
      assert.equal(
        await page
          .getByRole("link", { name: "Open in browser" })
          .getAttribute("href"),
        "http://localhost:5173/",
      );
      await page.getByRole("button", { name: "Copy URL" }).click();
      assert.equal(
        await page.evaluate(() => navigator.clipboard.readText()),
        "http://localhost:5173/",
      );
      await page.getByRole("button", { name: /two · running/ }).click();
      assert.equal(
        await page
          .getByRole("textbox", { name: "Application URL" })
          .inputValue(),
        "https://preview.example.test:18443/app",
      );
      await page
        .getByRole("textbox", { name: "Application URL" })
        .fill("javascript:alert(1)");
      assert.equal(
        await page.getByRole("link", { name: "Open in browser" }).count(),
        0,
      );
      await page
        .getByRole("textbox", { name: "Application URL" })
        .fill("https://user:secret@example.test");
      assert.equal(
        await page.getByRole("link", { name: "Open in browser" }).count(),
        0,
      );
      await page.getByRole("button", { name: /one · running/ }).click();
      assert.equal(
        await page
          .getByRole("textbox", { name: "Application URL" })
          .inputValue(),
        "http://localhost:5173/",
      );
      await page
        .getByRole("button", { name: "Edit dev server script" })
        .click();
      await page.getByRole("button", { name: "Stop", exact: true }).click();
      await page
        .getByRole("button", { name: /two · Failed \(exit 1\)/ })
        .waitFor();
      assert.deepEqual(await page.evaluate(() => window.calls), [
        "start",
        "dev_server",
        "stop",
      ]);
      assert.equal(await page.locator("iframe").count(), 0);
      assert.equal(await page.evaluate(() => window.restoredPanel), "logs");
      assert.equal(
        await page.locator("#history").textContent(),
        "Button.tsx:4",
      );
      assert.equal(
        await page.evaluate(() =>
          window.mobileTabs.some((tab) => tab.id === "preview"),
        ),
        false,
      );
      assert.deepEqual(errors, []);
      for (const theme of ["light", "dark"]) {
        await page.evaluate(
          (dark) => document.documentElement.classList.toggle("dark", dark),
          theme === "dark",
        );
        await page.screenshot({
          path: `${root}.build/local247/controls-${viewport.width}-${theme}.png`,
          fullPage: true,
        });
      }
      await context.close();
    }
  } finally {
    await browser.close();
  }
});
