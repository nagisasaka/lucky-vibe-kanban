import { test } from "node:test";
import assert from "node:assert/strict";
import { build } from "esbuild";
import { chromium, expect } from "@playwright/test";
import { fileURLToPath } from "node:url";
import { createRequire } from "node:module";
import { readFile, mkdtemp, mkdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

const root = fileURLToPath(new URL("../../", import.meta.url));
const require = createRequire(
  new URL("../../packages/web-core/package.json", import.meta.url),
);

test("named editor settings persist and prepared links require a user click", async () => {
  const bundle = await build({
    entryPoints: [
      new URL("./external-editors.fixture.tsx", import.meta.url).pathname,
    ],
    bundle: true,
    write: false,
    format: "iife",
    jsx: "automatic",
    define: { "import.meta.hot": "undefined" },
    alias: { "@": `${root}packages/web-core/src` },
    nodePaths: [`${root}packages/web-core/node_modules`],
    plugins: [
      {
        name: "editor-api-fixture",
        setup(builder) {
          builder.onResolve(
            {
              filter:
                /WebPushSettings$|TagManager$|FolderPickerDialog$|useTheme$|useIsMobile$|useUiPreferencesStore$/,
            },
            (args) => ({ path: args.path, namespace: "unrelated-settings" }),
          );
          builder.onLoad(
            { filter: /.*/, namespace: "unrelated-settings" },
            ({ path }) => ({
              contents: path.endsWith("WebPushSettings")
                ? "export const WebPushSettings = () => null;"
                : path.endsWith("TagManager")
                  ? "export const TagManager = () => null;"
                  : path.endsWith("FolderPickerDialog")
                    ? "export const FolderPickerDialog = { show: async () => null };"
                    : path.endsWith("useTheme")
                      ? "export const useTheme = () => ({ theme: 'light', setTheme: () => {} });"
                      : path.endsWith("useIsMobile")
                        ? "export const useIsMobile = () => false;"
                        : "export const useMobileFontScale = () => [1, () => {}];",
            }),
          );
          builder.onResolve({ filter: /shared\/lib\/api$/ }, () => ({
            path: "api",
            namespace: "fixture",
          }));
          builder.onLoad({ filter: /.*/, namespace: "fixture" }, () => ({
            contents: `export const workspacesApi = { openEditor: (...args) => window.resolve('workspace', ...args) }; export const repoApi = { openEditor: (...args) => window.resolve('repo', ...args) };`,
          }));
        },
      },
    ],
  });
  const postcss = require("postcss");
  const tailwind = require("tailwindcss");
  const config = require("tailwindcss/loadConfig")(
    `${root}packages/local-web/tailwind.new.config.js`,
  );
  const css = await postcss([
    tailwind({
      ...config,
      safelist: [...config.safelist, "dark"],
      content: [
        `${root}packages/web-core/src/shared/components/ExternalEditorSettings.tsx`,
        `${root}packages/web-core/src/shared/dialogs/command-bar/EditorSelectionDialog.tsx`,
        `${root}packages/ui/src/components/{Button,PrimaryButton,KeyboardDialog}.tsx`,
        `${root}tests/chat/external-editors.fixture.tsx`,
      ],
    }),
  ]).process(
    (
      await readFile(
        `${root}packages/web-core/src/app/styles/new/index.css`,
        "utf8",
      )
    ).replace(/^@(import|config).*$/gm, ""),
    { from: undefined },
  );
  const temp = await mkdtemp(join(tmpdir(), "local-161-ui-"));
  const screenshots = process.env.LVK_EDITOR_SCREENSHOT_DIR ?? temp;
  await mkdir(screenshots, { recursive: true });
  const browser = await chromium.launch({
    headless: true,
    args: ["--no-sandbox"],
  });
  try {
    const context = await browser.newContext();
    await context.route("https://**/*", (route) =>
      route.fulfill({
        contentType: "text/html",
        body: '<div id="root"></div>',
      }),
    );
    const page = await context.newPage();
    page.setDefaultTimeout(10000);
    const errors = [];
    page.on("pageerror", (e) => errors.push(e.message));
    await page.goto("https://lvk.example/");
    await page.addStyleTag({ content: css.css });
    await page.addScriptTag({ content: bundle.outputFiles[0].text });
    const names = page.getByLabel("メニュー表示名");
    const templates = page.getByLabel("URIテンプレート");
    await names.first().waitFor();
    assert.equal(await names.count(), 1);
    assert.equal(await names.first().inputValue(), "VS Code");
    assert.equal(
      await templates.first().inputValue(),
      "vscode://vscode-remote/ssh-remote+lvk.example<lvk_workspace_dir>",
    );
    assert.equal(await page.evaluate(() => window.saveCount), 1);
    assert.equal(
      await page.evaluate(() => window.saved.git_branch_prefix),
      "keep",
    );
    await page.getByText("File menu", { exact: true }).click();
    await page.getByRole("link", { name: "VS Code" }).waitFor();
    assert.equal(await page.getByRole("link").count(), 1);
    assert.match(
      await page.getByRole("link", { name: "VS Code" }).getAttribute("href"),
      /\/workspace\/repo$/,
    );
    assert.deepEqual(
      await page.evaluate(() => window.requests.at(-1).request),
      { repo_id: "repo", file_path: "file.txt" },
    );
    await page.keyboard.press("Escape");
    await page.getByRole("button", { name: "連携を追加" }).click();
    await names.nth(1).fill("Web IDE");
    await templates
      .nth(1)
      .fill("https://web-ide.example/?path=<lvk_workspace_dir>");
    await page
      .getByRole("button", { name: "上へ", exact: true })
      .nth(1)
      .click();
    await expect(names.first()).toHaveValue("Web IDE");
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await page.getByText("Workspace menu", { exact: true }).click();
    await page.getByRole("link", { name: "Web IDE" }).waitFor();
    assert.deepEqual(await page.getByRole("link").allTextContents(), [
      "Web IDE",
      "VS Code",
    ]);
    assert.equal(
      await page.getByRole("link", { name: "Web IDE" }).getAttribute("rel"),
      "noopener noreferrer",
    );
    const requestCount = await page.evaluate(() => window.requests.length);
    const popupPromise = context.waitForEvent("page");
    await page.getByRole("link", { name: "Web IDE" }).click();
    const popup = await popupPromise;
    await popup.waitForLoadState();
    assert.equal(await popup.evaluate(() => window.opener), null);
    assert.equal(
      await page.evaluate(() => window.requests.length),
      requestCount,
    );
    await popup.close();
    await page.keyboard.press("Escape");
    await page.getByRole("button", { name: "連携を追加" }).click();
    await names.nth(2).fill("File only");
    await templates.nth(2).fill("custom:/open<lvk_file_path>");
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await page.getByText("Workspace menu", { exact: true }).click();
    await page
      .getByText("This template requires a file; select a file to use it")
      .waitFor();
    assert.equal(
      await page.getByRole("link", { name: "File only" }).count(),
      0,
    );
    await page.keyboard.press("Escape");
    for (const [button, expectedKind, repoId] of [
      ["Worktree repo menu", "workspace", "repo"],
      ["Original repo menu", "repo", null],
    ]) {
      await page.getByText(button, { exact: true }).click();
      await page.getByRole("link", { name: "Web IDE" }).waitFor();
      assert.deepEqual(
        await page.evaluate(() => {
          const last = window.requests.at(-1);
          return [last.kind, last.request.repo_id];
        }),
        [expectedKind, repoId],
      );
      await page.keyboard.press("Escape");
    }
    await page.getByText("Missing menu", { exact: true }).click();
    await page.getByRole("alert").waitFor();
    assert.equal(await page.getByRole("link").count(), 0);
    await page.keyboard.press("Escape");
    // A now-unavailable target must not expose a previous cached URI.
    await page.evaluate(() => {
      window.failWorkspace = true;
    });
    await page.getByText("Workspace menu", { exact: true }).click();
    await page.getByRole("alert").waitFor();
    assert.equal(await page.getByRole("link").count(), 0);
    await page.keyboard.press("Escape");
    await page.evaluate(() => {
      window.failWorkspace = false;
    });
    // Exercise real theme styles and mobile layout, preserving screenshots only on request.
    for (const width of [1100, 390]) {
      await page.setViewportSize({ width, height: 900 });
      for (const dark of [false, true]) {
        await page.evaluate(
          (dark) => document.documentElement.classList.toggle("dark", dark),
          dark,
        );
        assert.ok(
          await page.evaluate(
            () => document.documentElement.scrollWidth <= window.innerWidth,
          ),
        );
        await page.screenshot({
          path: join(screenshots, `${width}-${dark}.png`),
          fullPage: true,
        });
      }
    }
    for (let i = 0; i < 3; i++)
      await page
        .getByRole("button", { name: "削除", exact: true })
        .first()
        .click();
    await page.getByRole("button", { name: "Save", exact: true }).click();
    const saves = await page.evaluate(() => window.saveCount);
    await page.getByRole("button", { name: "Reload", exact: true }).click();
    await page.waitForTimeout(150);
    assert.equal(await names.count(), 0);
    assert.equal(await page.evaluate(() => window.saveCount), saves);
    await page.getByText("Workspace menu", { exact: true }).click();
    await page
      .getByText("外部エディターが未登録です。設定から連携を追加してください。")
      .waitFor();
    await page.keyboard.press("Escape");
    await page.getByText("General settings", { exact: true }).click();
    for (const [name, template] of [
      ["One", "custom:/one"],
      ["Two", "custom:/two"],
    ]) {
      await page.getByRole("button", { name: "連携を追加" }).click();
      await names.last().fill(name);
      await templates.last().fill(template);
    }
    await page.getByRole("button", { name: "保存", exact: true }).click();
    await expect
      .poll(() => page.evaluate(() => window.saved.editor.entries.length))
      .toBe(2);
    await page
      .getByRole("button", { name: "削除", exact: true })
      .first()
      .click();
    await expect(names).toHaveCount(1);
    await page
      .getByRole("button", { name: "削除", exact: true })
      .first()
      .click();
    await expect(names).toHaveCount(0);
    await page.getByRole("button", { name: "保存", exact: true }).click();
    await expect
      .poll(() => page.evaluate(() => window.saved.editor.entries.length))
      .toBe(0);
    await page.getByRole("button", { name: "連携を追加" }).click();
    await names.last().fill("Invalid");
    await templates.last().fill("broken URI");
    await page.evaluate(() => {
      window.rejectSave = true;
    });
    await page.getByRole("button", { name: "保存", exact: true }).click();
    await page
      .getByText("Invalid URI template (fixture)", { exact: true })
      .waitFor();
    assert.equal(
      await page.evaluate(() => window.saved.editor.entries.length),
      0,
    );
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
    await rm(temp, { recursive: true, force: true });
  }
});
