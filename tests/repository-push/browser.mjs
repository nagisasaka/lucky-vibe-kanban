// All requests are intercepted. This never contacts an LVK server or GitHub.
import assert from "node:assert/strict";
import { build } from "esbuild";
import { chromium, expect } from "@playwright/test";
import { createRequire } from "node:module";
import { mkdir, readFile } from "node:fs/promises";
const root = process.cwd();
const require = createRequire(`${root}/packages/web-core/package.json`);
const bundle = await build({
  entryPoints: ["tests/repository-push/fixture.tsx"],
  bundle: true,
  write: false,
  format: "iife",
  jsx: "automatic",
  alias: { "@": `${root}/packages/web-core/src`, shared: `${root}/shared` },
  nodePaths: [
    `${root}/packages/web-core/node_modules`,
    `${root}/packages/ui/node_modules`,
  ],
  loader: { ".css": "empty", ".woff2": "empty", ".woff": "empty" },
  define: { "import.meta.env": "{}", "import.meta.hot": "undefined" },
});
const config = require("tailwindcss/loadConfig")(
  `${root}/packages/local-web/tailwind.new.config.js`,
);
const source = (
  await readFile("packages/web-core/src/app/styles/new/index.css", "utf8")
).replace(/^@(import|config).*$/gm, "");
const styles = await require("postcss")([
  require("tailwindcss")({
    ...config,
    safelist: [...config.safelist, "dark", "light"],
    content: [
      `${root}/tests/repository-push/fixture.tsx`,
      `${root}/packages/web-core/src/shared/dialogs/settings/settings/RepositoryPushSettings.tsx`,
      `${root}/packages/web-core/src/shared/dialogs/settings/settings/SettingsComponents.tsx`,
      `${root}/packages/ui/src/components/PrimaryButton.tsx`,
    ],
  }),
]).process(source, { from: undefined });
const browser = await chromium.launch({
  headless: true,
  executablePath: process.env.LVK_E2E_CHROMIUM,
  args: ["--no-sandbox"],
});
try {
  const page = await browser.newPage({
    viewport: { width: 1100, height: 1000 },
  });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.route("**/*", (route) =>
    route.fulfill({
      contentType: "text/html",
      body: '<!doctype html><html class="light"><div id="root"></div></html>',
    }),
  );
  const load = async () => {
    await page.goto("https://fixture.invalid/");
    await page.addStyleTag({ content: styles.css });
    await page.addScriptTag({ content: bundle.outputFiles[0].text });
    await expect(
      page.getByText("Repositoryブランチをpush", { exact: true }),
    ).toBeVisible();
  };
  await load();
  const select = page.getByLabel("ローカルブランチ");
  const preview = page.getByRole("button", {
    name: "push対象を確認",
    exact: true,
  });
  const push = page.getByRole("button", {
    name: "確認したcommitをpush",
    exact: true,
  });
  await select.selectOption("dev");
  await page.getByLabel("設定済みremote").selectOption("origin");
  await preview.click();
  await expect(page.getByText("a".repeat(40), { exact: true })).toBeVisible();
  await expect(
    page.getByText("docs/changes/2026-11-17-LOCAL-385-fixture.md", {
      exact: true,
    }),
  ).toBeVisible();
  await expect(push).toBeDisabled();
  await page.getByRole("checkbox").click();
  await expect(push).toBeEnabled();
  // An explicit server proof of no admission permits a fresh review.
  await push.click();
  await expect(page.getByRole("alert")).toContainText("source_drift");
  await expect(select).toBeEnabled();
  await expect(push).toHaveCount(0);
  assert.equal(await page.evaluate(() => window.pushFixture.pushes), 1);
  await page.evaluate(() => (window.pushFixture.mode = "lost"));
  await preview.click();
  await page.getByRole("checkbox").click();
  await expect(push).toBeEnabled();
  await mkdir("test-results/repository-push", { recursive: true });
  for (const [theme, width] of [
    ["light", 1100],
    ["dark", 390],
  ]) {
    await page.setViewportSize({ width, height: 1000 });
    await page
      .locator("html")
      .evaluate((el, theme) => (el.className = theme), theme);
    await page.screenshot({
      path: `test-results/repository-push/${theme}-${width}.png`,
      fullPage: true,
    });
    const overflow = await page.evaluate(
      () => document.documentElement.scrollWidth > innerWidth,
    );
    assert.equal(overflow, false);
  }
  await push.click();
  await expect(select).toBeDisabled();
  await expect(push).toBeDisabled();
  assert.equal(await page.evaluate(() => window.pushFixture.pushes), 2);
  // Reload preserves the request identity and offers only read-only verification.
  const request = await page.evaluate(() =>
    sessionStorage.getItem("repository-push:fixture:repo"),
  );
  assert.ok(request);
  await load();
  await expect(page.getByText(request, { exact: false })).toBeVisible();
  await expect(select).toBeDisabled();
  assert.equal(await page.evaluate(() => window.pushFixture.pushes), 0);
  await page.evaluate(() => (window.pushFixture.mode = "verify-fail"));
  await page
    .getByRole("button", { name: "remoteの結果を再確認", exact: true })
    .click();
  await expect(
    page.getByText("protected_branch", { exact: true }),
  ).toBeVisible();
  await expect(page.getByRole("alert")).toContainText(
    "authentication_rejected",
  );
  await expect(
    page.getByText(
      "remoteブランチは現在、確認したsource SHAを指していません。",
      { exact: true },
    ),
  ).toHaveCount(0);
  await page.evaluate(() => (window.pushFixture.mode = "verify-uncertain"));
  await page
    .getByRole("button", { name: "remoteの結果を再確認", exact: true })
    .click();
  await expect(
    page.getByText("remoteブランチは確認したsource SHAを指しています。", {
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    page.getByText("状態: uncertain", { exact: false }),
  ).toBeVisible();
  assert.equal(await page.evaluate(() => window.pushFixture.verifications), 2);
  assert.equal(await page.evaluate(() => window.pushFixture.pushes), 0);
  await page
    .getByRole("button", { name: "別のpush対象を確認", exact: true })
    .click();
  await expect(select).toBeEnabled();
  assert.equal(
    await page.evaluate(() =>
      sessionStorage.getItem("repository-push:fixture:repo"),
    ),
    null,
  );
  assert.deepEqual(errors, []);
  console.log(
    "PASS: explicit preview/confirmation, not-admitted recovery, response-loss/reload verification, light/desktop and dark/mobile",
  );
} finally {
  await browser.close();
}
