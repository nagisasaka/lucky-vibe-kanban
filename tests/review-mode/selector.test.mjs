import { test } from "node:test";
import assert from "node:assert/strict";
import { build } from "esbuild";
import { chromium, expect } from "@playwright/test";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
const stubs = {
  useUserSystem:
    "export const useUserSystem = () => ({profiles:null, setProfiles:()=>{}, reloadSystem:()=>{}});",
  useTheme:
    'export const useTheme = () => ({theme:"light"}); export const getResolvedTheme = () => "light";',
  useExecutorDiscovery:
    'export const useModelSelectorConfig = () => ({config:{models:[],providers:[],agents:[],permissions:["AUTO","SUPERVISED"],default_model:null},loadingModels:false});',
  usePresetOptions:
    'export const usePresetOptions = () => ({data:{executor:"CODEX",execution_mode:"code",permission_policy:"SUPERVISED"}});',
  api: "export const profilesApi = {};",
};

test("Chromium: Review selection, persisted restoration and submission; Code/Plan/Goal regression", async () => {
  const result = await build({
    entryPoints: [`${root}tests/review-mode/selector.fixture.tsx`],
    bundle: true,
    write: false,
    format: "iife",
    jsx: "automatic",
    alias: { "@": `${root}packages/web-core/src`, shared: `${root}shared` },
    nodePaths: [`${root}packages/web-core/node_modules`],
    plugins: [
      {
        name: "offline-provider-hooks",
        setup(builder) {
          builder.onResolve(
            {
              filter:
                /\/(useUserSystem|useTheme|useExecutorDiscovery|usePresetOptions|api)$/,
            },
            ({ path }) => ({ path: path.split("/").at(-1), namespace: "stub" }),
          );
          builder.onLoad({ filter: /.*/, namespace: "stub" }, ({ path }) => ({
            contents: stubs[path],
          }));
        },
      },
    ],
  });
  const browser = await chromium.launch({
    headless: true,
    args: ["--no-sandbox"],
  });
  try {
    for (const width of [1440, 390]) {
      const page = await browser.newPage({ viewport: { width, height: 900 } });
      const errors = [];
      page.on("pageerror", (e) => errors.push(e.message));
      // A local origin provides persistent storage without a backend or live data.
      await page.route("http://review.test/**", (route) =>
        route.fulfill({
          contentType: "text/html",
          body: '<div id="root"></div>',
        }),
      );
      await page.goto("http://review.test/");
      await page.addScriptTag({ content: result.outputFiles[0].text });
      const read = async (id) =>
        JSON.parse(await page.locator(`#${id}`).textContent());
      await page.waitForTimeout(200);
      assert.deepEqual(errors, [], await page.locator("body").innerText());
      let current = "Code";
      for (const next of ["Review", "Plan", "Goal", "Code", "Review"]) {
        await page.getByRole("button", { name: current, exact: true }).click();
        await page.getByRole("menuitem", { name: next, exact: true }).click();
        await expect(
          page.getByRole("button", { name: next, exact: true }),
        ).toBeVisible();
        assert.equal((await read("config")).execution_mode, next.toLowerCase());
        assert.equal((await read("config")).permission_policy, "SUPERVISED");
        await page
          .getByRole("button", { name: "Remount composer", exact: true })
          .click();
        await expect(
          page.getByRole("button", { name: next, exact: true }),
        ).toBeVisible();
        await page.getByRole("button", { name: "Submit", exact: true }).click();
        assert.equal(
          (await read("submitted")).execution_mode,
          next.toLowerCase(),
        );
        current = next;
      }
      await page.reload();
      await page.addScriptTag({ content: result.outputFiles[0].text });
      await expect(
        page.getByRole("button", { name: "Review", exact: true }),
      ).toBeVisible();
      assert.equal((await read("config")).execution_mode, "review");
      assert.deepEqual(errors, []);
      await page.close();
    }
  } finally {
    await browser.close();
  }
});
