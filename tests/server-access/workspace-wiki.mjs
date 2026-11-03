// Built application + real backend; disposable repository content is the only fixture.
// Run after building local-web, server and agent-process-host:
// node tests/server-access/workspace-wiki.mjs /absolute/evidence/directory
import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { once } from 'node:events';
import {
  mkdir,
  mkdtemp,
  rm,
  readFile,
  writeFile,
  open,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { createServer } from 'node:net';
import { request } from 'node:http';
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
const unrelatedJobs = [];
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
  await context.addInitScript(() => {
    const Original = window.WebSocket;
    window.__terminalSockets = [];
    window.WebSocket = class extends Original {
      constructor(...args) {
        super(...args);
        if (String(args[0]).includes('/api/terminal/ws'))
          window.__terminalSockets.push(this);
      }
    };
  });

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
  await expect(panel).toBeHidden();
  await page.getByRole('button', { name: /Show Right Sidebar/ }).click();
  await expect(panel).toBeVisible();
  await page.getByRole('button', { name: 'Open Wiki', exact: true }).click();
  const nav = page.getByRole('navigation', { name: 'Wiki pages' });
  await expect(nav).toBeVisible();
  await expect(
    page
      .getByRole('combobox', { name: 'Wiki repository', exact: true })
      .locator('option')
  ).toHaveCount(1);
  await expect(
    page.getByRole('combobox', { name: 'Wiki repository', exact: true })
  ).toHaveValue(repo.id);
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
  async function terminalLifecycle(view) {
    const sockets = () => page.evaluate(() => window.__terminalSockets.length);
    const send = (data) =>
      page.evaluate((data) => {
        const socket = window.__terminalSockets.at(-1);
        if (!socket || socket.readyState !== WebSocket.OPEN)
          throw new Error('Owned terminal is not open');
        socket.send(JSON.stringify({ type: 'input', data: btoa(data) }));
      }, data);
    const header = () =>
      page
        .getByRole('button', { name: 'Terminal', exact: true })
        .filter({ visible: true });
    const open = async () => {
      const before = await sockets();
      await expect(header()).toHaveAttribute('aria-expanded', 'false');
      await header().click();
      await expect
        .poll(() =>
          page.evaluate(() => window.__terminalSockets.at(-1)?.readyState)
        )
        .toBe(1);
      assert.equal(await sockets(), before + 1);
      return before + 1;
    };
    const isRunning = async (pid) => {
      try {
        const stat = await readFile(`/proc/${pid}/stat`, 'utf8');
        return !['Z', 'X'].includes(
          stat.slice(stat.lastIndexOf(')') + 2).split(' ')[0]
        );
      } catch (error) {
        if (error.code === 'ENOENT') return false;
        throw error;
      }
    };
    const readPids = async (path) => {
      await expect
        .poll(
          async () =>
            (await readFile(path, 'utf8').catch(() => '')).trim().split(/\s+/)
              .length
        )
        .toBe(3);
      return (await readFile(path, 'utf8')).trim().split(/\s+/).map(Number);
    };
    const assertStopped = async (ids) => {
      await expect
        .poll(async () => Promise.all(ids.map(isRunning)), { timeout: 15000 })
        .toEqual(ids.map(() => false));
      await expect
        .poll(
          () =>
            readFile(`/proc/${ids[0]}/stat`, 'utf8')
              .then(() => true)
              .catch(() => false),
          { timeout: 15000, message: 'Shell must be reaped' }
        )
        .toBe(false);
    };
    const foreignContext = await browser.newContext();
    const foreignPage = await foreignContext.newPage();
    await foreignPage.goto(`${base}/api/health`);
    const foreignRecord = join(root, `${view}-foreign`);
    const endpoint =
      view === 'main'
        ? `project_id=${project.id}&repo_id=${repo.id}`
        : `workspace_id=${workspace.id}`;
    await foreignPage.evaluate(
      ({ endpoint, record }) =>
        new Promise((resolve, reject) => {
          const socket = new WebSocket(
            `ws://${location.host}/api/terminal/ws?${endpoint}`
          );
          window.foreignTerminal = socket;
          socket.onopen = () => {
            socket.send(
              JSON.stringify({
                type: 'input',
                data: btoa(`echo $$ > '${record}'\n`),
              })
            );
            resolve();
          };
          socket.onerror = reject;
        }),
      { endpoint, record: foreignRecord }
    );
    await expect
      .poll(() => readFile(foreignRecord, 'utf8').catch(() => ''))
      .toMatch(/\d+/);
    const foreignPid = Number((await readFile(foreignRecord, 'utf8')).trim());
    const unrelated = spawn('sleep', ['300'], {
      cwd:
        view === 'main'
          ? repoPath
          : (await api(`/workspaces/${workspace.id}`)).container_ref,
      stdio: 'ignore',
    });
    unrelatedJobs.push(unrelated);
    const archiveStatus = () =>
      fetch(`${base}/api/workspaces/${workspace.id}/archive/prepare`, {
        method: 'POST',
        headers: { Origin: base, 'x-lvk-csrf': csrf },
      }).then((r) => r.status);
    try {
      if (view === 'main') {
        const selector = page.getByTestId('main-terminal-repository');
        const before = await sockets();
        await expect(selector).toHaveCount(0);
        assert.equal(
          await sockets(),
          before,
          'Rendering the default repository must not start a shell'
        );
      }
      let opened = await open();
      if (view === 'main')
        await expect(
          page.getByTestId('main-terminal-repository')
        ).toHaveCount(0);
      // Empty shell termination is independent from a running foreground job.
      const emptyFile = join(root, `${view}-empty`);
      await send(`echo $$ > '${emptyFile}'\n`);
      await expect
        .poll(() => readFile(emptyFile, 'utf8').catch(() => ''))
        .toMatch(/\d+/);
      const emptyPid = Number((await readFile(emptyFile, 'utf8')).trim());
      await header().click();
      await assertStopped([emptyPid]);
      assert.equal(await sockets(), opened);
      opened = await open();
      const record = join(root, `${view}-jobs`);
      await send(
        `echo $$ > '${record}'; python3 -c "import os,subprocess,time; p=subprocess.Popen(['sleep','300']); f=open('${record}','a'); f.write(str(os.getpid())+'\\n'+str(p.pid)+'\\n'); f.close(); time.sleep(300)"\n`
      );
      const ids = await readPids(record);
      if (view === 'workspace') assert.equal(await archiveStatus(), 409);
      assert.deepEqual(await Promise.all(ids.map(isRunning)), [
        true,
        true,
        true,
      ]);
      await page
        .getByRole('button', { name: 'Hide Right Sidebar', exact: true })
        .click();
      assert.deepEqual(await Promise.all(ids.map(isRunning)), [
        true,
        true,
        true,
      ]);
      await page
        .getByRole('button', { name: 'Show Right Sidebar', exact: true })
        .click();
      assert.equal(
        await sockets(),
        opened,
        'Sidebar reopening must reuse the existing shell'
      );
      await page.setViewportSize({ width: 390, height: 844 });
      if (view === 'main') {
        await page.locator('[data-workspace-file-path="hello.md"]').click();
        assert.deepEqual(await Promise.all(ids.map(isRunning)), [
          true,
          true,
          true,
        ]);
        await page
          .getByRole('button', { name: 'Open Wiki', exact: true })
          .click();
        await page
          .getByRole('navigation', { name: 'Wiki pages' })
          .getByRole('button', { name: 'index', exact: true })
          .click();
        assert.deepEqual(await Promise.all(ids.map(isRunning)), [
          true,
          true,
          true,
        ]);
        await page
          .getByRole('button', { name: 'Show Right Sidebar', exact: true })
          .click();
      } else {
        await page.getByRole('button', { name: 'Files', exact: true }).click();
        await page
          .locator('[data-workspace-file-path="openwiki"]')
          .filter({ visible: true })
          .click();
        await page
          .locator('[data-workspace-file-path="openwiki/index.md"]')
          .filter({ visible: true })
          .click();
        assert.deepEqual(await Promise.all(ids.map(isRunning)), [
          true,
          true,
          true,
        ]);
        await page.getByLabel('Wiki', { exact: true }).click();
        await page
          .getByRole('button', { name: 'index', exact: true })
          .filter({ visible: true })
          .click();
        assert.deepEqual(await Promise.all(ids.map(isRunning)), [
          true,
          true,
          true,
        ]);
        await page
          .getByRole('button', { name: 'Show Right Sidebar', exact: true })
          .click();
      }
      assert.equal(
        await sockets(),
        opened,
        'Temporary file/Wiki layout must not create another shell'
      );
      await header().click();
      await assertStopped(ids);
      await page.setViewportSize({ width: 1600, height: 1000 });
      // Shell exit must close the item, even while its surface is unmounted.
      opened = await open();
      await page
        .getByRole('button', { name: 'Hide Right Sidebar', exact: true })
        .click();
      await send('exit\n');
      await expect
        .poll(() =>
          page.evaluate(() => window.__terminalSockets.at(-1).readyState)
        )
        .toBe(3);
      await page
        .getByRole('button', { name: 'Show Right Sidebar', exact: true })
        .click();
      await expect(header()).toHaveAttribute('aria-expanded', 'false');
      assert.equal(await sockets(), opened);
      opened = await open();
      const disconnected = join(root, `${view}-disconnected`);
      await send(`echo $$ > '${disconnected}'\n`);
      await expect
        .poll(() => readFile(disconnected, 'utf8').catch(() => ''))
        .toMatch(/\d+/);
      const disconnectedPid = Number(
        (await readFile(disconnected, 'utf8')).trim()
      );
      await page.evaluate(() => window.__terminalSockets.at(-1).close());
      await expect(header()).toHaveAttribute('aria-expanded', 'false');
      await assertStopped([disconnectedPid]);
      await page.waitForTimeout(1100); // Exceeds the old first auto-reconnect delay.
      assert.equal(
        await sockets(),
        opened,
        'Disconnect must not reconnect or create a shell'
      );
      // Lost application frames while the transport still exists: advance only
      // Date.now so the real watchdog runs without waiting 45 seconds per view.
      opened = await open();
      const silentRecord = join(root, `${view}-silent`);
      await send(`echo $$ > '${silentRecord}'\n`);
      await expect
        .poll(() => readFile(silentRecord, 'utf8').catch(() => ''))
        .toMatch(/\d+/);
      const silentPid = Number((await readFile(silentRecord, 'utf8')).trim());
      await page.evaluate(() => {
        window.__terminalSockets.at(-1).onmessage = null;
        window.__terminalRealNow = Date.now;
        Date.now = () => window.__terminalRealNow() + 60000;
      });
      try {
        await expect(header()).toHaveAttribute('aria-expanded', 'false', {
          timeout: 10000,
        });
      } finally {
        await page.evaluate(() => {
          Date.now = window.__terminalRealNow;
        });
      }
      await assertStopped([silentPid]);
      assert.equal(await sockets(), opened, 'Watchdog must not reconnect');
      await open();
      await header().click();
      // A raw client deliberately never answers native Ping. This exercises the
      // real server timeout, independently of browser timers and close frames.
      if (view === 'main') {
        const record = join(root, 'no-pong');
        const socket = await new Promise((resolve, reject) => {
          const req = request(`${base}/api/terminal/ws?${endpoint}`, {
            headers: {
              Origin: base,
              Connection: 'Upgrade',
              Upgrade: 'websocket',
              'Sec-WebSocket-Version': '13',
              'Sec-WebSocket-Key': 'MDEyMzQ1Njc4OWFiY2RlZg==',
            },
          });
          req.on('upgrade', (_response, socket) => resolve(socket));
          req.on('response', (response) => {
            response.resume();
            reject(new Error(`Upgrade rejected: ${response.statusCode}`));
          });
          req.on('error', reject);
          req.end();
        });
        socket.on('error', () => {});
        socket.on('data', () => {}); // Consume frames but send no Pong.
        try {
          const data = Buffer.from(
            JSON.stringify({
              type: 'input',
              data: Buffer.from(`echo $$ > '${record}'\n`).toString('base64'),
            })
          );
          const header = Buffer.alloc(8);
          header[0] = 0x81;
          header[1] = 0xfe;
          header.writeUInt16BE(data.length, 2);
          header.fill(0x5a, 4);
          socket.write(
            Buffer.concat([
              header,
              Buffer.from(data.map((byte) => byte ^ 0x5a)),
            ])
          );
          await expect
            .poll(() => readFile(record, 'utf8').catch(() => ''))
            .toMatch(/\d+/);
          const pid = Number((await readFile(record, 'utf8')).trim());
          const started = Date.now();
          await expect
            .poll(
              () =>
                readFile(`/proc/${pid}/stat`, 'utf8')
                  .then(() => true)
                  .catch(() => false),
              { timeout: 65000 }
            )
            .toBe(false);
          const elapsed = Date.now() - started;
          assert.ok(
            elapsed >= 40000 && elapsed < 65000,
            `Unexpected timeout: ${elapsed}ms`
          );
          check(
            `Server no-Pong timeout kills and reaps owned shell after ${elapsed}ms without a client close`
          );
        } finally {
          socket.destroy();
        }
      }
      check(
        `${view}: explicit collapse kills/reaps empty shell and foreground/child; hidden/sidebar/file/Wiki preserve it; exit/disconnect/watchdog fold without respawn; explicit reopen starts one`
      );
      assert.equal(
        await isRunning(foreignPid),
        true,
        'Other browser terminal must survive'
      );
      assert.equal(
        unrelated.exitCode,
        null,
        'Unrelated execution with the same cwd must survive'
      );
      if (view === 'workspace')
        assert.equal(
          await archiveStatus(),
          409,
          'Other owners must still block archive'
        );
    } finally {
      await foreignContext.close();
      unrelated.kill('SIGTERM');
      if (unrelated.exitCode === null && unrelated.signalCode === null)
        await once(unrelated, 'exit');
    }
    await expect
      .poll(() => isRunning(foreignPid), { timeout: 15000 })
      .toBe(false);
    if (view === 'workspace') await expect.poll(archiveStatus).toBe(200);
  }
  await terminalLifecycle('main');
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
  const ensureWorkspaceSidebar = page.getByRole('button', {
    name: 'Show Right Sidebar',
    exact: true,
  });
  if (await ensureWorkspaceSidebar.count())
    await ensureWorkspaceSidebar.click();
  await terminalLifecycle('workspace');
  if (await article.isVisible())
    await page.getByRole('button', { name: /Toggle Wiki Reader/ }).click();
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
  await page.getByLabel('Wiki', { exact: true }).click();
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
  await expect(page.getByLabel('Wiki', { exact: true })).toHaveCount(0);
  await page.goto(`${base}/projects/${project.id}`);
  await expect(panel).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Open Wiki', exact: true })
  ).toHaveCount(0);
  check('Disabled OpenWiki removes entries in both views');
  const prepared = await api(
    `/workspaces/${workspace.id}/archive/prepare`,
    'POST'
  );
  const archived = await api(
    `/workspaces/${workspace.id}/archive/confirm`,
    'POST',
    { confirmation_token: prepared.confirmation_token }
  );
  assert.equal(archived.workspace.archived, true);
  check(
    'Archive guard rejects live owners and normal prepare/confirm succeeds after terminal cleanup'
  );
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
  for (const job of unrelatedJobs) {
    if (job.exitCode === null && job.signalCode === null) {
      job.kill('SIGTERM');
      await once(job, 'exit');
    }
  }
  await browser?.close();
  // Leave the isolated server alive long enough to settle socket-owner cleanup.
  await new Promise((resolve) => setTimeout(resolve, 500));
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
