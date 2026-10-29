// Chromium component E2E: actual landing/layout + UI, fixture data/navigation.
// No live API, DB, authentication or provider is accessed.
import { build } from "esbuild";
import { chromium, expect } from "@playwright/test";
import { createRequire } from "node:module";
import { readFile, mkdir } from "node:fs/promises";
const root = process.cwd();
const require = createRequire(`${root}/packages/web-core/package.json`);
const output = `${root}/.build/mobile-list`;
await mkdir(output, { recursive: true });
const passthrough = (name) => `export const ${name}=({children})=>children;`;
const stubs = {
  useWorkspaceContext: `export {context as useWorkspaceContext} from '${root}/tests/workspace-navigation/mobile-list.fixture.tsx';`,
  WorkspacesSidebarContainer: `export {Sidebar as WorkspacesSidebarContainer} from '${root}/tests/workspace-navigation/mobile-list.fixture.tsx';`,
  usePageTitle: "export const usePageTitle=()=>{};",
  useAppNavigation:
    "export const useAppNavigation=()=>({goToWorkspace:()=>{}});",
  useUserSystem: `export const useUserSystem=()=>({config:{showcases:{seen_features:['workspaces-guide']}},loading:false});`,
  WorkspacesGuideDialog: "export const WorkspacesGuideDialog={};",
  ReviewProvider: passthrough("ReviewProvider"),
  ChangesViewProvider: passthrough("ChangesViewProvider"),
  CreateModeProvider: passthrough("CreateModeProvider"),
  WorkspaceWikiProvider: passthrough("WorkspaceWikiProvider"),
  WorkspacesMainContainer: `import {useWorkspaceContext} from '@/shared/hooks/useWorkspaceContext'; export const WorkspacesMainContainer=()=> <div>{useWorkspaceContext().workspaceId ? 'Detail fixture' : 'Select a workspace to get started'}</div>;`,
  RightSidebar: `export const RightSidebar=()=> <div>Git fixture</div>;`,
  WorkspaceTodoPanel: `export const WorkspaceTodoPanel=()=> <div>TODO fixture</div>;`,
  LogsContentContainer: `export const LogsContentContainer=()=> <div>Logs fixture</div>;`,
  ChangesPanelContainer: `export const ChangesPanelContainer=()=> <div>Diff fixture</div>;`,
  CreateChatBoxContainer: `export const CreateChatBoxContainer=()=> null;`,
  WorkspaceFilesSurfaceContainer: `export const WorkspaceFilesSurfaceContainer=()=> <div>Files fixture</div>;`,
  WorkspaceWikiPanel: `export const WorkspaceWikiNavigation=()=>null; export const WorkspaceWikiArticle=()=>null;`,
};
const bundle = await build({
  entryPoints: ["tests/workspace-navigation/mobile-list.fixture.tsx"],
  bundle: true,
  write: false,
  format: "iife",
  define: { "import.meta.hot": "undefined" },
  jsx: "automatic",
  alias: { "@": `${root}/packages/web-core/src` },
  nodePaths: [`${root}/packages/web-core/node_modules`],
  loader: { ".css": "empty" },
  plugins: [
    {
      name: "inert-services",
      setup(b) {
        b.onResolve({ filter: /.*/ }, (args) => {
          const key = args.path.split("/").at(-1);
          if (stubs[key]) return { path: key, namespace: "stub" };
        });
        b.onLoad({ filter: /.*/, namespace: "stub" }, (args) => ({
          contents: stubs[args.path],
          loader: "tsx",
          resolveDir: root,
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
    content: [
      `${root}/packages/ui/src/**/*.tsx`,
      `${root}/packages/web-core/src/pages/workspaces/*.tsx`,
      `${root}/tests/workspace-navigation/*.tsx`,
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
  args: ["--no-sandbox"],
});
try {
  for (const width of [390, 360, 1440]) {
    const page = await browser.newPage({ viewport: { width, height: 844 } });
    const openList = async () => {
      if (page.viewportSize().width < 768) {
        await page
          .getByRole("button", { name: "Projects", exact: true })
          .click();
        await page
          .getByRole("button", { name: "Workspaces", exact: true })
          .click();
      } else {
        await page
          .getByRole("button", { name: "Local workspaces", exact: true })
          .click();
      }
    };
    page.setDefaultTimeout(5000);
    const errors = [];
    page.on("pageerror", (e) => errors.push(e.message));
    await page.route("https://fixture.invalid/**", (r) =>
      r.fulfill({
        contentType: "text/html",
        body: '<html><body style="margin:0"><div id="root" style="height:100dvh"></div></body></html>',
      }),
    );
    await page.goto("https://fixture.invalid/workspaces");
    if (width === 360)
      await page.evaluate(() => document.documentElement.classList.add("dark"));
    await page.addStyleTag({ content: css.css });
    await page.addScriptTag({ content: bundle.outputFiles[0].text });
    const first = page.getByText("Workspace 0", { exact: true });
    await expect(first).toHaveCount(1);
    await page.screenshot({ path: `${output}/${width}-list.png` });
    console.log(
      `${width}px entry: row exists, visible=${await first.isVisible()}`,
    );
    await expect(first).toBeVisible();
    await expect(first).toBeInViewport();
    if (width < 768)
      await expect(
        page.getByRole("button", { name: "Wksps", exact: true }),
      ).toHaveAttribute("aria-current", "page");
    const last = page.getByText("Workspace 39", { exact: true });
    await last.scrollIntoViewIfNeeded();
    await expect(last).toBeInViewport();
    await last.click();
    await expect(page).toHaveURL(/\/workspaces\/fixture-39$/);
    await expect(
      page.getByText("Detail fixture", { exact: true }),
    ).toBeVisible();
    if (width < 768) {
      for (const [tab, text] of [
        ["Files", "Files fixture"],
        ["Git", "Git fixture"],
        ["TODO", "TODO fixture"],
        ["Logs", "Logs fixture"],
      ]) {
        await page.getByRole("button", { name: tab, exact: true }).click();
        await expect(page.getByText(text, { exact: true })).toBeVisible();
      }
    }
    if (width < 768) {
      await page.setViewportSize({ width: 1440, height: 900 });
      await expect(first).toBeVisible();
      await page.setViewportSize({ width, height: 844 });
      await expect(
        page.getByText("Logs fixture", { exact: true }),
      ).toBeVisible();
    }
    await openList();
    await expect(first).toBeVisible();
    await expect(first).toBeInViewport();
    await page.setViewportSize({ width: 1440, height: 900 });
    await expect(first).toBeVisible();
    await first.click();
    await expect(
      page.getByText("Detail fixture", { exact: true }),
    ).toBeVisible();
    await page.setViewportSize({ width: 360, height: 844 });
    await openList();
    await expect(first).toBeInViewport();
    await page
      .getByRole("button", { name: "Toggle empty fixture", exact: true })
      .click();
    await expect(first).toHaveCount(0);
    await expect(
      page.getByText("No workspaces", { exact: true }).first(),
    ).toBeVisible();
    await page.setViewportSize({ width: 1440, height: 900 });
    await expect(
      page.getByText("No workspaces", { exact: true }).first(),
    ).toBeVisible();
    expect(errors).toEqual([]);
    await page.close();
    console.log(
      `${width}px: list, scroll, detail, return, resize, empty passed`,
    );
  }
} finally {
  await browser.close();
}
