// Real page ownership and pane branches, isolated data providers and speech API.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { createRequire } from "node:module";
import path from "node:path";
import { build } from "esbuild";
import { chromium, expect } from "@playwright/test";
const root = process.cwd();
const require = createRequire(
  path.join(root, "packages/web-core/package.json"),
);
const ts = require("typescript");
const source = await readFile(
  "packages/web-core/src/pages/kanban/ProjectKanban.tsx",
  "utf8",
);
const ast = ts.createSourceFile(
  "page.tsx",
  source,
  ts.ScriptTarget.Latest,
  true,
  ts.ScriptKind.TSX,
);
const names = ["ProjectKanbanPanes", "getClampedKanbanLeftPanelSize"];
const parts = ast.statements
  .filter((n) => ts.isFunctionDeclaration(n) && names.includes(n.name?.text))
  .map((n) => n.getText(ast))
  .join("\n");
assert.equal(
  names.filter((n) => parts.includes(`function ${n}`)).length,
  names.length,
);
const shell = await readFile(
  "packages/web-core/src/shared/components/ui-new/containers/SharedAppLayout.tsx",
  "utf8",
);
assert.match(
  shell,
  /<RunSpeechProvider key=\{boardProjectId\} projectId=\{boardProjectId \?\? ''\}>\s*\{content\}/,
);
const fixture = `
import React, {useState} from 'react';
import {createRoot} from 'react-dom/client';
import {Group,Panel,Separator} from 'react-resizable-panels';
import i18n from 'i18next';import {initReactI18next} from 'react-i18next';
import {RunSpeechProvider} from './packages/web-core/src/features/kanban/ui/RunSpeechProvider';
import {RunSpeechToggle} from './packages/web-core/src/features/kanban/ui/RunSpeechToggle';
i18n.use(initReactI18next).init({lng:'en',resources:{en:{common:{}}},initImmediate:false});
window.spoken=[];window.cancelled=0;
Object.defineProperty(window,'speechSynthesis',{value:{getVoices:()=>[],speak:u=>window.spoken.push(u.text),cancel:()=>window.cancelled++}});
window.SpeechSynthesisUtterance=class {constructor(text){this.text=text;}};
let state;const useCurrentKanbanRouteState=()=>({projectId:state.project,hostId:null,isPanelOpen:state.panel});
const useProjectContext=()=>({projectId:state.project,getIssue:()=>null});
const useIsMobile=()=>state.mobile;const useTodoViewStore=()=>false;const usePageTitle=()=>{};
const usePaneSize=()=>[75,()=>{}];const PERSIST_KEYS={};
const KANBAN_DEFAULT_LEFT_PANEL_SIZE=75,KANBAN_BOARD_MIN_PERCENT=35,KANBAN_BOARD_MIN_SIZE='35%',KANBAN_RIGHT_PANEL_MIN_SIZE='25%';
const ProjectKanbanBoard=()=>state.loading?null:<RunSpeechToggle projectId={state.project}/>;
const ProjectRightSidebarContainer=()=> <p>Issue / Workspace detail</p>;
const ProjectKanban=()=> <ProjectKanbanPanes projectName="Fixture" utilityBody={null}/>;
${parts}
function App(){const [s,set]=useState({project:'a',panel:false,mobile:true,loading:false,open:true,route:0});state=s;window.change=patch=>set(s=>({...s,...patch}));return <RunSpeechProvider key={s.open?s.project:null} projectId={s.open?s.project:""}>{s.open?<ProjectKanban key={s.route}/>:null}</RunSpeechProvider>;}
createRoot(document.getElementById('root')).render(<App/>);
`;
const js = await build({
  stdin: { contents: fixture, loader: "tsx", resolveDir: root },
  bundle: true,
  write: false,
  jsx: "automatic",
  format: "iife",
  alias: { "@": path.join(root, "packages/web-core/src") },
  nodePaths: [path.join(root, "packages/web-core/node_modules")],
  plugins: process.env.RUN_SPEECH_BEFORE_FIX
    ? [
        {
          name: "before-fix",
          setup(build) {
            build.onLoad({ filter: /RunSpeechProvider\.tsx$/ }, () => ({
              contents:
                "export const RunSpeechProvider=({children})=>children;",
              loader: "tsx",
            }));
            build.onLoad({ filter: /RunSpeechToggle\.tsx$/ }, () => ({
              contents: execFileSync(
                "git",
                [
                  "show",
                  "200a7669:packages/web-core/src/features/kanban/ui/RunSpeechToggle.tsx",
                ],
                { encoding: "utf8" },
              ),
              loader: "tsx",
              resolveDir: path.join(
                root,
                "packages/web-core/src/features/kanban/ui",
              ),
            }));
          },
        },
      ]
    : [],
});
let browser;
try {
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext();
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  let cursor = 10,
    offline = false;
  const events = [];
  const requests = [];
  const event = () => {
    cursor++;
    events.push({
      cursor,
      run_id: "run-" + cursor,
      attempt: 1,
      issue: "TEST-" + cursor,
      payload: { type: "lifecycle_changed", data: { status: "succeeded" } },
    });
  };
  await context.route("**/*", (route) => {
    const url = new URL(route.request().url());
    if (url.pathname.includes("run-observations")) {
      requests.push(url.pathname + url.search);
      if (offline) return route.fulfill({ status: 503, body: "" });
      const after = url.searchParams.get("after");
      return route.fulfill({
        json: {
          cursor,
          events: after ? events.filter((e) => e.cursor > Number(after)) : [],
          pending: [],
          active: [],
        },
      });
    }
    return route.fulfill({
      contentType: "text/html",
      body: `<div id="root"></div><script>${js.outputFiles[0].text}</script>`,
    });
  });
  await page.goto("http://fixture.invalid");
  const button = page.getByRole("button");
  const change = (patch) => page.evaluate((p) => window.change(p), patch);
  await expect(button).toHaveAttribute("aria-pressed", "false");
  assert.equal(requests.length, 0);
  await button.click();
  await expect(button).toHaveAttribute("aria-pressed", "true");
  const other = await context.newPage();
  await other.goto("http://fixture.invalid");
  await expect(other.getByRole("button")).toHaveAttribute(
    "aria-pressed",
    "false",
  );
  await other.close();
  const cancellations = await page.evaluate(() => window.cancelled);
  // The actual mobile issue/workspace branch removes the whole board.
  await change({ panel: true });
  await expect(button).toHaveCount(0);
  await change({ panel: false });
  await expect(button).toHaveAttribute("aria-pressed", "true");
  await change({ route: 1 });
  await expect(button).toHaveAttribute("aria-pressed", "true");
  await change({ mobile: false });
  await expect(button).toHaveAttribute("aria-pressed", "true");
  await change({ loading: true });
  await expect(button).toHaveCount(0);
  await change({ loading: false });
  await expect(button).toHaveAttribute("aria-pressed", "true");
  assert.equal(await page.evaluate(() => window.cancelled), cancellations);
  // Only one initial subscription despite all internal remounts.
  assert.equal(requests.filter((r) => !r.includes("?")).length, 1);
  event();
  await expect.poll(() => page.evaluate(() => window.spoken.length)).toBe(1);
  // Explicit off immediately cancels and clears queued utterances.
  event();
  await page.waitForTimeout(1100);
  await button.click();
  await expect(button).toHaveAttribute("aria-pressed", "false");
  assert.ok((await page.evaluate(() => window.cancelled)) > cancellations);
  await change({ mobile: true });
  await expect(button).toHaveAttribute("aria-pressed", "false");
  event();
  await button.click();
  await expect(button).toHaveAttribute("aria-pressed", "true");
  await page.waitForTimeout(1100);
  assert.equal(await page.evaluate(() => window.spoken.length), 1);
  // Failed polling keeps the choice, then establishes a fresh baseline.
  const beforeOffline = requests.length;
  offline = true;
  await expect.poll(() => requests.length).toBeGreaterThan(beforeOffline);
  event();
  offline = false;
  await expect
    .poll(() => requests.filter((r) => !r.includes("?")).length)
    .toBe(3);
  await expect(button).toHaveAttribute("aria-pressed", "true");
  assert.equal(await page.evaluate(() => window.spoken.length), 1);
  event();
  await expect.poll(() => page.evaluate(() => window.spoken.length)).toBe(2);
  // Different project and newly opened board start off; old subscription stops.
  await change({ project: "b" });
  await expect(button).toHaveAttribute("aria-pressed", "false");
  const count = requests.length;
  await page.waitForTimeout(1200);
  assert.equal(requests.length, count);
  await button.click();
  await expect(button).toHaveAttribute("aria-pressed", "true");
  await expect.poll(() => requests.slice(count).length).toBeGreaterThan(1);
  assert.ok(
    requests.slice(count).every((r) => r.startsWith("/api/projects/b/")),
  );
  await change({ project: "a" });
  await expect(button).toHaveAttribute("aria-pressed", "false");
  await button.click();
  await expect(button).toHaveAttribute("aria-pressed", "true");
  await change({ open: false });
  await expect(button).toHaveCount(0);
  await change({ open: true });
  await expect(button).toHaveAttribute("aria-pressed", "false");

  assert.deepEqual(errors, []);
  console.log(
    "PASS: mobile detail, responsive/loading remounts, explicit off, fresh baseline, reconnect, project and page lifetime (Chromium; mocked speech).",
  );
} finally {
  await browser?.close();
}
