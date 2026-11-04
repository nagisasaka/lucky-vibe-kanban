// Real React components, browser decoding and ZIP parser; mocked preferences/attachment transport only.
import assert from "node:assert/strict";
import { build } from "esbuild";
import { chromium, expect } from "@playwright/test";
import { createServer } from "node:http";
import { mkdtemp, mkdir, readFile, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { execFileSync } from "node:child_process";
const root = process.cwd(),
  output = path.resolve("test-results/pets"),
  scratch = await mkdtemp(path.join(tmpdir(), "lvk-pets-"));
await mkdir(output, { recursive: true });
let server, browser;
const errors = [],
  cases = [];
let uploads = 0;
try {
  const bundle = await build({
    entryPoints: ["tests/pets/browser.fixture.tsx"],
    bundle: true,
    write: false,
    format: "iife",
    jsx: "automatic",
    define: { "import.meta.env.DEV": "false" },
    alias: {
      "@/shared/stores/useUiPreferencesStore": path.join(
        root,
        "tests/pets/store.fixture.ts",
      ),
      "@/shared/lib/petPreferencesPersistence": path.join(root, "tests/pets/api.fixture.ts"),
      "@/shared/lib/api": path.join(root, "tests/pets/api.fixture.ts"),
      "@": path.join(root, "packages/web-core/src"),
    },
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
  const css = await readFile(path.join(scratch, "style.css")),
    atlas = await readFile(
      "packages/public/pets/lvk-rabbit/spritesheet.webp",
    );
  server = createServer((req, res) => {
    if (req.url === "/missing-import.webp") { res.writeHead(404); res.end(); return; }
    if (req.url === "/fixture-upload") {
      uploads++;
      req.resume();
      res.end("{}");
      return;
    }
    const [mime, body] =
      req.url === "/app.js"
        ? ["text/javascript", bundle.outputFiles[0].text]
        : req.url === "/style.css"
          ? ["text/css", css]
          : req.url === "/pets/lvk-rabbit/spritesheet.webp"
            ? ["image/webp", atlas]
            : [
                "text/html",
                '<!doctype html><html><head><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/style.css"></head><body><div id="root"></div><script src="/app.js"></script></body></html>',
              ];
    res.setHeader("Content-Type", mime);
    res.end(body);
  });
  await new Promise((r) => server.listen(0, "127.0.0.1", r));
  const base = `http://127.0.0.1:${server.address().port}`;
  browser = await chromium.launch({
    headless: true,
    executablePath: process.env.LVK_E2E_CHROMIUM,
  });
  const page = await browser.newPage({
    viewport: { width: 1200, height: 780 },
  });
  page.on("pageerror", (e) => errors.push(e.message));
  await page.route("**/*", (r) =>
    new URL(r.request().url()).origin === base ? r.continue() : r.abort(),
  );
  await page.goto(base);
  const sidebar = page.getByTestId("sidebar-pet"),
    pet = sidebar.getByRole("button", { name: "LVK Rabbit", exact: true });
  await expect(pet).toBeVisible();
  await expect(pet).toHaveCSS("width", "80px");
  await expect(pet).toHaveCSS("height", "80px");
  await expect(pet.locator("img")).toHaveCSS("height", "720px");
  await expect(pet.locator("img")).toHaveJSProperty("naturalWidth", 1536);
  const states = [
    "idle",
    "running-right",
    "running-left",
    "waving",
    "jumping",
    "failed",
    "waiting",
    "running",
    "review",
  ];
  for (const [row, state] of states.entries()) {
    const sprite = page.getByRole("button", { name: state, exact: true });
    await expect(sprite).toHaveAttribute("data-pet-state", state);
    await expect(sprite).toHaveCSS("width", "64px");
    await expect(sprite).toHaveCSS("height", "64px");
    await expect(sprite.locator("img")).toHaveCSS("top", `${-row * 64}px`);
    await expect
      .poll(() => sprite.getAttribute("data-pet-frame"))
      .not.toBe("0");
  }
  cases.push(
    "nine states: image decode, correct row and real timer frame progression",
  );
  await sidebar.locator("summary").first().click();
  for (const [row, state] of states.entries()) {
    await sidebar.getByLabel("Animation preview").selectOption(state);
    await expect(pet.locator("img")).toHaveCSS("top", `${-row * 80}px`);
  }
  await sidebar.getByLabel("Animation preview").selectOption("idle");
  await sidebar.locator("summary").first().click();
  cases.push("sidebar body 80px; nine rows scaled consistently; default sprites stay 64px");
  await pet.click();
  await expect(pet).toHaveAttribute("data-pet-state", "waving");
  await expect(pet).toHaveAttribute("data-pet-state", "idle", {
    timeout: 2000,
  });
  cases.push("click waving reaction returns to idle");
  for (const dark of [false, true]) {
    await page.evaluate(
      (d) => document.documentElement.classList.toggle("dark", d),
      dark,
    );
    await page.screenshot({
      path: path.join(output, dark ? "dark.png" : "light.png"),
    });
  }
  await page.emulateMedia({ reducedMotion: "reduce" });
  for (const state of states)
    await expect(
      page.getByRole("button", { name: state, exact: true }),
    ).toHaveAttribute("data-pet-frame", "0");
  await page.waitForTimeout(400);
  await expect(pet).toHaveAttribute("data-pet-frame", "0");
  cases.push("reduced motion stops all nine at frame zero");
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await sidebar.locator("summary").first().click();
  const show = sidebar.getByRole("checkbox");
  await show.uncheck();
  await expect(pet).toHaveCount(0);
  await page.reload();
  await expect(sidebar.locator("[data-pet-state]")).toHaveCount(0);
  await sidebar.locator("summary").first().click();
  await sidebar.getByRole("button", { name: "Restore LVK Rabbit" }).click();
  await expect(pet).toBeVisible();
  cases.push("hidden preference survives reload; reset restores standard pet");
  const zip = path.join(scratch, "pet.zip");
  execFileSync("python3", [
    "-c",
    `import zipfile,sys,json\nfrom pathlib import Path\np=Path(sys.argv[1]);o=Path(sys.argv[2])\nm=json.loads((p/'pet.json').read_text());m['id']='fixture-rabbit';m['displayName']='Imported fixture rabbit';m['author']='Fixture author';m['license']='CC BY 4.0'\nwith zipfile.ZipFile(o,'w',zipfile.ZIP_DEFLATED) as z:\n z.writestr('pet.json',json.dumps(m));z.write(p/'spritesheet.webp','spritesheet.webp');z.write(p/'LICENSE','LICENSE')`,
    "packages/public/pets/lvk-rabbit",
    zip,
  ]);
  await sidebar.locator("input[type=file]").setInputFiles(zip);
  const imported = sidebar.getByRole("button", {
    name: "Imported fixture rabbit",
  });
  await expect(imported).toBeVisible();
  assert.equal(uploads, 1);
  await page.reload();
  await expect(imported).toBeVisible();
  await sidebar.locator("summary").first().click();
  await expect(sidebar.locator("select").first()).toHaveValue(
    /^[0-9a-f-]{36}$/,
  );
  await sidebar.locator("select").first().selectOption("lvk-rabbit");
  await expect(pet).toBeVisible();
  await sidebar
    .locator("select")
    .first()
    .selectOption({ label: "Imported fixture rabbit" });
  await expect(imported).toBeVisible();
  cases.push(
    "real v1 ZIP decode/import, mocked upload, selection and reload restoration",
  );
  await sidebar
    .locator("input[type=file]")
    .setInputFiles({
      name: "broken.zip",
      mimeType: "application/zip",
      buffer: Buffer.from("not zip"),
    });
  await expect(sidebar.getByRole("status")).not.toBeEmpty();
  await expect(imported).toBeVisible();
  assert.equal(uploads, 1);
  cases.push(
    "broken ZIP reports error without losing selected pet or uploading",
  );
  await page.getByRole("button", { name: "Toggle sidebar" }).click();
  await expect(sidebar).toHaveCount(0);
  await page.getByRole("button", { name: "Toggle sidebar" }).click();
  await expect(imported).toBeVisible();
  cases.push("sidebar unmount removes pet; remount keeps selection");
  await sidebar.locator('summary').first().click();
  await page.evaluate(() => localStorage.setItem('pet-save-failure', '1'));
  await sidebar.getByRole('button', {name:'Restore LVK Rabbit'}).click();
  await expect(sidebar.getByRole('status')).toContainText('Could not load or save');
  await expect(imported).toBeVisible();
  await page.reload();
  await expect(imported).toBeVisible();
  await page.evaluate(() => localStorage.removeItem('pet-save-failure'));
  cases.push('failed settings persistence reports error and retains the previous selection');
  await page.evaluate(() => localStorage.setItem('pet-image-failure', '1'));
  await page.reload();
  await expect(pet).toBeVisible();
  await expect(imported).toHaveCount(0);
  await sidebar.locator('summary').first().click();
  await expect(sidebar).toContainText('Showing LVK Rabbit because Imported fixture rabbit could not be loaded');
  await expect(sidebar).toContainText('LVK Rabbit — Masato Fujino');
  await expect(sidebar.locator('select').first()).toHaveValue(/^[0-9a-f-]{36}$/);
  const credits = sidebar.getByText('Retained selection metadata / license (not displayed)', {exact:true});
  await credits.click();
  await expect(sidebar).toContainText('Fixture author');
  cases.push('missing imported image displays Rabbit name and attribution, retaining separate selected-pet credits');
  assert.deepEqual(errors, []);
  console.log(
    JSON.stringify({ status: "passed", cases, errors, uploads }, null, 2),
  );
  await writeFile(
    path.join(output, "results.json"),
    JSON.stringify({ status: "passed", cases, errors, uploads }, null, 2),
  );
} finally {
  await browser?.close();
  if (server) await new Promise((r) => server.close(r));
  await rm(scratch, { recursive: true, force: true });
}
