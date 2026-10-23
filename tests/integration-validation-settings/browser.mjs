// Product component + mock file API. Never connects to a running LVK instance.
import assert from "node:assert/strict";
import { build } from "esbuild";
import { chromium, expect } from "@playwright/test";
import { createServer } from "node:http";
import { mkdtemp, mkdir, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { execFileSync } from "node:child_process";
const root = process.cwd();
const scratch = await mkdtemp(path.join(tmpdir(), "lvk-validation-ui-"));
let browser, server;
const errors = [];
let state = {
  branch: "refs/heads/main",
  head: "a".repeat(40),
  revision: "r1",
  content: JSON.stringify({
    commands: ["echo original", "echo first\necho second"],
  }),
};
let failRead = false,
  failSave = false,
  writes = 0;
try {
  const js = await build({
    entryPoints: ["tests/integration-validation-settings/browser.fixture.tsx"],
    bundle: true,
    write: false,
    format: "esm",
    jsx: "automatic",
    alias: { "@": path.join(root, "packages/web-core/src") },
    nodePaths: [path.join(root, "packages/web-core/node_modules")],
  });
  execFileSync(
    path.join(root, "packages/local-web/node_modules/.bin/tailwindcss"),
    [
      "-c",
      "tailwind.new.config.js",
      "-i",
      "../web-core/src/app/styles/new/index.css",
      "-o",
      path.join(scratch, "style.css"),
    ],
    { cwd: path.join(root, "packages/local-web"), stdio: "pipe" },
  );
  const css = await readFile(path.join(scratch, "style.css"));
  server = createServer(async (req, res) => {
    if (req.url === "/fixture/settings") {
      if (req.method === "PUT") {
        writes++;
        const chunks = [];
        for await (const chunk of req) chunks.push(chunk);
        const input = JSON.parse(Buffer.concat(chunks).toString());
        if (failSave || input.expected_revision !== state.revision) {
          res.writeHead(failSave ? 500 : 409);
          return res.end(
            failSave
              ? "Fixture write failed"
              : "Settings changed; reload before saving",
          );
        }
        state = {
          ...state,
          content: input.content,
          revision: `r${writes + 1}`,
        };
      } else if (failRead) {
        res.writeHead(500);
        return res.end("Fixture read failed");
      }
      res.setHeader("Content-Type", "application/json");
      return res.end(JSON.stringify(state));
    }
    if (req.url === "/app.js") {
      res.setHeader("Content-Type", "text/javascript");
      return res.end(js.outputFiles[0].text);
    }
    if (req.url === "/style.css") {
      res.setHeader("Content-Type", "text/css");
      return res.end(css);
    }
    res.setHeader("Content-Type", "text/html");
    res.end(
      '<!doctype html><html><head><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/style.css"></head><body><div id="root"></div><script type="module" src="/app.js"></script></body></html>',
    );
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const base = `http://127.0.0.1:${server.address().port}`;
  browser = await chromium.launch({
    headless: true,
    ...(process.env.LVK_E2E_CHROMIUM
      ? { executablePath: process.env.LVK_E2E_CHROMIUM }
      : {}),
  });
  const page = await browser.newPage();
  page.on("pageerror", (error) => errors.push(error.message));
  await page.route("**/*", (route) =>
    new URL(route.request().url()).origin === base
      ? route.continue()
      : route.abort(),
  );
  const command = (n) =>
    page.getByRole("textbox", { name: new RegExp(`^Command ${n}`) });
  const save = page.getByRole("button", {
    name: "Save validation commands",
    exact: true,
  });
  const reload = page.getByRole("button", {
    name: "Discard draft and reload",
    exact: true,
  });
  await page.goto(base);
  await expect(command(2)).toHaveValue("echo first\necho second");
  await expect(save).toBeDisabled();
  assert.equal(writes, 0);
  await command(1).fill("");
  await expect(save).toBeDisabled();
  await expect(command(1)).toHaveValue("");
  await command(1).fill("echo updated");
  await expect(page.getByTestId("dirty")).toHaveText("true");
  failSave = true;
  await save.click();
  await expect(page.getByRole("alert")).toContainText("Fixture write failed");
  await expect(command(1)).toHaveValue("echo updated");
  failSave = false;
  await save.click();
  await expect(page.getByRole("status")).toContainText(
    "File saved and read back",
  );
  await expect(page.getByTestId("dirty")).toHaveText("false");
  assert.equal(JSON.parse(state.content).commands[0], "echo updated");
  await page.reload();
  await expect(command(1)).toHaveValue("echo updated");
  await command(1).fill("echo my draft");
  state = {
    ...state,
    revision: "external",
    content: '{"commands":["echo external"]}',
  };
  await save.click();
  await expect(page.getByRole("alert")).toContainText("Settings changed");
  await expect(command(1)).toHaveValue("echo my draft");
  failRead = true;
  await reload.click();
  await expect(page.getByRole("alert")).toContainText("Could not load");
  await expect(command(1)).toHaveValue("echo my draft");
  failRead = false;
  await reload.click();
  await expect(command(1)).toHaveValue("echo external");
  await page.getByRole("button", { name: "Add command", exact: true }).click();
  await expect(save).toBeDisabled();
  await command(2).fill("echo added\necho multiline");
  await save.click();
  await expect(save).toBeDisabled();
  assert.deepEqual(JSON.parse(state.content).commands, [
    "echo external",
    "echo added\necho multiline",
  ]);
  state = { ...state, revision: "invalid", content: "{ invalid" };
  await reload.click();
  const json = page.getByRole("textbox", {
    name: "Configuration JSON (advanced / repair)",
    exact: true,
  });
  await expect(json).toHaveValue("{ invalid");
  await json.fill('{"commands":["echo repaired"]}');
  await save.click();
  await expect(page.getByRole("status")).toContainText("File saved");
  state = { ...state, revision: "missing", content: null };
  await reload.click();
  await expect(page.getByRole("status")).toContainText("No configuration");
  await command(1).fill("echo created");
  await save.click();
  await expect(page.getByRole("status")).toContainText("File saved");
  await page
    .getByRole("button", { name: "Remove command", exact: true })
    .click();
  await expect(save).toBeDisabled();
  await page.getByRole("button", { name: "Add command", exact: true }).click();
  await command(1).fill("echo created");
  await expect(save).toBeDisabled();
  const evidence = path.join(
    root,
    "test-results/integration-validation-settings",
  );
  await mkdir(evidence, { recursive: true });
  for (const [width, dark, lang] of [
    [1280, false, "en"],
    [375, true, "ja"],
  ]) {
    await page.setViewportSize({ width, height: 900 });
    await page.goto(`${base}/?lang=${lang}`);
    await expect(page.getByRole("textbox").last()).toHaveValue("echo created");
    await page.evaluate(
      (dark) => document.documentElement.classList.toggle("dark", dark),
      dark,
    );
    await page.screenshot({
      path: path.join(evidence, `${lang}-${width}.png`),
      fullPage: true,
    });
    const action = page.getByRole("button").last();
    await expect(action).toBeVisible();
    const colour = await action.evaluate((el) => getComputedStyle(el).color);
    if (dark)
      assert.notEqual(
        colour,
        "rgb(33, 33, 33)",
        "action label must be legible in dark mode",
      );
    const scrollWidth = await page.evaluate(
      () => document.documentElement.scrollWidth,
    );
    assert.ok(
      scrollWidth <= width,
      `horizontal overflow ${scrollWidth} > ${width}`,
    );
  }
  assert.deepEqual(errors, []);
  console.log(
    "PASS: read/multiline, invalid input, save/readback, write failure, stale revision, reload failure retains draft, repair, missing file, add command, EN/JA desktop/mobile themes; no runtime API calls.",
  );
} finally {
  await browser?.close();
  if (server) await new Promise((resolve) => server.close(resolve));
  await rm(scratch, { recursive: true, force: true });
}
