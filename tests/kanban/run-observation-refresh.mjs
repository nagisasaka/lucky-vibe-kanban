// Card refresh from the shared Run observation: real provider, hook, Issue
// collection and 30s fallback code against a mocked server and speech API.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { build } from "esbuild";
import { chromium, expect } from "@playwright/test";
const root = process.cwd();
const page_source = await readFile(
  "packages/web-core/src/pages/kanban/ProjectKanban.tsx",
  "utf8",
);
// The real board page owns the subscription above pane and loading remounts.
assert.match(
  page_source,
  /function ProjectKanbanLayout[^]*?useRunObservationIssueRefresh\(projectId\);/,
);
const fixture = `
import React, {useState} from 'react';
import {createRoot} from 'react-dom/client';
import i18n from 'i18next';import {initReactI18next} from 'react-i18next';
import {PROJECT_ISSUES_SHAPE, ISSUE_MUTATION} from 'shared/remote-types';
import {useShape} from './packages/web-core/src/shared/integrations/electric/hooks';
import {RunSpeechProvider} from './packages/web-core/src/features/kanban/ui/RunSpeechProvider';
import {RunSpeechToggle} from './packages/web-core/src/features/kanban/ui/RunSpeechToggle';
import {useRunObservationIssueRefresh} from './packages/web-core/src/features/kanban/ui/useRunObservationIssueRefresh';
i18n.use(initReactI18next).init({lng:'en',resources:{en:{common:{}}},initImmediate:false});
window.spoken=[];window.changes=[];
Object.defineProperty(window,'speechSynthesis',{value:{getVoices:()=>[],speak:u=>{window.spoken.push(u.text);setTimeout(()=>u.onend?.(),0);},cancel:()=>{}}});
window.SpeechSynthesisUtterance=class {constructor(text){this.text=text;}};
// Record the 30s fallback poll but keep it from firing during exact counts.
const setIntervalNative=window.setInterval.bind(window),clearIntervalNative=window.clearInterval.bind(window);window.fallbackPolls=new Set();
window.setInterval=(fn,ms,...args)=>{if(ms!==30000)return setIntervalNative(fn,ms,...args);const id=setIntervalNative(()=>{},1e9);window.fallbackPolls.add(id);return id;};
window.clearInterval=id=>{window.fallbackPolls.delete(id);clearIntervalNative(id);};
function Board({projectId}){
  useRunObservationIssueRefresh(projectId);
  const {data}=useShape(PROJECT_ISSUES_SHAPE,{project_id:projectId},{mutation:ISSUE_MUTATION});
  return <ul>{data.map(i=><li key={i.id} data-issue={i.id} data-status={i.status_id}>{i.status_id}</li>)}</ul>;
}
function App(){
  const [s,set]=useState({project:'a',board:true});window.change=p=>set(s=>({...s,...p}));
  return <RunSpeechProvider key={s.project} projectId={s.project}>
    <RunSpeechToggle projectId={s.project}/>
    <button data-start onClick={()=>fetch('/api/fixture/start',{method:'POST'})}>start</button>
    {s.board?<Board projectId={s.project}/>:null}
  </RunSpeechProvider>;
}
new MutationObserver(()=>{for(const li of document.querySelectorAll('[data-issue]')){const key=li.dataset.issue+'='+li.dataset.status;if(!window.changes.some(c=>c.key===key))window.changes.push({key,at:Date.now()});}}).observe(document.getElementById('root'),{subtree:true,childList:true,attributes:true,characterData:true});
createRoot(document.getElementById('root')).render(<App/>);
`;
const js = await build({
  stdin: { contents: fixture, loader: "tsx", resolveDir: root },
  bundle: true,
  write: false,
  jsx: "automatic",
  format: "iife",
  alias: {
    "@": path.join(root, "packages/web-core/src"),
    shared: path.join(root, "shared"),
  },
  nodePaths: [path.join(root, "packages/web-core/node_modules")],
  loader: { ".css": "empty" },
  logLevel: "error",
  // Without the hook the board only has its 30s fallback poll.
  plugins: process.env.RUN_OBSERVATION_BEFORE_FIX
    ? [
        {
          name: "before-fix",
          setup(build) {
            build.onLoad(
              { filter: /useRunObservationIssueRefresh\.ts$/ },
              () => ({
                contents: "export const useRunObservationIssueRefresh=()=>{};",
                loader: "ts",
              }),
            );
          },
        },
      ]
    : [],
});
const statuses = {
  todo: "Not Started",
  progress: "In Progress",
  review: "In Review",
};
const server = {
  online: true,
  projects: {
    a: { cursor: 50, events: [], issues: { "issue-a": "todo" } },
    b: { cursor: 900, events: [], issues: { "issue-b": "todo" } },
  },
};
const requests = [];
let browser;
const evidence = [];
try {
  browser = await chromium.launch({ headless: true, args: ["--no-sandbox"] });
  const context = await browser.newContext();
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  if (process.env.DEBUG_FIXTURE)
    page.on("console", (m) => console.error("console:", m.text()));
  const emit = (project, run, status, attempt = 1) => {
    const p = server.projects[project];
    p.cursor++;
    p.events.push({
      cursor: p.cursor,
      run_id: run,
      attempt,
      issue: "LOCAL-" + project,
      session_number: 1,
      other_active_sessions: 0,
      provider: "codex",
      payload: { type: "lifecycle_changed", data: { status } },
    });
  };
  // Mirrors the server: the card status and running/terminal event commit together.
  const transition = (project, run, status, issueStatus) => {
    emit(project, run, status);
    if (issueStatus)
      server.projects[project].issues["issue-" + project] = issueStatus;
    return Date.now();
  };
  const startRun = async (project, run) => {
    emit(project, run, "starting");
    await new Promise((r) => setTimeout(r, 150));
    return transition(project, run, "running", "progress");
  };
  await context.route("**/*", async (route) => {
    const url = new URL(route.request().url());
    const observed = url.pathname.match(
      /^\/api\/projects\/(\w+)\/run-observations$/,
    );
    if (observed) {
      requests.push({
        kind: "observe",
        path: url.pathname + url.search,
        at: Date.now(),
      });
      if (!server.online) return route.fulfill({ status: 503, body: "" });
      const p = server.projects[observed[1]];
      const after = url.searchParams.get("after");
      return route.fulfill({
        json: {
          cursor: p.cursor,
          events: after ? p.events.filter((e) => e.cursor > Number(after)) : [],
          pending: [],
          active: [],
        },
      });
    }
    if (url.pathname === "/api/local/v1/fallback/issues") {
      const project = url.searchParams.get("project_id");
      requests.push({ kind: "issues", project, at: Date.now() });
      if (!server.online) return route.fulfill({ status: 503, body: "" });
      const issues = Object.entries(server.projects[project].issues).map(
        ([id, status]) => ({ id, project_id: project, status_id: status }),
      );
      return route.fulfill({ json: { issues } });
    }
    if (url.pathname === "/api/fixture/start") {
      void startRun("a", "board-run").then((at) => (server.boardCommit = at));
      return route.fulfill({ json: {} });
    }
    return route.fulfill({
      contentType: "text/html",
      body: `<div id="root"></div><script>${js.outputFiles[0].text}</script>`,
    });
  });
  const count = (kind, project) =>
    requests.filter(
      (r) => r.kind === kind && (!project || r.project === project),
    ).length;
  const baselines = () =>
    requests.filter((r) => r.kind === "observe" && !r.path.includes("?"))
      .length;
  const card = (id) => page.locator(`[data-issue="${id}"]`);
  const shownAt = async (key) => {
    await expect
      .poll(
        () =>
          page.evaluate(
            (k) => window.changes.find((c) => c.key === k)?.at ?? 0,
            key,
          ),
        { timeout: 5000 },
      )
      .toBeGreaterThan(0);
    return page.evaluate(
      (k) => window.changes.find((c) => c.key === k).at,
      key,
    );
  };
  const toggle = page.locator("button[aria-pressed]");
  // Past history before opening is a baseline only: no replay, no extra refresh.
  emit("a", "old", "running");
  emit("a", "old", "succeeded");
  await page.goto("http://fixture.invalid");
  await expect(card("issue-a")).toHaveAttribute("data-status", "todo");
  assert.equal(count("issues"), 1);
  await expect(toggle).toHaveAttribute("aria-pressed", "false");
  assert.equal(await page.evaluate(() => window.fallbackPolls.size), 1);

  // 1) External/MCP start with speech muted.
  let issuesBefore = count("issues");
  let commit = await startRun("a", "mcp-run");
  let latency = (await shownAt("issue-a=progress")) - commit;
  evidence.push(`MCP/external start, speech off: ${latency}ms`);
  assert.ok(latency < 2500, `card moved after ${latency}ms`);
  assert.equal(count("issues") - issuesBefore, 1);
  assert.ok(count("observe") > 1);
  assert.equal(baselines(), 1);

  // 2) Speech on joins the board observation without a second subscription.
  const observeBefore = count("observe");
  await toggle.click();
  await expect(toggle).toHaveAttribute("aria-pressed", "true");
  assert.equal(baselines(), 1);
  const windowStart = Date.now();
  await page.waitForTimeout(3200);
  const polls = requests.filter(
    (r) => r.kind === "observe" && r.at >= windowStart,
  ).length;
  assert.ok(polls <= 4, `expected one ~1s poll loop, saw ${polls} in 3.2s`);
  assert.ok(count("observe") > observeBefore);

  // 3) Terminal transition with speech on: card and terminal speech.
  issuesBefore = count("issues");
  commit = transition("a", "mcp-run", "succeeded", "review");
  latency = (await shownAt("issue-a=review")) - commit;
  evidence.push(`terminal, speech on: ${latency}ms`);
  assert.ok(latency < 2500, `card moved after ${latency}ms`);
  assert.equal(count("issues") - issuesBefore, 1);
  await expect.poll(() => page.evaluate(() => window.spoken.length)).toBe(1);

  // 4) Board-originated start, speech on; then off keeps card observation.
  server.projects.a.issues["issue-a"] = "todo";
  await page.evaluate(() => (window.changes = []));
  issuesBefore = count("issues");
  await page.locator("[data-start]").click();
  await expect.poll(() => server.boardCommit ?? 0).toBeGreaterThan(0);
  latency = (await shownAt("issue-a=progress")) - server.boardCommit;
  evidence.push(`board start, speech on: ${latency}ms`);
  assert.ok(latency < 2500, `card moved after ${latency}ms`);
  assert.equal(count("issues") - issuesBefore, 1);
  await toggle.click();
  await expect(toggle).toHaveAttribute("aria-pressed", "false");
  issuesBefore = count("issues");
  commit = transition("a", "board-run", "failed", "review");
  latency = (await shownAt("issue-a=review")) - commit;
  evidence.push(`terminal after speech off: ${latency}ms`);
  assert.ok(latency < 2500);
  assert.equal(count("issues") - issuesBefore, 1);

  // 5) Duplicate/related events on one page refresh once; non-status events never.
  issuesBefore = count("issues");
  emit("a", "r1", "starting");
  emit("a", "r1", "running");
  emit("a", "r2", "running");
  emit("a", "r3", "succeeded");
  await expect.poll(() => count("issues") - issuesBefore).toBe(1);
  emit("a", "r1", "awaiting_input");
  emit("a", "r1", "running");
  await page.waitForTimeout(2500);
  assert.equal(count("issues") - issuesBefore, 1);

  // 6) Reconnect: a transition during the outage is recovered by one refresh.
  server.online = false;
  const offlineAt = Date.now();
  await expect
    .poll(
      () =>
        requests.filter((r) => r.kind === "observe" && r.at > offlineAt).length,
    )
    .toBeGreaterThan(1);
  server.projects.a.issues["issue-a"] = "progress";
  emit("a", "r4", "running");
  await page.evaluate(() => (window.changes = []));
  issuesBefore = count("issues");
  server.online = true;
  const onlineAt = Date.now();
  latency = (await shownAt("issue-a=progress")) - onlineAt;
  evidence.push(`reconnect recovery: ${latency}ms`);
  assert.equal(count("issues") - issuesBefore, 1);

  // 7) Hidden tab without speech stops polling; showing it refreshes once.
  await page.evaluate(() => {
    Object.defineProperty(document, "visibilityState", {
      value: "hidden",
      configurable: true,
    });
    document.dispatchEvent(new Event("visibilitychange"));
  });
  await page.waitForTimeout(1200);
  let quietFrom = count("observe");
  await page.waitForTimeout(2200);
  assert.equal(count("observe"), quietFrom, "hidden board keeps polling");
  issuesBefore = count("issues");
  await page.evaluate(() => {
    Object.defineProperty(document, "visibilityState", {
      value: "visible",
      configurable: true,
    });
    document.dispatchEvent(new Event("visibilitychange"));
  });
  await expect.poll(() => count("issues") - issuesBefore).toBe(1);

  // 8) Leaving the board stops observation; speech alone still observes.
  await page.evaluate(() => window.change({ board: false }));
  await page.waitForTimeout(1200);
  quietFrom = count("observe");
  await page.waitForTimeout(2200);
  assert.equal(count("observe"), quietFrom, "closed board keeps polling");
  await toggle.click();
  await expect(toggle).toHaveAttribute("aria-pressed", "true");
  await expect.poll(() => count("observe")).toBeGreaterThan(quietFrom + 1);
  await toggle.click();
  await page.evaluate(() => window.change({ board: true }));

  // 9) Another Project: separate observation, no refresh of the old Project.
  await page.evaluate(() => window.change({ project: "b" }));
  await expect(card("issue-b")).toHaveAttribute("data-status", "todo");
  await page.waitForTimeout(1200);
  const aIssues = count("issues", "a");
  const aObserve = requests.filter((r) =>
    r.path?.includes("/projects/a/"),
  ).length;
  commit = await startRun("b", "b-run");
  latency = (await shownAt("issue-b=progress")) - commit;
  evidence.push(`other Project start: ${latency}ms`);
  emit("a", "late", "running");
  await page.waitForTimeout(2200);
  assert.equal(count("issues", "a"), aIssues);
  assert.equal(
    requests.filter((r) => r.path?.includes("/projects/a/")).length,
    aObserve,
  );

  // 10) Idle board: observation polls, the full Issue list does not.
  issuesBefore = count("issues");
  await page.waitForTimeout(5000);
  assert.equal(count("issues"), issuesBefore);
  // The normal 30s fallback poll remains registered for the open board.
  assert.equal(await page.evaluate(() => window.fallbackPolls.size), 1);
  assert.deepEqual(errors, []);
  console.log(
    `PASS: shared observation card refresh (Chromium; mocked server and speech)\n  ${evidence.join("\n  ")}`,
  );
} finally {
  await browser?.close();
}
