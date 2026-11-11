// No running backend/dev server or shared data. API responses are deterministic mocks.
import assert from "node:assert/strict";
import { build } from "esbuild";
import { chromium, expect } from "@playwright/test";
import { createRequire } from "node:module";
import { mkdir, readFile } from "node:fs/promises";
const root = process.cwd();
const require = createRequire(`${root}/packages/web-core/package.json`);
const bundle = await build({
  entryPoints: ["tests/issue-versions/editor.fixture.tsx"],
  bundle: true,
  write: false,
  format: "iife",
  jsx: "automatic",
  alias: { "@": `${root}/packages/web-core/src` },
  nodePaths: [`${root}/packages/web-core/node_modules`],
  loader: { ".css": "empty", ".woff2": "empty", ".woff": "empty" },
  define: { "import.meta.env": "{}" },
});
const config = require("tailwindcss/loadConfig")(
  `${root}/packages/local-web/tailwind.new.config.js`,
);
const css = await require("postcss")([
  require("tailwindcss")({
    ...config,
    content: [
      `${root}/packages/web-core/src/pages/kanban/IssueVersionEditor.tsx`,
      `${root}/packages/ui/src/components/*.tsx`,
    ],
  }),
]).process(
  (
    await readFile("packages/web-core/src/app/styles/new/index.css", "utf8")
  ).replace(/^@(import|config).*$/gm, ""),
  { from: undefined },
);
const browser = await chromium.launch({
  headless: true,
  executablePath: process.env.LVK_E2E_CHROMIUM,
  args: ["--no-sandbox"],
});
try {
  const page = await browser.newPage({
    viewport: { width: 1100, height: 800 },
  });
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  let state = {
    current_version_id: "a",
    current_revision: 1,
    versions: [
      {
        id: "a",
        issue_id: "issue",
        name: "draft1",
        description: "A original",
        revision: 1,
      },
      {
        id: "b",
        issue_id: "issue",
        name: "draft2",
        description: "B original",
        revision: 1,
      },
    ],
  };
  let releaseSave;
  let autoRelease = false;
  let lastPatch;
  let failSave = false;
  let saveStarted = false;
  let gets = 0;
  await page.route("**/*", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    if (url.pathname.startsWith("/api/local/v1/issues/issue/versions")) {
      if (request.method() === "GET") {
        gets++;
        return route.fulfill({ json: state });
      }
      if (request.method() === "POST" && url.pathname.endsWith("/versions")) {
        const body = request.postDataJSON();
        const created = {
          id: "c",
          issue_id: "issue",
          name: body.name,
          description: "",
          revision: 1,
        };
        state = { ...state, versions: [...state.versions, created] };
        return route.fulfill({
          json: { ...state, created_version_id: created.id },
        });
      }
      if (request.method() === "PATCH") {
        if (failSave)
          return route.fulfill({
            status: 409,
            json: { message: "Version conflict. Unsaved text retained." },
          });
        const body = request.postDataJSON();
        lastPatch = body;
        const id = url.pathname.split("/").at(-1);
        const version = state.versions.find((item) => item.id === id);
        assert.equal(body.expected_revision, version.revision);
        saveStarted = true;
        if (!autoRelease)
          await new Promise((resolve) => {
            releaseSave = resolve;
          });
        state = {
          ...state,
          versions: state.versions.map((item) =>
            item.id === id
              ? {
                  ...item,
                  name: body.name,
                  ...(Object.hasOwn(body, "description")
                    ? { description: body.description }
                    : {}),
                  revision: item.revision + 1,
                }
              : item,
          ),
        };
        return route.fulfill({ json: state });
      }
      throw new Error("Unexpected fixture mutation");
    }
    if (url.pathname.startsWith("/api/")) return route.fulfill({ json: {} });
    return route.fulfill({
      contentType: "text/html",
      body: '<!doctype html><div id="root"></div>',
    });
  });
  await page.goto("https://fixture.invalid/");
  await page.addStyleTag({ content: css.css });
  await page.addScriptTag({ content: bundle.outputFiles[0].text });
  const body = page.locator('[contenteditable="true"]');
  const tabA = page.getByRole("tab", { name: /draft1/ });
  const tabB = page.getByRole("tab", { name: /draft2/ });
  const save = page.getByRole("button", {
    name: "Save this version",
    exact: true,
  });
  await expect(tabA).toHaveAttribute("aria-selected", "true");
  await expect(body).toHaveText("A original");
  await body.fill("A captured");
  await save.click();
  await expect.poll(() => saveStarted).toBe(true);
  await expect(
    page.getByRole("status").filter({ hasText: "Saving…" }),
  ).toBeVisible();
  await tabB.click();
  await expect(body).toHaveText("B original");
  await expect(
    page.getByRole("status").filter({ hasText: "Saving…" }),
  ).toHaveCount(0);
  await body.fill("B local");
  await tabA.click();
  await expect(body).toHaveText("A captured");
  releaseSave();
  await expect(save).toBeDisabled();
  await expect(
    page.getByRole("status").filter({ hasText: "Saved" }),
  ).toBeVisible();
  assert.equal(state.versions[1].description, "B original");
  await tabB.click();
  await expect(body).toHaveText("B local");
  await expect(save).toBeEnabled();
  // Conflict must retain this version's local text, and stay out of other tabs.
  failSave = true;
  await save.click();
  await expect(page.getByRole("alert")).toContainText("Version conflict");
  await expect(body).toHaveText("B local");
  await tabA.click();
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect(body).toHaveText("A captured");
  // A sync notification from another screen refreshes persisted current text,
  // while preserving dirty text and the conflict's original base revision.
  const previousGets = gets;
  state = {
    ...state,
    current_revision: 2,
    versions: state.versions.map((item) =>
      item.id === "a"
        ? { ...item, description: "A external", revision: 3 }
        : item,
    ),
  };
  await page.evaluate(() => window.observeRevision("external-2"));
  await expect.poll(() => gets).toBeGreaterThan(previousGets);
  await expect(page.locator("#current-description")).toHaveText("A external");
  await expect(body).toHaveText("A external");
  await tabB.click();
  await expect(body).toHaveText("B local");
  // Visual fixture states at narrow widths and both themes.
  await mkdir("test-results/issue-versions", { recursive: true });
  for (const dark of [false, true]) {
    await page.evaluate(
      (value) => document.documentElement.classList.toggle("dark", value),
      dark,
    );
    await page.setViewportSize({ width: 390, height: 844 });
    await page.screenshot({
      path: `test-results/issue-versions/${dark ? "dark" : "light"}-mobile.png`,
      fullPage: true,
    });
  }
  // Another screen created a version that this screen has never observed.
  state = {
    ...state,
    versions: [
      ...state.versions,
      {
        id: "unobserved",
        issue_id: "issue",
        name: "Other screen",
        description: "Other draft",
        revision: 1,
      },
    ],
  };
  await page
    .getByRole("textbox", { name: "New version name", exact: true })
    .fill("My new version");
  await page
    .getByRole("button", { name: "Create an empty version", exact: true })
    .click();
  await expect(
    page.getByRole("tab", { name: "My new version", exact: true }),
  ).toHaveAttribute("aria-selected", "true");
  await expect(
    page.getByRole("tab", { name: "Other screen", exact: true }),
  ).toHaveAttribute("aria-selected", "false");
  await expect(body).toHaveText("");
  // Renaming an unchanged body must omit the compatibility body write,
  // preserving even an existing empty string rather than replacing it with null.
  failSave = false;
  autoRelease = true;
  const previousBodies = state.versions.map((item) => [
    item.id,
    item.description,
  ]);
  await page
    .getByRole("textbox", { name: "Version name", exact: true })
    .fill("Renamed empty");
  await save.click();
  await expect(
    page.getByRole("tab", { name: "Renamed empty", exact: true }),
  ).toHaveAttribute("aria-selected", "true");
  await expect(save).toBeDisabled();
  await expect
    .poll(() => state.versions.find((item) => item.id === "c")?.name)
    .toBe("Renamed empty");
  assert.equal(Object.hasOwn(lastPatch, "description"), false);
  assert.deepEqual(
    state.versions.map((item) => [item.id, item.description]),
    previousBodies,
  );
  await tabB.click();
  await expect(body).toHaveText("B local");
  assert.deepEqual(errors, []);
  console.log(
    "PASS: delayed save, tab isolation, conflict retention, external revision refresh, exact created version selection, name-only body preservation; production editor, light/dark mobile screenshots",
  );
} finally {
  await browser.close();
}
