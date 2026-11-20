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
import { verifyScrollFollow } from './scroll-follow-app.mjs';

const root = process.cwd();
const additional = process.env.LVK_CHAT_ADDITIONAL === '1';
const inputOnly = process.env.LVK_CHAT_INPUT_ONLY === '1';
const frontendDir = resolve(
  process.env.LVK_CHAT_FRONTEND_DIR ?? 'packages/local-web/dist'
);
const evidence = resolve(
  process.env.LVK_CHAT_EVIDENCE ?? 'test-results/local32'
);
mkdirSync(evidence, { recursive: true });
mkdirSync('.build', { recursive: true });
const instance = mkdtempSync(resolve('.build/local32-'));
// Prevent the isolated application from launching host provider CLIs, including
// automatic startup discovery before the browser can intercept requests.
const isolatedBin = join(instance, 'bin');
mkdirSync(isolatedBin);
for (const name of ['codex', 'claude']) {
  writeFileSync(join(isolatedBin, name), '#!/bin/sh\nexit 1\n', {
    mode: 0o755,
  });
}
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
  startedAt: new Date().toISOString(),
  head: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(),
  dirty:
    execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).trim()
      .length > 0,
  artifacts: execFileSync(
    'sha256sum',
    [
      process.env.LVK_CHAT_SERVER_BIN || 'target/debug/server',
      process.env.LVK_CHAT_AGENT_HOST_BIN || 'target/debug/agent-process-host',
      join(frontendDir, 'index.html'),
    ],
    { encoding: 'utf8' }
  )
    .trim()
    .split('\n'),
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
  server = spawn(
    resolve(process.env.LVK_CHAT_SERVER_BIN || 'target/debug/server'),
    [],
    {
      cwd: root,
      env: {
        ...process.env,
        PATH: `${isolatedBin}:/usr/bin:/bin`,
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
          process.env.LVK_CHAT_AGENT_HOST_BIN ||
            'target/debug/agent-process-host'
        ),
      },
      stdio: ['ignore', 'pipe', 'pipe'],
    }
  );
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
  report.serverBuild = (
    await (await fetch(`${origin}/api/info`)).json()
  ).data?.build_info;
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
 d=json.loads(p.read_text());d.update(disclaimer_acknowledged=True,onboarding_acknowledged=True,remote_onboarding_acknowledged=True,language='JA',theme='SYSTEM');p.write_text(json.dumps(d))
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
  await page.emulateMedia({ colorScheme: 'light' });
  await page.route('**/api/provider-usage/read', (route) =>
    route.fulfill({ json: { success: true, data: null } })
  );
  await page.route(`**/api/workspaces/${workspace}/git/status`, (route) =>
    route.fulfill({ json: { success: true, data: [] } })
  );
  await page.routeWebSocket(`**/api/workspaces/${workspace}/git/diff/ws*`, (ws) => {
    const fixture = {
      type: 'DIFF',
      content: {
        change: 'modified', oldPath: 'src/fixture.ts', newPath: 'src/fixture.ts',
        oldContent: Array.from({ length: 80 }, (_, i) => `const answer${i} = 1;`).join('\n') + '\n',
        newContent: Array.from({ length: 80 }, (_, i) => `const answer${i} = 42;`).join('\n') + '\n',
        contentOmitted: false, additions: 80, deletions: 80, repoId: null,
      },
    };
    ws.send(JSON.stringify({ JsonPatch: [{ op: 'add', path: '/entries/fixture', value: { 'src/fixture.ts': fixture } }] }));
    ws.send(JSON.stringify({ Ready: true }));
  });
  await page.routeWebSocket('**/api/agents/discovered-options/ws*', (ws) =>
    ws.send(JSON.stringify({ Ready: true }))
  );
  if (process.env.LVK_CHAT_FRONTEND_DIR) {
    // Serve an isolated candidate to this browser only. The running public
    // server's asset directory and the temporary API server stay untouched.
    report.frontendOverride = frontendDir;
    await page.route(`${origin}/**`, async (route) => {
      const path = new URL(route.request().url()).pathname;
      if (path.startsWith('/api/')) return route.fallback();
      const file = resolve(frontendDir, '.' + path);
      const asset =
        file.startsWith(frontendDir + '/') && existsSync(file) && path !== '/'
          ? file
          : join(frontendDir, 'index.html');
      const extension = asset.split('.').at(-1);
      const contentType = {
        js: 'text/javascript',
        css: 'text/css',
        html: 'text/html',
        svg: 'image/svg+xml',
        woff2: 'font/woff2',
      }[extension];
      await route.fulfill({ body: readFileSync(asset), contentType });
    });
  }
  page.on('pageerror', (e) => {
    report.errors.push(e.message);
    (report.errorStacks ??= []).push(e.stack);
  });
  report.consoleErrors = [];
  page.on('console', (message) => {
    if (message.type() === 'error') report.consoleErrors.push(message.text());
  });
  const stamp = new Date().toISOString();
  const state = {
    state_schema_version: 1,
    reducer_version: 1,
    session_id: session,
    agent_run_id: run,
    turn_id: run,
    status: 'running',
    projection_status: 'current',
    last_run_attempt_id: run,
    last_run_attempt_number: 1,
    last_event_sequence: 0,
    unknown_event_count: 0,
    updated_at: stamp,
  };
  const extraRuns = [];
  await page.route(`**/api/agent-runs/session/${session}`, (route) =>
    route.fulfill({
      json: {
        success: true,
        data: [
          ...extraRuns,
          {
            agent_run_id: run,
            session_id: session,
            turn_id: run,
            state,
            created_at: new Date(Date.parse(stamp) - 13000).toISOString(),
            completed_at: null,
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
  let initialReplay;
  report.socketEvents = [];
  await page.routeWebSocket(
    `**/api/agent-runs/${run}/events/ws`,
    async (ws) => {
      let closed = false;
      report.socketEvents.push('opened');
      ws.onClose(() => {
        closed = true;
        report.socketEvents.push('closed');
      });
      // Do not replay to an initial React subscription already disposed while opening.
      await new Promise((resolve) => setTimeout(resolve, 50));
      if (closed) return;
      stream = ws;
      const frames = (initialReplay ??= [
        ...(process.env.LVK_CHAT_SCROLL_ROOT
          ? Array.from({ length: 100 }, (_, i) => ({
              type: 'message',
              data: {
                message: {
                  message_id: `history-${i}`,
                  role: i % 2 ? 'assistant' : 'user',
                  content: `Synthetic history ${i}\nLine A\nLine B\nLine C`,
                },
                final_output: false,
              },
            }))
          : []),
        tool,
        ...(process.env.LVK_CHAT_FRONTEND_DIR
          ? [
              {
                type: 'provider_extension',
                data: {
                  provider_namespace: 'codex',
                  provider_event: 'reasoning_summary',
                  payload: {
                    provider_item_id: 'summary-fixture',
                    content:
                      'Inspect README.md\nKeep the original provider text.',
                  },
                },
              },
              {
                type: 'provider_extension',
                data: {
                  provider_namespace: 'codex',
                  provider_event: 'plan',
                  payload: {
                    provider_item_id: 'plan-fixture',
                    content: 'Run focused tests\nCheck the result.',
                  },
                },
              },
            ]
          : []),
        ...(additional
          ? [
              {
                type: 'provider_extension',
                data: {
                  provider_namespace: 'codex',
                  provider_event: 'plan_update',
                  payload: {
                    provider_turn_id: run,
                    explanation: '受信した作業計画',
                    steps: [
                      { step: '構造を確認', status: 'completed' },
                      { step: '回帰試験を確認', status: 'inProgress' },
                      { step: '証跡を用意', status: 'pending' },
                    ],
                  },
                },
              },
              {
                type: 'message',
                data: {
                  message: {
                    message_id: 'normal-commentary',
                    role: 'assistant',
                    content: '通常の進捗説明はチャット本文です。',
                  },
                  final_output: false,
                },
              },
              {
                type: 'provider_extension',
                data: {
                  provider_namespace: 'claude_code',
                  provider_event: 'thinking',
                  payload: {
                    provider_item_id: 'claude-thinking',
                    content:
                      'Checking the received Claude thought\nFull original thought stays in disclosure.',
                  },
                },
              },
              {
                type: 'tool_call',
                data: {
                  tool_call_id: 'todo',
                  tool_name: 'TodoWrite',
                  status: 'succeeded',
                  arguments: {
                    todos: [
                      {
                        content: 'Claudeの計画項目',
                        status: 'in_progress',
                        priority: 'high',
                      },
                    ],
                  },
                  result: 'Todos updated',
                },
              },
            ]
          : []),
        {
          type: 'message',
          data: {
            message: {
              message_id: 'claude-style-markdown',
              role: 'assistant',
              content: 'Claude-style fixture\n\n### 調査結果\n\n**要点**と `inline` を確認。',
            },
            final_output: false,
          },
        },
        message('LOCAL32 code\n\n```bash\necho "$HOME'),
      ].map((payload) =>
        JSON.stringify({
          type: 'event',
          data: { event: envelope(payload), replay: true },
        })
      ));
      for (const frame of frames) {
        if (closed) return;
        ws.send(frame);
        await new Promise((resolve) => setTimeout(resolve, 2));
      }
      if (closed) return;
      ws.send(JSON.stringify({ type: 'ready', data: { state, cursor: null } }));
    }
  );
  await page.goto(`${origin}/workspaces/${workspace}?session_id=${session}`);
  await expect(page.locator('code[data-language="bash"]').first()).toBeVisible({
    timeout: 60000,
  });
  if (process.env.LVK_CAPTURE_RESOURCES === '1') {
    await page.waitForTimeout(1500);
    report.initialJavascriptResources = await page.evaluate(() =>
      performance.getEntriesByType('resource')
        .filter((entry) => new URL(entry.name).pathname.endsWith('.js'))
        .map((entry) => ({ name: new URL(entry.name).pathname,
          decodedBytes: entry.decodedBodySize, transferBytes: entry.transferSize }))
    );
    const lines = Array.from({ length: 40 }, (_, i) => `echo "$HOME" && printf 'line ${i} %s\\n' value`);
    await page.evaluate(() => {
      const frames = [];
      const longTasks = [];
      const observer = new PerformanceObserver((entries) => {
        for (const entry of entries.getEntries()) longTasks.push(entry.duration);
      });
      observer.observe({ type: 'longtask', buffered: false });
      let active = true;
      const sample = (time) => {
        if (!active) return;
        frames.push(time);
        requestAnimationFrame(sample);
      };
      requestAnimationFrame(sample);
      window.local432StopComparison = () => {
        active = false;
        observer.disconnect();
        return { frames, longTasks };
      };
    });
    for (let i = 1; i <= 20; i++) {
      stream.send(JSON.stringify({ type: 'event', data: {
        event: envelope({ type: 'message', data: { message: {
          message_id: 'comparison-stream', role: 'assistant',
          content: (i === 1 ? 'Comparison sample\n\n```bash\n' : '') +
            lines.slice((i - 1) * 2, i * 2).join('\n') + (i === 20 ? '\n```' : '\n'),
        }, final_output: false } }), replay: false,
      } }));
      await new Promise((resolve) => setTimeout(resolve, 40));
    }
    await expect.poll(() => page.locator('code[data-language="bash"]').last().innerText()).toBe(lines.join('\n'));
    await page.waitForTimeout(250);
    const timing = await page.evaluate(() => window.local432StopComparison());
    report.comparisonStreaming = {
      bytes: Buffer.byteLength(lines.join('\n')),
      frames: timing.frames.length,
      maxFrameIntervalMs: Math.max(0, ...timing.frames.slice(1).map((time, i) => time - timing.frames[i])),
      longTasksMs: timing.longTasks,
    };
  }
  report.checks.push(
    'built app loaded real workspace/session and incomplete fixture Markdown'
  );
  await expect(page.getByText('Claude-style fixture', { exact: true })).toBeVisible();
  await expect(page.getByRole('heading', { name: '調査結果' })).toBeVisible();
  await expect(page.getByText('要点', { exact: true })).toBeVisible();
  report.checks.push('Codex-style and Claude-style assistant Markdown share the real renderer');
  await page.keyboard.press('Escape');
  if (process.env.LVK_CHAT_SCROLL_ROOT) {
    await verifyScrollFollow({ page, session, extraRuns, evidence, report });
  } else {
    if (process.env.LVK_CHAT_FRONTEND_DIR) {
      await expect(
        page.getByText('plain output: const x = 42;', { exact: true })
      ).toBeVisible();
      await expect(
        page.getByRole('button', { name: /ツール呼び出し完了/ })
      ).toHaveCount(0);
      await expect(
        page.getByTestId('run-elapsed').locator('time')
      ).toHaveAttribute('datetime', stamp);
      await expect(
        page.locator('summary').getByText('Inspect README.md', { exact: true })
      ).toBeVisible();
      await expect(
        page
          .getByRole('region', { name: '計画' })
          .filter({ hasText: 'Run focused tests' })
      ).toBeVisible();
      if (additional) {
        await expect(
          page.getByText('構造を確認', { exact: true })
        ).toBeVisible();
        await expect(
          page.getByText('回帰試験を確認', { exact: true })
        ).toBeVisible();
        await expect(
          page.getByText('Claudeの計画項目', { exact: true })
        ).toBeVisible();
        await expect(
          page.getByText('通常の進捗説明はチャット本文です。', { exact: true })
        ).toBeVisible();
        await expect(
          page
            .locator('summary')
            .getByText('Checking the received Claude thought', { exact: true })
        ).toBeVisible();
      }
      report.checks.push(
        'operation/result visible initially; retained run completion timestamp visible without expansion'
      );
      await page.screenshot({
        path: join(evidence, 'initial.png'),
        fullPage: true,
      });
    }
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
    await expect
      .poll(() => commandPre.locator('.shiki-token').count())
      .toBeGreaterThan(0);
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
      'LOCAL32 code\n\n```bash\necho "$HOME"\nprintf "a\\_b"\n```\n\n```json\n{"ok": true}\n```\n\n```tsx\nconst View = () => <div>Hello</div>;\nconst payload = "<img src=x onerror=window.local432Injected=1>";\n```\n\n```rust\nfn main() { println!("hello"); }\n```\n\n```python\ndef greet(name):\n    return f"hello {name}"\n```\n\n```unknown-language\nconst untouched = 42;\n```\n\n```text\nconst plain = 42;\n```\n\n```\nconst unspecified = 42;\n```';
    const composer = page.locator('[contenteditable="true"]').last();
    if (additional || inputOnly) {
      const plain = '**literal**\n```js\nconst a_b = "a\\_b";\n```\n';
      await composer.focus();
      await page.evaluate(
        async (text) =>
          navigator.clipboard.write([
            new ClipboardItem({
              'text/plain': new Blob([text], { type: 'text/plain' }),
              'text/html': new Blob(
                ['<pre><code><strong>formatted</strong></code></pre>'],
                { type: 'text/html' }
              ),
            }),
          ]),
        plain
      );
      await composer.press('Control+v');
      await expect.poll(() => composer.innerText()).toBe(plain + '\n');
      await expect(composer.locator('pre,code,strong,b')).toHaveCount(0);
      await page.screenshot({
        path: join(evidence, 'input-rich-paste.png'),
        fullPage: true,
      });
      report.checks.push(
        'real composer accepts rich HTML as literal clipboard text without formatting'
      );
      await composer.evaluate(async (el) => {
        const canvas = document.createElement('canvas');
        canvas.width = 2;
        canvas.height = 2;
        const blob = await new Promise((resolve) =>
          canvas.toBlob(resolve, 'image/png')
        );
        const data = new DataTransfer();
        data.items.add(
          new File([blob], 'synthetic-attachment.png', { type: 'image/png' })
        );
        el.dispatchEvent(
          new ClipboardEvent('paste', {
            clipboardData: data,
            bubbles: true,
            cancelable: true,
          })
        );
      });
      const removeAttachment = composer.getByRole('button', {
        name: '画像を削除',
        exact: true,
      });
      await expect(removeAttachment).toBeAttached({ timeout: 15000 });
      await page.screenshot({
        path: join(evidence, 'input-attachment.png'),
        fullPage: true,
      });
      await removeAttachment.click({ force: true });
      await expect(removeAttachment).toHaveCount(0);
      report.checks.push(
        'real composer file paste uploads only synthetic PNG to isolated workspace; attachment preview and remove remain available'
      );
    }
    await composer.fill('入力を保持する');
    report.socketEvents.push('sending update');
    stream.send(
      JSON.stringify({
        type: 'event',
        data: { event: envelope(message(full)), replay: false },
      })
    );
    await expect(page.locator('code[data-language="json"]')).toContainText(
      '"ok"'
    );
    for (const language of ['tsx', 'rust', 'python']) {
      const block = page.locator(`code[data-language="${language}"]`);
      await expect(block).toBeVisible();
      await expect.poll(() => block.locator('span[style*="color"]').count(), {
        timeout: 15000,
      }).toBeGreaterThan(0);
    }
    await expect(page.locator('code[data-language="tsx"]')).toContainText('<img src=x onerror=window.local432Injected=1>');
    expect(await page.locator('code[data-language="tsx"] img').count()).toBe(0);
    expect(await page.evaluate(() => window.local432Injected)).toBeUndefined();
    report.checks.push('TSX, Rust and Python Markdown code blocks receive syntax colors');
    expect(await composer.textContent()).toBe('入力を保持する');
    expect(await composer.evaluate((el) => el === document.activeElement)).toBe(
      true
    );
    const assistantRow = page.locator('[data-row-index]').filter({
      has: page.getByText('LOCAL32 code', { exact: true }),
    });
    await assistantRow.scrollIntoViewIfNeeded();
    await assistantRow.hover();
    await assistantRow
      .getByRole('button', { name: /Markdownとしてコピー|Copy as Markdown/ })
      .click();
    await expect
      .poll(() => page.evaluate(() => navigator.clipboard.readText()))
      .toBe(full);
    report.checks.push(
      'stream update preserves composer and Markdown copy equals fixture source'
    );
    for (const language of ['unknown-language', 'text'])
      expect(
        await page
          .locator(`code[data-language="${language}"] span[style*="color"]`)
          .count()
      ).toBe(0);
    const colors = [];
    const editorColors = [];
    const bashBlock = page.locator('code[data-language="bash"]').first();
    const bashContent = await bashBlock.textContent();
    const dimensions = [];
    for (const theme of ['light', 'dark']) {
      await page.emulateMedia({ colorScheme: theme });
      await expect(page.locator('html')).toHaveClass(theme === 'dark' ? /dark/ : /light/);
      const token = commandPre.locator('.shiki-token').first();
      const color = await token.evaluate((el) => getComputedStyle(el).color);
      colors.push(color);
      const editorToken = bashBlock.locator('span[style*="color"]').first();
      await expect(editorToken).toBeAttached();
      if (editorColors.length) {
        await expect.poll(() => editorToken.evaluate((el) => getComputedStyle(el).color)).not.toBe(editorColors[0]);
      }
      editorColors.push(await editorToken.evaluate((el) => getComputedStyle(el).color));
      expect(await bashBlock.textContent()).toBe(bashContent);
      dimensions.push(await bashBlock.evaluate((el) => ({
        width: el.getBoundingClientRect().width,
        height: el.getBoundingClientRect().height,
      })));
      // Existing button descendants transition colors for 200ms on theme changes.
      await expect
        .poll(() =>
          commandPre
            .locator('.shiki-token')
            .first()
            .evaluate((el) => getComputedStyle(el).color)
        )
        .toBe(color);
      report.contrast ??= [];
      report.contrast.push({
        theme,
        samples: await page.evaluate(() => {
          const rgb = (value) => value.match(/[\d.]+/g)?.map(Number) ?? [];
          const luminance = (color) => {
            const linear = color.slice(0, 3).map((v) => {
              const n = v / 255;
              return n <= 0.04045 ? n / 12.92 : ((n + 0.055) / 1.055) ** 2.4;
            });
            return linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722;
          };
          return [
            ...document.querySelectorAll(
              '.shiki-token, button .text-low, time, pre.text-low'
            ),
          ]
            .filter(
              (el) =>
                el.getBoundingClientRect().height > 0 && el.textContent?.trim()
            )
            .slice(0, 20)
            .map((el) => {
              const foreground = getComputedStyle(el).color;
              let background = 'rgb(255, 255, 255)';
              for (let parent = el; parent; parent = parent.parentElement) {
                const value = getComputedStyle(parent).backgroundColor;
                const channels = rgb(value);
                if (channels.length === 3 || channels[3] === 1) {
                  background = value;
                  break;
                }
              }
              const a = luminance(rgb(foreground));
              const b = luminance(rgb(background));
              return {
                text: el.textContent?.slice(0, 60),
                foreground,
                background,
                ratio: (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05),
              };
            });
        }),
      });
      await page.screenshot({
        path: join(evidence, `${theme}.png`),
        fullPage: true,
      });
    }
    expect(colors[0]).not.toBe(colors[1]);
    expect(Math.abs(dimensions[0].width - dimensions[1].width)).toBeLessThan(1);
    expect(Math.abs(dimensions[0].height - dimensions[1].height)).toBeLessThan(1);
    report.themeDimensions = dimensions;
    report.colors = colors;
    report.editorColors = editorColors;
    report.checks.push(
      'stream completion, JSON, unknown/plain/unspecified and system theme changes for command and Lexical with screenshots'
    );
    await page.getByText(/変更ファイル1件/).first().click();
    await expect(page.getByText('src/fixture.ts', { exact: true }).first()).toBeVisible();
    const diff = page.locator('diffs-container').first();
    await expect(diff).toBeVisible();
    await expect(diff).toContainText('answer');
    const diffLine = diff.locator('[data-line="1"]').first();
    await expect(diffLine.locator('span').first()).toBeAttached();
    await expect.poll(() => diffLine.locator('span').evaluateAll((spans) =>
      new Set(spans.map((span) => getComputedStyle(span).color)).size
    ), { timeout: 15000 }).toBeGreaterThan(1);
    const diffText = await diffLine.innerText();
    const diffThemeColors = [];
    for (const theme of ['light', 'dark']) {
      await page.emulateMedia({ colorScheme: theme });
      await expect(page.locator('html')).toHaveClass(theme === 'dark' ? /dark/ : /light/);
      const color = await diffLine.locator('span').first().evaluate((el) => getComputedStyle(el).color);
      diffThemeColors.push(color);
      expect(await diffLine.innerText()).toBe(diffText);
    }
    expect(diffThemeColors[0]).not.toBe(diffThemeColors[1]);
    report.diffThemeColors = diffThemeColors;
    report.diffStyles = await diffLine.evaluate((el) => ({
      html: el.innerHTML,
      colors: [...el.querySelectorAll('span')].map((span) => getComputedStyle(span).color),
    }));
    expect(new Set(report.diffStyles.colors).size).toBeGreaterThan(1);
    await diff.getByRole('button', { name: 'パスをコピー' }).click();
    await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe('src/fixture.ts');
    await diff.locator('[data-column-number="1"][data-line-type="change-addition"]').click();
    const reviewEditor = diff.locator('[contenteditable="true"]');
    await expect(reviewEditor).toBeVisible();
    await reviewEditor.fill('隔離レビュー');
    await diff.getByRole('button', { name: /レビューコメントを追加/ }).click();
    await expect(diff.getByText('隔離レビュー')).toBeVisible();
    const scrollRoot = page.locator('diffs-container').first().locator('xpath=ancestor::*[contains(@class,"overflow-auto")][1]');
    const scrollResult = await scrollRoot.evaluate((el) => {
      el.scrollTop = el.scrollHeight;
      return { top: el.scrollTop, height: el.scrollHeight, client: el.clientHeight };
    });
    expect(scrollResult.height).toBeGreaterThan(scrollResult.client);
    expect(scrollResult.top).toBeGreaterThan(0);
    await page.screenshot({ path: join(evidence, 'changes.png'), fullPage: true });
    report.checks.push('Pierre Changes panel renders a synthetic file diff, copies its path, adds a review comment and scrolls');
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
    await expect.poll(() => longBlock.textContent()).toBe(long);
    expect(await longBlock.locator('span[style*="color"]').count()).toBe(0);
    report.checks.push(
      'long streamed block remains complete with plain fallback'
    );
    const streamedLines = Array.from(
      { length: 40 },
      (_, i) => `echo "$HOME" && printf 'line ${i} %s\\n' value`
    );
    await page.evaluate(() => {
      const samples = [];
      const tasks = [];
      const observer = new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) tasks.push(entry.duration);
      });
      observer.observe({ type: 'longtask', buffered: false });
      let active = true;
      const sample = (time) => {
        if (!active) return;
        const block = [...document.querySelectorAll('code[data-language="bash"]')].at(-1);
        if (block) {
          const rect = block.getBoundingClientRect();
          samples.push({ time, length: block.textContent.length, height: rect.height,
            colored: Boolean(block.querySelector('span[style*="color"]')) });
        }
        requestAnimationFrame(sample);
      };
      requestAnimationFrame(sample);
      window.local432StopMeasurement = () => {
        active = false;
        observer.disconnect();
        return { samples, tasks };
      };
    });
    for (let i = 1; i <= 20; i++) {
      stream.send(JSON.stringify({
        type: 'event',
        data: {
          event: envelope({
            type: 'message',
            data: { message: {
              message_id: 'stream-perf',
              role: 'assistant',
              content: (i === 1 ? 'Streaming sample\n\n```bash\n' : '') +
                streamedLines.slice((i - 1) * 2, i * 2).join('\n') +
                (i === 20 ? '\n```' : '\n'),
            }, final_output: false },
          }),
          replay: false,
        },
      }));
      await new Promise((resolve) => setTimeout(resolve, 40));
    }
    const streamedBlock = page.locator('code[data-language="bash"]').last();
    await expect.poll(() => streamedBlock.innerText()).toBe(streamedLines.join('\n'));
    await page.waitForTimeout(250);
    const streamingMeasurement = await page.evaluate(() => window.local432StopMeasurement());
    const intervals = streamingMeasurement.samples.slice(1).map((sample, i) =>
      sample.time - streamingMeasurement.samples[i].time
    );
    report.streamingMeasurement = {
      updatesSent: 20,
      bytes: Buffer.byteLength(streamedLines.join('\n')),
      frames: streamingMeasurement.samples.length,
      maxFrameIntervalMs: Math.max(0, ...intervals),
      longTasksMs: streamingMeasurement.tasks,
      samples: streamingMeasurement.samples,
    };
    report.streamedBlockHtml = await streamedBlock.evaluate((el) => el.innerHTML.slice(0, 2000));
    await expect.poll(() => streamedBlock.locator('span[style*="color"]').count()).toBeGreaterThan(0);
    report.checks.push('20 streamed updates retain the final source; frame and long-task samples recorded');
    if (additional) {
      // The follow-up transport is intercepted before HTTP reaches the test API;
      // no provider is launched, including on the rejected/retried submission.
      let pendingFollowUp;
      await page.route(`**/api/sessions/${session}/follow-up`, (route) => {
        pendingFollowUp = route;
      });
      await composer.fill('送信直後の本文');
      await composer.press('Enter');
      const pendingRow = page.locator('[data-pending-delivery="sending"]');
      await expect(pendingRow).toContainText('送信直後の本文');
      await page.screenshot({
        path: join(evidence, 'pending-send.png'),
        fullPage: true,
      });
      await expect.poll(() => Boolean(pendingFollowUp)).toBe(true);
      await pendingFollowUp.fulfill({
        status: 403,
        json: { success: false, message: 'Fixture rejection' },
      });
      await expect(
        page.locator('[data-pending-delivery="failed"]')
      ).toContainText('Fixture rejection');
      await expect(composer).toHaveText('送信直後の本文');
      pendingFollowUp = null;
      await composer.press('Enter');
      await expect(
        page.locator('[data-pending-delivery="sending"]')
      ).toHaveCount(1);
      stream.send(
        JSON.stringify({
          type: 'event',
          data: {
            event: envelope({
              type: 'message',
              data: {
                message: {
                  message_id: 'synthetic-user-send',
                  role: 'user',
                  content: '送信直後の本文',
                },
                final_output: false,
              },
            }),
            replay: false,
          },
        })
      );
      await expect(page.locator('[data-pending-delivery]')).toHaveCount(0);
      await expect.poll(() => Boolean(pendingFollowUp)).toBe(true);
      await pendingFollowUp.fulfill({
        json: { success: true, data: { agent_run_id: run, state } },
      });
      await expect(composer).toHaveText('');
      report.checks.push(
        'real composer/list shows pending before delayed ACK; explicit rejection retains draft; event-before-ACK reconciles retry exactly once; no provider call'
      );
      // Native live summaries are small active notes, then authoritative content
      // replaces them without turning commentary into a disclosure.
      const activeState = {
        ...state,
        status: 'running',
        updated_at: new Date(Date.parse(stamp) + 1000).toISOString(),
        last_event_sequence: seq,
      };
      stream.send(
        JSON.stringify({
          type: 'state',
          data: { state: activeState, cursor: null },
        })
      );
      stream.send(
        JSON.stringify({
          type: 'live',
          data: {
            event: {
              schema_version: 1,
              event_id: randomUUID(),
              session_id: session,
              agent_run_id: run,
              turn_id: run,
              run_attempt_id: run,
              run_attempt_number: 1,
              native_sequence: ++seq,
              timestamp: new Date(Date.parse(stamp) + 1000).toISOString(),
              payload: {
                type: 'thinking_delta',
                data: {
                  provider_item_id: 'active-summary',
                  delta: 'Inspecting the next source file',
                  summary_index: 0,
                },
              },
            },
          },
        })
      );
      const summaryLine = page
        .locator('summary')
        .getByText('Inspecting the next source file', { exact: true });
      await expect(summaryLine).toHaveClass(/vk-activity-text/);
      await summaryLine.scrollIntoViewIfNeeded();
      await page.screenshot({
        path: join(evidence, 'notes-active.png'),
        fullPage: true,
      });
      stream.send(
        JSON.stringify({
          type: 'event',
          data: {
            event: envelope({
              type: 'provider_extension',
              data: {
                provider_namespace: 'codex',
                provider_event: 'reasoning_summary',
                payload: {
                  provider_item_id: 'active-summary',
                  content:
                    'Inspecting the next source file\nOriginal completed summary.',
                },
              },
            }),
            replay: false,
          },
        })
      );
      await expect(summaryLine).not.toHaveClass(/vk-activity-text/);
      await expect(
        summaryLine.locator('xpath=ancestor::details')
      ).not.toHaveAttribute('open');
      stream.send(
        JSON.stringify({
          type: 'state',
          data: {
            state: {
              ...state,
              updated_at: new Date(Date.parse(stamp) + 2000).toISOString(),
              last_event_sequence: seq,
            },
            cursor: null,
          },
        })
      );
      await page.setViewportSize({ width: 375, height: 900 });
      await page.screenshot({
        path: join(evidence, 'narrow-notes.png'),
        fullPage: true,
      });
      report.checks.push(
        'live summary progress stops on completed authoritative summary and remains collapsible; narrow screen captured'
      );
    }
  }
  expect(report.errors).toEqual([]);
  expect(report.consoleErrors).toEqual([]);
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
  report.finishedAt = new Date().toISOString();
  writeFileSync(
    join(evidence, 'isolated-e2e.json'),
    JSON.stringify(report, null, 2)
  );
  console.log(JSON.stringify(report, null, 2));
}
