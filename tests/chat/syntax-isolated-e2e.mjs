// Run after local-web and server builds. Only the agent stream is a fixture:
// workspace/session, configuration and HTTP application use an isolated real DB.
import { chromium, expect } from '@playwright/test';
import { spawn, execFileSync } from 'node:child_process';
import {
  mkdtempSync,
  mkdirSync,
  createWriteStream,
  readFileSync,
  writeFileSync,
  rmSync,
  existsSync,
} from 'node:fs';
import { resolve, join } from 'node:path';
import { randomUUID } from 'node:crypto';
import { createServer, connect } from 'node:net';
import { once } from 'node:events';

const root = process.cwd();
const evidence = resolve('test-results/local32');
mkdirSync(evidence, { recursive: true });
mkdirSync('.build', { recursive: true });
const instance = mkdtempSync(resolve('.build/local32-'));
const workspace = randomUUID(),
  session = randomUUID(),
  run = randomUUID();
const listener = createServer();
await new Promise((r) => listener.listen(0, '127.0.0.1', r));
const port = listener.address().port;
await new Promise((r) => listener.close(r));
const origin = `http://127.0.0.1:${port}`;
let server, browser, page;
const serverPids = [];
let browserPids = [];
const report = {
  origin,
  instance,
  workspace,
  session,
  checks: [],
  fixture:
    'AgentRun list and WebSocket events only; real temporary SQLite workspace/session and built app',
  errors: [],
};
async function stopServer() {
  if (!server || server.exitCode !== null || server.signalCode !== null) return;
  const done = once(server, 'exit');
  server.kill('SIGTERM');
  const timer = setTimeout(() => server.kill('SIGKILL'), 10000);
  await done;
  clearTimeout(timer);
}
async function startServer() {
  const log = createWriteStream(join(evidence, 'isolated-server.log'), {
    flags: 'a',
  });
  server = spawn(resolve('target/debug/server'), [], {
    cwd: root,
    env: {
      ...process.env,
      LVK_INSTANCE_DIR: instance,
      LVK_ACCESS_DEPLOYMENT: 'local',
      LVK_AUTH_MODE: '',
      LVK_PUBLIC_URL: '',
      HOST: '127.0.0.1',
      PORT: '',
      BACKEND_PORT: String(port),
      PREVIEW_PROXY_PORT: '0',
      VK_ALLOWED_ORIGINS: origin,
      VK_PREVIEW_DOMAIN: '',
      DISABLE_WORKTREE_CLEANUP: '1',
      BROWSER: 'true',
      RUST_LOG: 'info',
      VIBE_KANBAN_AGENT_PROCESS_HOST: resolve(
        'target/debug/agent-process-host'
      ),
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  serverPids.push(server.pid);
  server.stdout.pipe(log);
  server.stderr.pipe(log);
  for (let i = 0; i < 120; i++) {
    if (server.exitCode !== null || server.signalCode !== null)
      throw new Error(`server exited ${server.exitCode}`);
    try {
      if ((await fetch(`${origin}/api/health`)).ok) return;
    } catch {}
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new Error('server startup timeout');
}
try {
  await startServer();
  await stopServer();
  execFileSync('python3', [
    '-c',
    `
import sqlite3,sys,uuid, pathlib,json,datetime
root,ws,se=sys.argv[1:]
db=pathlib.Path(root)/'data/db.v2.sqlite'
assert db.exists(), str(db)
c=sqlite3.connect(db)
now=datetime.datetime.now(datetime.timezone.utc).isoformat()
c.execute("insert into workspaces (id,branch,name,container_ref,workspace_kind,container_ownership,created_at,updated_at) values (?,?,?,?,?,?,?,?)",(uuid.UUID(ws).bytes,'fixture','LOCAL32 syntax',root,'direct_folder','external',now,now))
c.execute("insert into sessions (id,workspace_id,executor,created_at,updated_at) values (?,?,?,?,?)",(uuid.UUID(se).bytes,uuid.UUID(ws).bytes,'CODEX',now,now))
c.commit();c.close()
p=pathlib.Path(root)/'data/config.json'
if p.exists():
 d=json.loads(p.read_text());d.update(disclaimer_acknowledged=True,onboarding_acknowledged=True,remote_onboarding_acknowledged=True,language='JA',theme='LIGHT');p.write_text(json.dumps(d))
`,
    instance,
    workspace,
    session,
  ]);
  await startServer();
  browser = await chromium.launch({
    args: ['--no-sandbox', '--disable-dev-shm-usage'],
  });
  const cdp = await browser.newBrowserCDPSession();
  browserPids = (await cdp.send('SystemInfo.getProcessInfo')).processInfo.map(
    (p) => p.id
  );
  await cdp.detach();
  const context = await browser.newContext({
    permissions: ['clipboard-read', 'clipboard-write'],
    viewport: { width: 1440, height: 1100 },
  });
  page = await context.newPage();
  page.on('pageerror', (e) => report.errors.push(e.message));
  const stamp = new Date().toISOString();
  const state = {
    state_schema_version: 1,
    reducer_version: 1,
    session_id: session,
    agent_run_id: run,
    turn_id: run,
    status: 'succeeded',
    projection_status: 'current',
    last_run_attempt_id: run,
    last_run_attempt_number: 1,
    last_event_sequence: 0,
    unknown_event_count: 0,
    updated_at: stamp,
  };
  await page.route(`**/api/agent-runs/session/${session}`, (route) =>
    route.fulfill({
      json: {
        success: true,
        data: [
          {
            agent_run_id: run,
            session_id: session,
            turn_id: run,
            state,
            created_at: stamp,
            updated_at: stamp,
          },
        ],
      },
    })
  );
  let stream,
    seq = 0;
  const envelope = (payload) => ({
    schema_version: 1,
    payload_version: 1,
    event_id: randomUUID(),
    session_id: session,
    agent_run_id: run,
    turn_id: run,
    run_attempt_id: run,
    run_attempt_number: 1,
    sequence: ++seq,
    correlation_id: run,
    timestamp: stamp,
    native_refs: [],
    payload,
  });
  const message = (content) => ({
    type: 'message',
    data: {
      message: { message_id: 'syntax-message', role: 'assistant', content },
      final_output: false,
    },
  });
  const command = '  echo "$HOME" && printf \'日本語 <tag>\\n\'  ';
  const tool = {
    type: 'tool_call',
    data: {
      tool_call_id: 'syntax-command',
      tool_name: 'Shell',
      status: 'succeeded',
      arguments: { command },
      result: { aggregatedOutput: 'plain output: const x = 42;', exitCode: 0 },
    },
  };
  await page.routeWebSocket(`**/api/agent-runs/${run}/events/ws`, (ws) => {
    stream = ws;
    for (const payload of [
      tool,
      message('LOCAL32 code\n\n```bash\necho "$HOME'),
    ])
      ws.send(
        JSON.stringify({
          type: 'event',
          data: { event: envelope(payload), replay: true },
        })
      );
    ws.send(JSON.stringify({ type: 'ready', data: { state, cursor: null } }));
  });
  await page.goto(`${origin}/workspaces/${workspace}?session_id=${session}`);
  await expect(page.locator('code[data-language="bash"]').first()).toBeVisible({
    timeout: 60000,
  });
  report.checks.push(
    'built app loaded real workspace/session and incomplete fixture Markdown'
  );
  await page.keyboard.press('Escape');
  const aggregate = page
    .getByRole('button', { name: /ツール呼び出し完了|件のコマンドを実行/ })
    .first();
  if (await aggregate.isVisible()) await aggregate.click();
  const commandButton = page
    .getByRole('button')
    .filter({ hasText: 'echo "$HOME" && printf' })
    .first();
  if (await commandButton.isVisible()) await commandButton.click();
  const commandPre = page
    .locator('pre')
    .filter({ hasText: 'echo "$HOME" && printf' })
    .first();
  expect(await commandPre.textContent()).toBe(command);
  expect(
    await commandPre.locator('[class*="syntax-"]').count()
  ).toBeGreaterThan(0);
  await commandPre.evaluate((el) => {
    const range = document.createRange();
    range.selectNodeContents(el);
    const s = window.getSelection();
    s.removeAllRanges();
    s.addRange(range);
  });
  await page.keyboard.press('Control+c');
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
    command
  );
  report.checks.push(
    'command expanded, highlighted, whitespace and clipboard exact'
  );
  const full =
    'LOCAL32 code\n\n```bash\necho "$HOME"\n```\n\n```json\n{"ok": true}\n```\n\n```unknown-language\nconst untouched = 42;\n```\n\n```text\nconst plain = 42;\n```\n\n```\nconst unspecified = 42;\n```';
  const composer = page.locator('[contenteditable="true"]').last();
  await composer.fill('入力を保持する');
  stream.send(
    JSON.stringify({
      type: 'event',
      data: { event: envelope(message(full)), replay: false },
    })
  );
  await expect(page.locator('code[data-language="json"]')).toContainText(
    '"ok"'
  );
  expect(await composer.textContent()).toBe('入力を保持する');
  expect(await composer.evaluate((el) => el === document.activeElement)).toBe(
    true
  );
  await page
    .getByRole('button', { name: 'Copy as Markdown', exact: true })
    .first()
    .click({ force: true });
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(full);
  report.checks.push(
    'stream update preserves composer and Markdown copy equals fixture source'
  );
  for (const language of ['unknown-language', 'text'])
    expect(
      await page
        .locator(`code[data-language="${language}"] [class*="syntax-"]`)
        .count()
    ).toBe(0);
  const colors = [];
  for (const theme of ['light', 'dark']) {
    await page.evaluate(
      (theme) =>
        document.documentElement.classList.toggle('dark', theme === 'dark'),
      theme
    );
    const token = page
      .locator('code[data-language="bash"] [class*="syntax-string"]')
      .first();
    const color = await token.evaluate((el) => getComputedStyle(el).color);
    colors.push(color);
    // Existing button descendants transition colors for 200ms on theme changes.
    await expect
      .poll(() =>
        commandButton
          .locator('[class*="syntax-string"]')
          .first()
          .evaluate((el) => getComputedStyle(el).color)
      )
      .toBe(color);
    await page.screenshot({
      path: join(evidence, `${theme}.png`),
      fullPage: true,
    });
  }
  expect(colors[0]).not.toBe(colors[1]);
  report.colors = colors;
  report.checks.push(
    'stream completion, JSON, unknown/plain/unspecified, light/dark computed colors and screenshots'
  );
  const long = 'echo "$HOME"\n'.repeat(1800).trimEnd();
  stream.send(
    JSON.stringify({
      type: 'event',
      data: {
        event: envelope(message(full + '\n\n```bash\n' + long + '\n```')),
        replay: false,
      },
    })
  );
  const longBlock = page.locator('code[data-language="bash"]').last();
  await expect(longBlock).toContainText('echo');
  await expect.poll(() => longBlock.locator('br').count()).toBe(1799);
  expect(await longBlock.locator('[class*="syntax-"]').count()).toBe(0);
  report.checks.push(
    'long streamed block remains complete with plain fallback'
  );
  expect(report.errors).toEqual([]);
  report.passed = true;
} catch (error) {
  report.failure = String(error.stack || error);
  if (page) {
    await page
      .screenshot({ path: join(evidence, 'failure.png'), fullPage: true })
      .catch(() => {});
    writeFileSync(
      join(evidence, 'failure.html'),
      await page.content().catch(() => '')
    );
  }
  process.exitCode = 1;
} finally {
  if (browser) {
    try {
      const cdp = await browser.newBrowserCDPSession();
      browserPids = [
        ...new Set([
          ...browserPids,
          ...(await cdp.send('SystemInfo.getProcessInfo')).processInfo.map(
            (p) => p.id
          ),
        ]),
      ];
      await cdp.detach();
    } catch {}
    await browser.close();
  }
  await stopServer();
  report.serverExited =
    !!server && (server.exitCode !== null || server.signalCode !== null);
  report.browserClosed = !browser?.isConnected();
  const alive = (pid) => {
    try {
      process.kill(pid, 0);
      return true;
    } catch {
      return false;
    }
  };
  report.remainingTestPids = [...serverPids, ...browserPids].filter(alive);
  const logPath = join(evidence, 'isolated-server.log');
  const proxyPorts = existsSync(logPath)
    ? [
        ...readFileSync(logPath, 'utf8').matchAll(/Preview proxy on :(\d+)/g),
      ].map((m) => Number(m[1]))
    : [];
  report.proxyPortsClosed = [];
  for (const proxy of proxyPorts) {
    const closed = await new Promise((resolve) => {
      const socket = connect(proxy, '127.0.0.1');
      socket.once('error', () => resolve(true));
      socket.once('connect', () => {
        socket.destroy();
        resolve(false);
      });
    });
    report.proxyPortsClosed.push({ port: proxy, closed });
  }
  report.portClosed = await new Promise((resolve) => {
    const s = connect(port, '127.0.0.1');
    s.once('error', () => resolve(true));
    s.once('connect', () => {
      s.destroy();
      resolve(false);
    });
  });
  rmSync(instance, { recursive: true, force: true });
  report.temporaryDbRemoved = !existsSync(instance);
  report.cleanupPassed =
    report.serverExited &&
    report.browserClosed &&
    report.portClosed &&
    report.temporaryDbRemoved &&
    report.remainingTestPids.length === 0 &&
    report.proxyPortsClosed.every((p) => p.closed);
  if (!report.cleanupPassed) process.exitCode = 1;
  writeFileSync(
    join(evidence, 'isolated-e2e.json'),
    JSON.stringify(report, null, 2)
  );
  console.log(JSON.stringify(report, null, 2));
}
