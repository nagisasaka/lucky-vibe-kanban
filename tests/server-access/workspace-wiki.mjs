// Built application + real backend; disposable repository content is the only fixture.
// Run after building local-web, server and agent-process-host:
// node tests/server-access/workspace-wiki.mjs /absolute/evidence/directory
import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { once } from 'node:events';
import { mkdir, mkdtemp, rm, writeFile, open } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { createServer } from 'node:net';
import { chromium, expect } from '@playwright/test';

const evidence = resolve(process.argv[2] || 'test-results/project-utilities');
await mkdir(evidence, { recursive: true });
const root = await mkdtemp(join(tmpdir(), 'lvk-workspace-wiki-'));
const reservation = createServer();
await new Promise((done) => reservation.listen(0, '127.0.0.1', done));
const port = reservation.address().port;
await new Promise((done) => reservation.close(done));
const base = `http://127.0.0.1:${port}`;
const log = await open(join(evidence, 'server.log'), 'w');
const server = spawn(resolve('target/debug/server'), [], {
  detached: true,
  stdio: ['ignore', log.fd, log.fd],
  env: {
    ...Object.fromEntries(
      Object.entries(process.env).filter(([key]) => !key.startsWith('LVK_'))
    ),
    LVK_INSTANCE_DIR: join(root, 'instance'),
    LVK_ACCESS_DEPLOYMENT: 'local',
    LVK_AUTH_MODE: 'local',
    LVK_PUBLIC_URL: '',
    HOST: '127.0.0.1',
    BACKEND_PORT: String(port),
    PORT: '',
    PREVIEW_PROXY_PORT: '0',
    RUST_LOG: 'info',
    VK_ALLOWED_ORIGINS: base,
    VK_PREVIEW_DOMAIN: '',
    BROWSER: 'true',
    DISABLE_WORKTREE_CLEANUP: '1',
    VIBE_KANBAN_AGENT_PROCESS_HOST: resolve('target/debug/agent-process-host'),
  },
});
const exited = once(server, 'exit');
let csrf;
let browser;
let page;
const results = {
  base,
  fixture:
    'Temporary repositories and isolated DB; real file/Wiki/PTY APIs, stubbed account quota and injected resource failure; no AI execution',
  checks: [],
};
function descendants() {
  const rows = execFileSync('ps', ['-eo', 'pid=,ppid='], { encoding: 'utf8' })
    .trim()
    .split('\n')
    .map((line) => line.trim().split(/\s+/).map(Number));
  const found = new Set([process.pid]);
  for (let changed = true; changed; ) {
    changed = false;
    for (const [pid, parent] of rows)
      if (found.has(parent) && !found.has(pid)) {
        found.add(pid);
        changed = true;
      }
  }
  found.delete(process.pid);
  return [...found];
}
function alive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    if (error.code !== 'ESRCH') throw error;
    return false;
  }
}
const check = (name) => {
  results.checks.push(name);
  console.log(`PASS ${name}`);
};
async function api(path, method = 'GET', data) {
  const response = await fetch(`${base}/api${path}`, {
    method,
    signal: AbortSignal.timeout(30000),
    headers: {
      'Content-Type': 'application/json',
      Origin: base,
      ...(csrf ? { 'x-lvk-csrf': csrf } : {}),
    },
    body: data === undefined ? undefined : JSON.stringify(data),
  });
  const body = await response.json();
  assert.equal(
    response.ok && body.success,
    true,
    `${method} ${path}: ${JSON.stringify(body)}`
  );
  return body.data;
}
try {
  await expect
    .poll(
      async () => {
        if (server.exitCode !== null)
          throw new Error('Server exited before readiness');
        return fetch(`${base}/api/health`)
          .then((r) => r.status)
          .catch(() => 0);
      },
      { timeout: 120000 }
    )
    .toBe(200);
  const access = await (await fetch(`${base}/api/access/status`)).json();
  assert.equal(access.mode, 'local');
  csrf = access.csrf;
  const info = await api('/info');
  await api('/config', 'PUT', {
    ...info.config,
    onboarding_acknowledged: true,
    disclaimer_acknowledged: true,
    telemetry_acknowledged: true,
    analytics_enabled: false,
    showcases: { seen_features: ['workspaces-guide'] },
    language: 'EN',
  });
  const repoPath = join(root, 'fixture-repo');
  await mkdir(join(repoPath, 'openwiki'), { recursive: true });
  await writeFile(
    join(repoPath, 'openwiki/index.md'),
    '# Wiki E2E Home\n\n[Second page](second.md)\n'
  );
  await writeFile(
    join(repoPath, 'openwiki/second.md'),
    '# Wiki E2E Second\n\n[Home](index.md)\n'
  );
  const git = (...args) =>
    execFileSync('git', ['-C', repoPath, ...args], { stdio: 'pipe' });
  git('init', '-b', 'main');
  git('add', '.');
  git(
    '-c',
    'user.name=Fixture',
    '-c',
    'user.email=fixture@example.invalid',
    'commit',
    '-m',
    'Wiki fixture'
  );
  const repo = await api('/repos', 'POST', {
    path: repoPath,
    display_name: 'Wiki fixture',
  });
  await writeFile(join(repoPath, 'hello.md'), '# Main file preview\n');
  const disabledPath = join(root, 'disabled-repo');
  await mkdir(disabledPath);
  execFileSync('git', ['init', '-b', 'main', disabledPath], { stdio: 'pipe' });
  await writeFile(join(disabledPath, 'README.md'), 'Disabled Wiki fixture\n');
  execFileSync('git', ['-C', disabledPath, 'add', '.']);
  execFileSync(
    'git',
    [
      '-C',
      disabledPath,
      '-c',
      'user.name=Fixture',
      '-c',
      'user.email=fixture@example.invalid',
      'commit',
      '-m',
      'Fixture',
    ],
    { stdio: 'pipe' }
  );
  const disabledRepo = await api('/repos', 'POST', {
    path: disabledPath,
    display_name: 'Disabled Wiki fixture',
  });
  await api(`/repos/${disabledRepo.id}/memory`, 'PUT', {
    enabled: false,
    target_branch: 'main',
    output_language: 'ja',
  });
  const local = async (path, data) => {
    const response = await fetch(`${base}/api/local/v1/${path}`, {
      method: data ? 'POST' : 'GET',
      headers: {
        'Content-Type': 'application/json',
        Origin: base,
        'x-lvk-csrf': csrf,
      },
      body: data ? JSON.stringify(data) : undefined,
    });
    assert.ok(response.ok, `${path}: ${response.status}`);
    return response.json();
  };
  const organizations = await local('organizations');
  const { data: project } = await local('projects', {
    organization_id: organizations.organizations[0].id,
    name: 'Utilities project',
    key: 'UTIL',
    color: '210 80% 52%',
  });
  await api(`/scratch/PROJECT_REPO_DEFAULTS/${project.id}`, 'PUT', {
    payload: {
      type: 'PROJECT_REPO_DEFAULTS',
      data: {
        repos: [
          { repo_id: repo.id, target_branch: 'main' },
          { repo_id: disabledRepo.id, target_branch: 'main' },
        ],
        directory_path: null,
      },
    },
  });
  const mainInfo = await api(`/projects/${project.id}/utilities/info`);
  assert.equal(mainInfo.repos[0].wiki_enabled, true);
  assert.equal(mainInfo.repos[1].wiki_enabled, false);
  assert.equal(
    (
      await api(
        `/projects/${project.id}/utilities/content?repo_id=${repo.id}&path=hello.md`
      )
    ).content,
    '# Main file preview\n'
  );
  for (const path of ['../outside', '/etc/passwd']) {
    const response = await fetch(
      `${base}/api/projects/${project.id}/utilities/content?repo_id=${repo.id}&path=${encodeURIComponent(path)}`
    );
    assert.equal(response.status, 400);
  }
  assert.equal(
    (
      await fetch(
        `${base}/api/projects/${project.id}/utilities/wiki?repo_id=00000000-0000-0000-0000-000000000099`
      )
    ).status,
    400
  );
  assert.equal((await api('/environment-info')).running_sessions, 0);
  check(
    'Registered main repository, traversal/foreign repository rejection, zero running Sessions'
  );
  const workspace = await api('/workspaces', 'POST', { name: 'Wiki pane E2E' });
  await api(`/workspaces/${workspace.id}/repos`, 'POST', {
    repo_id: repo.id,
    target_branch: 'main',
  });
  await api('/sessions', 'POST', {
    workspace_id: workspace.id,
    executor: 'CODEX',
    name: 'Draft preservation',
  });
  const wiki = await api(`/workspaces/${workspace.id}/wiki?repo_id=${repo.id}`);
  assert.equal(wiki.wiki.exists, true);
  check(
    'Real backend: isolated DB, workspace, repository, session and Wiki snapshot'
  );
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    viewport: { width: 1600, height: 1000 },
  });
  await context.tracing.start({
    screenshots: true,
    snapshots: true,
    sources: false,
  });
  page = await context.newPage();
  const errors = [];
  let terminalSockets = 0;
  page.on('websocket', (socket) => {
    if (socket.url().includes('/api/terminal/ws')) terminalSockets++;
  });
  page.on('pageerror', (error) => errors.push(error.message));
  // Only account quota is stubbed: never contact real provider accounts in E2E.
  await page.route('**/api/provider-usage/read', (route) =>
    route.fulfill({
      json: { success: true, data: { result: { status: 'unsupported' } } },
    })
  );
  await page.goto(`${base}/projects/${project.id}`);
  const panel = page.getByTestId('environment-panel');
  await expect(panel).toBeVisible({ timeout: 60000 });
  await expect(panel).toContainText('Running AI Sessions');
  await expect(
    page.locator('header [data-testid="resource-summary"]')
  ).toHaveCount(0);
  await page.getByRole('button', { name: /Hide Right Sidebar/ }).click();
  await expect(panel).toHaveCount(0);
  await page.getByRole('button', { name: /Show Right Sidebar/ }).click();
  await expect(panel).toBeVisible();
  await page.getByRole('button', { name: 'Open Wiki', exact: true }).click();
  const nav = page.getByRole('navigation', { name: 'Wiki pages' });
  await expect(nav).toBeVisible();
  await expect(
    page.getByRole('combobox', { name: 'Repository' }).locator('option')
  ).toHaveCount(1);
  await expect(page.getByRole('combobox', { name: 'Repository' })).toHaveValue(
    repo.id
  );
  await nav.getByRole('button', { name: 'index', exact: true }).click();
  const article = page.getByRole('region', { name: 'Wiki article' });
  await expect(article).toContainText('Wiki E2E Home');
  await expect(
    page.getByRole('button', { name: 'Files · main repository', exact: true })
  ).toBeVisible();
  await expect(
    page.getByText('Not Started', { exact: true }).first()
  ).toBeVisible();
  const separator = page.locator('#kanban-separator');
  const grip = await separator.boundingBox();
  const beforeResize = await article.boundingBox();
  assert.ok(grip && beforeResize);
  await page.mouse.move(grip.x + grip.width / 2, grip.y + grip.height / 2);
  await page.mouse.down();
  await page.mouse.move(grip.x - 100, grip.y + grip.height / 2, { steps: 8 });
  await page.mouse.up();
  await expect
    .poll(async () => (await article.boundingBox()).width)
    .toBeGreaterThan(beforeResize.width + 50);
  await expect(
    page.getByRole('button', { name: 'Local workspaces', exact: true })
  ).toBeVisible();
  await page.screenshot({ path: join(evidence, 'kanban-desktop.png') });
  check(
    'Kanban toggle, header metrics removed, Wiki article beside board, other utilities retained'
  );
  await page.locator('[data-workspace-file-path="hello.md"]').click();
  await expect(article).toHaveCount(0);
  await expect(
    page.getByText('Main file preview', { exact: true }).first()
  ).toBeVisible();
  await expect(
    page.getByText('Not Started', { exact: true }).first()
  ).toBeVisible();
  check('Main file reuses preview while keeping board');
  const sidebar = panel.locator('..');
  const beforeTerminal = terminalSockets;
  await sidebar.getByRole('button', { name: 'Terminal', exact: true }).click();
  await expect(sidebar.locator('.xterm')).toBeVisible();
  const output = await page.evaluate(
    async ({ project, repo }) => {
      const access = await (await fetch('/api/access/status')).json();
      return new Promise((resolve, reject) => {
        const socket = new WebSocket(
          `ws://${location.host}/api/terminal/ws?project_id=${project}&repo_id=${repo}&cols=80&rows=24&csrf=${encodeURIComponent(access.csrf)}`
        );
        let text = '';
        const timer = setTimeout(() => {
          socket.close();
          reject(new Error('PTY timeout: ' + text));
        }, 10000);
        socket.onopen = () =>
          socket.send(JSON.stringify({ type: 'input', data: btoa('pwd\n') }));
        socket.onmessage = (event) => {
          const message = JSON.parse(event.data);
          if (message.type === 'output') text += atob(message.data);
          if (text.includes('fixture-repo')) {
            clearTimeout(timer);
            socket.close();
            resolve(text);
          }
        };
        socket.onerror = () => {
          clearTimeout(timer);
          reject(new Error('PTY connection failed'));
        };
      });
    },
    { project: project.id, repo: repo.id }
  );
  assert.ok(output.includes(repoPath));
  await page.waitForTimeout(800);
  assert.equal(
    terminalSockets - beforeTerminal,
    2,
    'One UI PTY plus one direct PTY; no tab/connection loop'
  );
  check('Real existing PTY starts in registered main repository');
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole('button', { name: /Hide Right Sidebar/ }).click();
  await page.getByRole('button', { name: /Show Right Sidebar/ }).click();
  await page.getByRole('button', { name: 'Open Wiki', exact: true }).click();
  await nav.getByRole('button', { name: 'second', exact: true }).click();
  await expect(article).toContainText('Wiki E2E Second');
  await expect(
    page.getByText('Not Started', { exact: true }).first()
  ).toBeVisible();
  assert.ok(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth
    )
  );
  await page.screenshot({ path: join(evidence, 'kanban-mobile.png') });
  check(
    'Mobile main Wiki leaves board visible without horizontal page overflow'
  );

  await page.setViewportSize({ width: 1600, height: 1000 });
  await page.goto(`${base}/workspaces/${workspace.id}`);
  const editor = page.locator('[contenteditable="true"]').first();
  await expect(editor).toBeVisible({ timeout: 60000 });
  const draft = 'Utilities 未送信の会話下書き';
  await editor.fill(draft);
  const showSidebar = page.getByRole('button', { name: /Show Right Sidebar/ });
  if (await showSidebar.count()) await showSidebar.click();
  await page.getByRole('button', { name: /Toggle Wiki Reader/ }).click();
  await expect(article).toContainText('Wiki E2E Home');
  await expect(editor).toContainText(draft);
  await expect(
    page.getByRole('button', { name: 'Git', exact: true })
  ).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Terminal', exact: true })
  ).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Files', exact: true })
  ).toBeVisible();
  await page.screenshot({ path: join(evidence, 'workspace-desktop.png') });
  check(
    'Workspace common panel, Wiki and other utilities coexist; unsent draft retained'
  );
  await article.getByRole('link', { name: 'Second page' }).click();
  await expect(article).toContainText('Wiki E2E Second');
  await page.getByRole('button', { name: /Toggle Wiki Reader/ }).click();
  await expect(editor).toContainText(draft);
  await page.setViewportSize({ width: 1000, height: 800 });
  await page.getByRole('button', { name: /Toggle Wiki Reader/ }).click();
  await expect(article).toBeVisible();
  await expect(editor).toBeVisible();
  await page.screenshot({ path: join(evidence, 'workspace-narrow.png') });
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole('button', { name: 'Wiki', exact: true }).click();
  await page
    .getByRole('button', { name: 'index', exact: true })
    .filter({ visible: true })
    .click();
  await expect(article).toContainText('Wiki E2E Home');
  await page.getByRole('button', { name: 'Chat', exact: true }).click();
  await expect(editor).toContainText(draft);
  await page
    .getByRole('button', { name: 'Show Right Sidebar', exact: true })
    .click();
  await expect(panel.filter({ visible: true })).toHaveCount(1);
  await page
    .getByRole('button', { name: 'Hide Right Sidebar', exact: true })
    .click();
  await expect(editor).toBeVisible();
  await expect(editor).toContainText(draft);
  check('Workspace narrow/mobile Wiki and chat navigation preserves draft');
  const currentInfo = await api('/info');
  await api('/config', 'PUT', { ...currentInfo.config, language: 'JA' });
  await page.goto(`${base}/projects/${project.id}`);
  const showJapaneseSidebar = page.getByRole('button', {
    name: /右サイドバーを表示/,
  });
  if (await showJapaneseSidebar.count()) await showJapaneseSidebar.click();
  await expect(panel).toContainText('LVK実行環境');
  await expect(panel).toContainText('ホスト共有');
  await page.screenshot({ path: join(evidence, 'kanban-ja.png') });
  await page.route('**/api/resource-observation', (route) =>
    route.fulfill({ status: 503, json: { success: false } })
  );
  await page.reload();
  await expect(
    panel
      .locator('dl > div')
      .filter({ has: page.getByText('CPU', { exact: true }) })
      .locator('dd')
  ).toHaveText('不明');
  await page.unroute('**/api/resource-observation');
  await api('/config', 'PUT', { ...currentInfo.config, language: 'EN' });
  check(
    'Japanese panel and resource failure show unknown instead of cached current values'
  );
  await api(`/repos/${repo.id}/memory`, 'PUT', {
    enabled: false,
    target_branch: 'main',
    output_language: 'ja',
  });
  await page.setViewportSize({ width: 1600, height: 1000 });
  await page.goto(`${base}/workspaces/${workspace.id}`);
  await expect(panel).toBeVisible({ timeout: 60000 });
  await expect(
    page.getByRole('button', { name: /Toggle Wiki Reader/ })
  ).toHaveCount(0);
  await expect(
    page.getByRole('button', { name: 'Wiki', exact: true })
  ).toHaveCount(0);
  await page.goto(`${base}/projects/${project.id}`);
  await expect(panel).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Open Wiki', exact: true })
  ).toHaveCount(0);
  check('Disabled OpenWiki removes entries in both views');
  assert.deepEqual(errors, []);
  check('No browser page errors');
  await context.tracing.stop({ path: join(evidence, 'trace.zip') });
} catch (error) {
  results.error = String(error.stack || error);
  if (page) {
    await page
      .screenshot({ path: join(evidence, 'failure.png') })
      .catch(() => {});
    await writeFile(join(evidence, 'failure.html'), await page.content()).catch(
      () => {}
    );
  }
  throw error;
} finally {
  const childPids = descendants();
  await browser?.close();
  if (server.exitCode === null) {
    process.kill(-server.pid, 'SIGTERM');
    const timer = setTimeout(() => {
      try {
        process.kill(-server.pid, 'SIGKILL');
      } catch {}
    }, 10000);
    await exited;
    clearTimeout(timer);
  }
  // Also settle owned descendants that started their own process group.
  for (const pid of childPids.filter(alive)) {
    try {
      process.kill(pid, 'SIGTERM');
    } catch (error) {
      if (error.code !== 'ESRCH') throw error;
    }
  }
  await log.close();
  await expect.poll(() => alive(-server.pid), { timeout: 15000 }).toBe(false);
  await expect
    .poll(() => childPids.filter(alive), { timeout: 15000 })
    .toEqual([]);
  await assert.rejects(
    fetch(`${base}/api/health`),
    'Temporary port must be closed'
  );
  await rm(root, { recursive: true, force: true });
  results.cleanup = {
    serverProcessGroupGone: true,
    portClosed: true,
    browserClosed: true,
    descendantsGone: childPids,
    temporaryDirectoryRemoved: true,
  };
  await writeFile(
    join(evidence, 'result.json'),
    JSON.stringify(results, null, 2)
  );
  console.log(
    'Cleanup verified: server group gone, browser closed, port closed, temporary data removed'
  );
}
