// Production dialog and API adapter; browser-only fake transport, never a live instance.
import assert from "node:assert/strict";
import { build } from "esbuild";
import { chromium, expect } from "@playwright/test";
import { createRequire } from "node:module";
import { readFile, mkdir } from "node:fs/promises";
const root = process.cwd();
const require = createRequire(`${root}/packages/web-core/package.json`);
const bundle = await build({
  entryPoints: ["tests/issue-delete/dialog.fixture.tsx"],
  bundle: true,
  write: false,
  format: "iife",
  jsx: "automatic",
  alias: { "@": `${root}/packages/web-core/src` },
  nodePaths: [`${root}/packages/web-core/node_modules`],
  loader: { ".css": "empty", ".woff2": "empty", ".woff": "empty" },
  define: { "import.meta.env": "{}" },
  plugins: [
    {
      name: "fixture-only-refresh",
      setup(build) {
        build.onResolve({ filter: /\/issueArchiveApi$/ }, () => ({
          path: "refresh",
          namespace: "fixture",
        }));
        build.onLoad({ filter: /.*/, namespace: "fixture" }, () => ({
          contents: "export function refreshIssueArchive() {}",
        }));
      },
    },
  ],
});
const config = require("tailwindcss/loadConfig")(
  `${root}/packages/local-web/tailwind.new.config.js`,
);
const css = await require("postcss")([
  require("tailwindcss")({
    ...config,
    content: [`${root}/packages/{ui,web-core}/src/**/*.{tsx,ts}`],
  }),
]).process(
  (
    await readFile("packages/web-core/src/app/styles/new/index.css", "utf8")
  ).replace(/^@(import|config).*$/gm, ""),
  { from: undefined },
);
const browser = await chromium.launch({
  headless: true,
  args: ["--no-sandbox"],
});
let passed = 0;
try {
  for (const lang of ["en", "ja"]) {
    for (const mode of [
      "off",
      "on",
      "partial",
      "unknown",
      "changed",
      "blocked",
      "cancel",
      "malformed",
      "wrongReceipt",
    ]) {
      const page = await browser.newPage({
        viewport: { width: lang === "ja" ? 390 : 1100, height: 900 },
      });
      const errors = [];
      page.on("pageerror", (error) => errors.push(error.message));
      const calls = [];
      let confirmBody;
      const target = {
        workspace_id: "workspace-1",
        name: "Fixture Workspace",
        branch: "kept-branch",
        path: "/fixture/managed",
        excluded: false,
        reason: mode === "blocked" ? "busy" : null,
      };
      const external = {
        ...target,
        workspace_id: "external-2",
        name: "External",
        excluded: true,
        reason: "external",
      };
      const complete = {
        request_id: "request",
        status: "succeeded",
        result: JSON.stringify({
          issue_deleted: true,
          workspaces: [
            {
              workspace_id: target.workspace_id,
              db_deleted: true,
              cleanup_completed: mode !== "partial",
              cleanup_errors:
                mode === "partial" ? ["fixture cleanup failed"] : [],
            },
          ],
        }),
      };
      await page.route("https://fixture.invalid/**", async (route) => {
        const path = new URL(route.request().url()).pathname;
        if (!path.startsWith("/api/"))
          return route.fulfill({
            contentType: "text/html",
            body: '<div id="root"></div>',
          });
        calls.push(path);
        let data;
        if (path.endsWith("/prepare"))
          data = {
            confirmation_token: "token",
            workspaces: [target, external],
          };
        else if (path.endsWith("/confirm")) {
          confirmBody = route.request().postDataJSON();
          if (mode === "unknown") return route.abort();
          data =
            mode === "changed"
              ? {
                  request_id: confirmBody.request_id,
                  status: "failed",
                  result: JSON.stringify({
                    message:
                      "Workspace targets changed; refresh and confirm again",
                  }),
                }
              : {
                  ...complete,
                  request_id:
                    mode === "wrongReceipt"
                      ? "another-request"
                      : confirmBody.request_id,
                  result:
                    mode === "malformed"
                      ? JSON.stringify({ issue_deleted: true, workspaces: [] })
                      : complete.result,
                };
        } else if (path.includes("/issue-delete-operations/"))
          data = { ...complete, request_id: confirmBody.request_id };
        else throw new Error(`unexpected API: ${path}`);
        await route.fulfill({
          contentType: "application/json",
          body: JSON.stringify({ success: true, data }),
        });
      });
      await page.goto(`https://fixture.invalid/?lang=${lang}`);
      await page.addStyleTag({ content: css.css });
      if (lang === "ja")
        await page.evaluate(() =>
          document.documentElement.classList.add("dark"),
        );
      await page.addScriptTag({ content: bundle.outputFiles[0].text });
      await page.locator("#open").click();
      const dialog = page.getByRole("dialog");
      await expect(dialog).toBeVisible();
      const checkbox = dialog.getByRole("checkbox");
      await expect(checkbox).not.toBeChecked();
      const deletion = () =>
        dialog.getByRole("button", {
          name: lang === "ja" ? "削除" : "Delete",
          exact: true,
        });
      if (mode === "off") {
        await deletion().click();
        await expect(dialog).toHaveCount(0);
        assert.equal(await page.evaluate(() => window.offDeletes), 1);
        assert.equal(calls.length, 0);
      } else if (mode === "cancel") {
        await checkbox.check();
        await expect(dialog.getByText(/Fixture Workspace/)).toBeVisible();
        await dialog
          .getByRole("button", {
            name: lang === "ja" ? "キャンセル" : "Cancel",
            exact: true,
          })
          .click();
        assert.equal(calls.filter((p) => p.endsWith("/confirm")).length, 0);
      } else {
        await checkbox.check();
        await expect(dialog.getByText(/Fixture Workspace/)).toBeVisible();
        await expect(
          dialog.getByText(
            lang === "ja" ? /除外：外部フォルダ/ : /Excluded: external folder/,
          ),
        ).toBeVisible();
        if (mode === "blocked") {
          await expect(deletion()).toBeDisabled();
          assert.equal(calls.length, 1);
        } else {
          await deletion().dblclick();
          if (["unknown", "malformed", "wrongReceipt"].includes(mode)) {
            await expect(
              dialog.getByText(
                lang === "ja" ? /処理中または結果不明/ : /result is unknown/,
              ),
            ).toBeVisible();
            await expect(deletion()).toHaveCount(0);
            await dialog
              .getByRole("button", {
                name: lang === "ja" ? "結果を確認" : "Check result",
              })
              .click();
            await expect(
              dialog.getByText(
                lang === "ja"
                  ? /物理cleanupも完了/
                  : /Physical cleanup completed/,
              ),
            ).toBeVisible();
          } else if (mode === "partial") {
            await expect(
              dialog.getByText(
                lang === "ja"
                  ? /物理cleanupは未完了/
                  : /physical cleanup is incomplete/,
              ),
            ).toBeVisible();
            await expect(
              dialog.getByText("fixture cleanup failed"),
            ).toBeVisible();
            await expect(deletion()).toHaveCount(0);
          } else if (mode === "changed") {
            await expect(
              dialog.getByText(
                lang === "ja" ? /削除を拒否しました/ : /Deletion refused/,
              ),
            ).toBeVisible();
            await expect(deletion()).toHaveCount(0);
            await dialog
              .getByRole("button", {
                name: lang === "ja" ? "対象を再確認" : "Refresh targets",
              })
              .click();
            await expect(deletion()).toBeEnabled();
          } else
            await expect(
              dialog.getByText(
                lang === "ja"
                  ? /物理cleanupも完了/
                  : /Physical cleanup completed/,
              ),
            ).toBeVisible();
          assert.equal(
            calls.filter((p) => p.endsWith("/confirm")).length,
            1,
            "double click must not duplicate",
          );
          assert.equal(confirmBody.confirmation_token, "token");
          assert.ok(confirmBody.request_id);
        }
      }
      if (mode === "partial") {
        await mkdir(`${root}/.build/issue-delete-validation`, {
          recursive: true,
        });
        await page.screenshot({
          path: `${root}/.build/issue-delete-validation/${lang}-${lang === "ja" ? "mobile-dark" : "desktop-light"}.png`,
          fullPage: true,
        });
      }
      if (mode !== "off" && mode !== "cancel") {
        const geometry = await dialog.evaluate((element) => ({
          client: element.clientWidth,
          scroll: element.scrollWidth,
        }));
        assert.ok(
          geometry.scroll <= geometry.client + 1,
          `${lang}/${mode}: dialog overflow`,
        );
      }
      assert.deepEqual(errors, []);
      await page.close();
      passed++;
    }
  }
  console.log(
    `Issue delete dialog: ${passed} scenarios passed (EN/JA, desktop/mobile, light/dark)`,
  );
} finally {
  await browser.close();
}
