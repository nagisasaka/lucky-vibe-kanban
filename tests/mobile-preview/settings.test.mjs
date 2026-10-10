import { test } from "node:test";
import assert from "node:assert/strict";
import { build } from "esbuild";
import { chromium } from "@playwright/test";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "../..");

test("settings sections load only when selected and retain the surrounding dialog while loading", async () => {
  const result = await build({
    stdin: {
      contents: `
        import React from 'react';
        import { createRoot } from 'react-dom/client';
        import { SettingsSection } from '@/shared/dialogs/settings/settings/SettingsSection';
        const root = createRoot(document.getElementById('root'));
        window.selectSection = type => root.render(<SettingsSection type={type} />);
        window.ready = true;
      `,
      resolveDir: path.join(root, "packages/web-core"),
      loader: "tsx",
    },
    bundle: true,
    splitting: true,
    format: "esm",
    write: false,
    outdir: "/virtual",
    jsx: "automatic",
    alias: { "@": path.join(root, "packages/web-core/src") },
    plugins: [
      {
        name: "isolated-settings",
        setup(builder) {
          builder.onResolve({ filter: /^react-i18next$/ }, () => ({
            path: "translation",
            namespace: "fixture",
          }));
          builder.onLoad({ filter: /.*/, namespace: "fixture" }, () => ({
            contents: "export const useTranslation = () => ({t: key => key});",
          }));
          builder.onLoad(
            { filter: /settings\/[A-Z]\w+SettingsSection\.tsx$/ },
            (args) => {
              const name = path.basename(args.path, ".tsx");
              const exported =
                name === "RelaySettingsSection" ? `${name}Content` : name;
              return {
                contents: `import React from 'react';
          window.loadedSections = [...(window.loadedSections || []), '${name}'];
          export function ${exported}() { return React.createElement('p', null, '${name}'); }`,
                resolveDir: path.dirname(args.path),
                loader: "jsx",
              };
            },
          );
        },
      },
    ],
  });
  const files = new Map(
    result.outputFiles.map((file) => [path.basename(file.path), file.text]),
  );
  const browser = await chromium.launch({
    headless: true,
    args: ["--no-sandbox"],
  });
  try {
    const page = await browser.newPage();
    let release;
    const pending = new Promise((resolve) => {
      release = resolve;
    });
    await page.route("http://fixture.test/**", async (route) => {
      const name = new URL(route.request().url()).pathname.slice(1);
      if (!name)
        return route.fulfill({
          contentType: "text/html",
          body: '<div id="root"></div><script type="module" src="/stdin.js"></script>',
        });
      if (name.startsWith("AgentsSettingsSection-")) await pending;
      await route.fulfill({
        contentType: "text/javascript",
        body: files.get(name),
      });
    });
    await page.goto("http://fixture.test/");
    await page.waitForFunction(() => window.ready);
    assert.deepEqual(
      await page.evaluate(() => window.loadedSections || []),
      [],
    );
    await page.evaluate(() => window.selectSection("agents"));
    await page.getByRole("status").waitFor();
    assert.equal(
      await page.getByRole("heading").textContent(),
      "settings.layout.nav.agents",
    );
    release();
    await page.getByText("AgentsSettingsSection", { exact: true }).waitFor();
    assert.deepEqual(await page.evaluate(() => window.loadedSections), [
      "AgentsSettingsSection",
    ]);
    await page.evaluate(() => window.selectSection("resources"));
    await page.getByText("ResourceSettingsSection", { exact: true }).waitFor();
    assert.deepEqual(await page.evaluate(() => window.loadedSections), [
      "AgentsSettingsSection",
      "ResourceSettingsSection",
    ]);
    await page.evaluate(() => window.selectSection("agents"));
    await page.getByText("AgentsSettingsSection", { exact: true }).waitFor();
    assert.equal(await page.evaluate(() => window.loadedSections.length), 2);
  } finally {
    await browser.close();
  }
});
