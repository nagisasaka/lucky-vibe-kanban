import { test } from "node:test";
import assert from "node:assert/strict";
import { build } from "esbuild";
import { chromium, expect } from "@playwright/test";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
test("Project creation suggests keys, preserves manual edits and failed input, and sends normalized keys", async () => {
  const bundle = await build({
    entryPoints: [new URL("./dialog.fixture.tsx", import.meta.url).pathname],
    bundle: true,
    write: false,
    format: "iife",
    jsx: "automatic",
    alias: { "@": `${root}packages/web-core/src` },
    nodePaths: [`${root}packages/web-core/node_modules`],
    plugins: [
      {
        name: "shape-fixture",
        setup(builder) {
          builder.onResolve(
            { filter: /shared\/integrations\/electric\/hooks$/ },
            () => ({ path: "shape", namespace: "fixture" }),
          );
          builder.onLoad({ filter: /.*/, namespace: "fixture" }, () => ({
            contents: `
        const projects = [{key:'LVK'}, {key:'LVK2'}];
        export function useShape() { return { data:projects, error:null, insert(input) {
          window.inputs.push(input);
          return { data:input, persisted: window.rejectCreate ? Promise.reject(new Error('Project key is already in use')) : Promise.resolve(input) };
        } }; }
      `,
          }));
          builder.onResolve({ filter: /ColorPickerContainer$/ }, () => ({
            path: "color",
            namespace: "color",
          }));
          builder.onLoad({ filter: /.*/, namespace: "color" }, () => ({
            contents: "export const ColorPicker = ({children}) => children;",
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
    const page = await browser.newPage();
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.route("https://lvk.example/**", (route) =>
      route.fulfill({
        contentType: "text/html",
        body: '<div id="root"></div>',
      }),
    );
    await page.goto("https://lvk.example/");
    await page.evaluate(() => {
      window.inputs = [];
      window.rejectCreate = true;
    });
    await page.addScriptTag({ content: bundle.outputFiles[0].text });
    await expect(page.getByText("LVK-161", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Open", exact: true }).click();
    const name = page.locator("#project-name");
    const key = page.getByLabel("Projectキー", { exact: true });
    const create = page.getByRole("button", {
      name: "Create Project",
      exact: true,
    });
    await name.fill("Lucky Vibe Kanban");
    await expect(key).toHaveValue("LVK3");
    await expect(page.getByText(/後から変更できません/)).toBeVisible();
    await name.fill("日本語");
    await expect(key).toHaveValue("");
    await create.click();
    await expect(page.getByText(/半角英数字2〜10文字で入力/)).toBeVisible();
    await key.fill("lvk");
    await create.click();
    await expect(
      page.getByText("このProjectキーは既に使用されています。"),
    ).toBeVisible();
    assert.equal(await page.evaluate(() => window.inputs.length), 0);
    await key.fill(" custom2 ");
    await name.fill("Renamed Project");
    await expect(key).toHaveValue("CUSTOM2");
    await create.click();
    await expect(page.getByText("Project key is already in use")).toBeVisible();
    await expect(name).toHaveValue("Renamed Project");
    await expect(key).toHaveValue("CUSTOM2");
    await page.evaluate(() => {
      window.rejectCreate = false;
    });
    await create.click();
    await expect(key).not.toBeVisible();
    assert.equal(
      await page.evaluate(() => window.inputs.at(-1).key),
      "CUSTOM2",
    );
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});
