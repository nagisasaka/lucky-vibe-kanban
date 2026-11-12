// Inert browser fixture: real help UI and transport; synthetic API, no DB/provider.
import assert from "node:assert/strict";
import { build } from "esbuild";
import { chromium, expect } from "@playwright/test";
import { createRequire } from "node:module";
import { readFile, mkdir } from "node:fs/promises";
const root = process.cwd();
const output = `${root}/.build/help-review`;
await mkdir(output, { recursive: true });
const require = createRequire(`${root}/packages/web-core/package.json`);
const bundle = await build({
  entryPoints: ["tests/help/fixture.tsx"],
  bundle: true,
  write: false,
  format: "iife",
  jsx: "automatic",
  alias: { "@": `${root}/packages/web-core/src`, shared: `${root}/shared` },
  nodePaths: [`${root}/packages/web-core/node_modules`],
  loader: { ".css": "empty", ".woff2": "empty", ".woff": "empty" },
  define: { "import.meta.env": "{}", "import.meta.hot": "undefined" },
});
const config = require("tailwindcss/loadConfig")(
  `${root}/packages/local-web/tailwind.new.config.js`,
);
const rawCss = await readFile(
  "packages/web-core/src/app/styles/new/index.css",
  "utf8",
);
const css = await require("postcss")([
  require("tailwindcss")({
    ...config,
    content: [
      `${root}/packages/{ui,web-core}/src/**/*.{tsx,ts}`,
      `${root}/tests/help/*.tsx`,
    ],
  }),
]).process(rawCss.replace(/^@(import|config).*$/gm, ""), { from: undefined });
const browser = await chromium.launch({
  headless: true,
  args: ["--no-sandbox"],
});
try {
  const page = await browser.newPage({
    viewport: { width: 1100, height: 800 },
  });
  const errors = [];
  const calls = [];
  page.on("pageerror", (error) => errors.push(error.message));
  let exists = false;
  let status = "running";
  let question = "";
  const session = {
    id: "help-session",
    workspace_id: "help-workspace",
    name: "LVK Help",
    created_at: "2026-11-11T00:00:00Z",
  };
  const state = () => ({
    status,
    terminal_output:
      status === "succeeded"
        ? {
            content:
              "See `CONTRIBUTING.md` at **release-sha**.\n\nThe author wrote “preserve history” in commit `previous-sha`; interpreting this as a maintenance goal is an inference.",
          }
        : null,
    last_error:
      status === "failed"
        ? { message: "Provider authentication required" }
        : null,
  });
  await page.route("https://fixture.invalid/**", async (route) => {
    const request = route.request();
    const path = new URL(request.url()).pathname;
    if (!path.startsWith("/api/"))
      return route.fulfill({
        contentType: "text/html",
        body: '<div id="root"></div>',
      });
    calls.push(`${request.method()} ${path}`);
    const respond = (data) =>
      route.fulfill({
        contentType: "application/json",
        body: JSON.stringify({ success: true, data }),
      });
    if (path === "/api/help/conversations" && request.method() === "GET")
      return respond(exists ? [session] : []);
    if (path === "/api/help/conversations" && request.method() === "POST") {
      question = request.postDataJSON().prompt;
      if (question === "missing source")
        return route.fulfill({
          status: 400,
          contentType: "application/json",
          body: JSON.stringify({
            success: false,
            message: "Bundled LVK source unavailable",
          }),
        });
      exists = true;
      status = "running";
      return respond({ session_id: session.id });
    }
    if (request.method() === "DELETE") {
      exists = false;
      return respond(null);
    }
    if (path.endsWith("/cancel")) {
      status = "cancelled";
      return respond(null);
    }
    return respond({
      workspace_id: session.workspace_id,
      session_id: session.id,
      source: { build_revision: "release-sha", commit: "release-sha" },
      questions: [
        {
          agent_run_id: "run",
          prompt: question,
          source_commit: "old-sha",
          build_revision: "old-build",
        },
      ],
      runs: [{ agent_run_id: "run", state: state() }],
    });
  });
  await page.goto("https://fixture.invalid");
  await page.addStyleTag({ content: css.css });
  await page.addScriptTag({ content: bundle.outputFiles[0].text });
  const input = page.getByLabel("Your question", { exact: true });
  await input.fill("How do I send a PR?");
  await page.getByRole("button", { name: "Ask", exact: true }).click();
  await expect(page.getByText("Investigating…")).toBeVisible();
  await expect(input).toBeDisabled();
  status = "succeeded";
  await expect(page.getByText("Answered", { exact: true })).toBeVisible();
  await expect(
    page.getByText("How do I send a PR?", { exact: true }),
  ).toBeVisible();
  await expect(input).toBeEnabled();
  await expect(
    page.getByText("Current distribution: release-sha · release-sha"),
  ).toBeVisible();
  await expect(
    page.getByText("Source for this question and answer: old-build · old-sha"),
  ).toBeVisible();
  await expect(
    page.getByRole("link", {
      name: "Open input, approvals and execution details",
    }),
  ).toHaveAttribute("href", "/workspaces/help-workspace");
  for (const width of [1100, 390]) {
    await page.setViewportSize({ width, height: 800 });
    if (width === 390)
      await page.getByRole("button", { name: "Ask LVK", exact: true }).click();
    for (const theme of ["light", "dark"]) {
      await page.evaluate(
        (theme) =>
          document.documentElement.classList.toggle("dark", theme === "dark"),
        theme,
      );
      await expect(input).toBeVisible();
      await page.screenshot({
        path: `${output}/${theme}-${width}.png`,
        fullPage: true,
      });
      assert.equal(
        await page.evaluate(
          () => document.documentElement.scrollWidth > innerWidth,
        ),
        false,
      );
    }
  }
  await input.fill("continue");
  await page.getByRole("button", { name: "Ask", exact: true }).click();
  await expect(page.getByText("Investigating…")).toBeVisible();
  await page
    .getByRole("button", { name: "Cancel answer", exact: true })
    .click();
  await expect(page.getByText("Cancelled", { exact: true })).toBeVisible();
  await input.fill("retry");
  await page.getByRole("button", { name: "Ask", exact: true }).click();
  await expect(page.getByText("Investigating…")).toBeVisible();
  status = "failed";
  await expect(page.getByRole("alert")).toHaveText(
    "Provider authentication required",
  );
  await expect(page.getByText("Answered", { exact: true })).toHaveCount(0);
  await page
    .getByRole("button", { name: "Delete conversation", exact: true })
    .click();
  await expect(page.getByText("Provider authentication required")).toHaveCount(
    0,
  );
  await input.fill("missing source");
  await page.getByRole("button", { name: "Ask", exact: true }).click();
  await expect(page.getByRole("alert")).toContainText(
    "Bundled LVK source unavailable",
  );
  assert(calls.includes("POST /api/help/conversations/help-session/cancel"));
  assert(calls.includes("DELETE /api/help/conversations/help-session"));
  assert.deepEqual(errors, []);
  console.log(
    "Help mock browser fixture passed: ask, progress, answer, continuation, cancel, failure, delete; light/dark wide/narrow screenshots. No real provider/API/auth validation.",
  );
} finally {
  await browser.close();
}
