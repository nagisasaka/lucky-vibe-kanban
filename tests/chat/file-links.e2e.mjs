import assert from 'node:assert/strict';
import { build } from 'esbuild';
import { chromium } from '@playwright/test';
import { spawn, execFileSync } from 'node:child_process';
import { once } from 'node:events';
import {
  mkdtemp,
  mkdir,
  writeFile,
  readFile,
  readdir,
  rm,
  symlink,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { randomUUID } from 'node:crypto';

// Run after building this checkout's server and local-web. Own instance, DB,
// runtime, kernel-assigned loopback ports, browser context and foreground child.
const nativeLayout = process.argv.includes('--native');
const scratch = await mkdtemp(join(tmpdir(), 'lvk-chat-files-'));
const instance = join(scratch, 'instance');
const workspace = join(scratch, 'workspace');
const repo = join(workspace, 'project');
const workspaceId = randomUUID();
const repoId = randomUUID();
const foreignRepoId = randomUUID();
let server;
let browser;
let log = '';
let spawnError;
let cleanupPromise;
function cleanup() {
  return (cleanupPromise ??= (async () => {
    try {
      await browser?.close();
    } finally {
      if (
        server?.pid &&
        server.exitCode === null &&
        server.signalCode === null
      ) {
        const exited = once(server, 'exit');
        server.kill('SIGTERM');
        const timeout = setTimeout(() => server.kill('SIGKILL'), 10_000);
        await exited;
        clearTimeout(timeout);
      }
      await rm(scratch, { recursive: true, force: true });
    }
  })());
}
process.once('SIGINT', () => {
  void cleanup().finally(() => process.exit(130));
});
process.once('SIGTERM', () => {
  void cleanup().finally(() => process.exit(143));
});
try {
  await mkdir(join(repo, 'src'), { recursive: true });
  await writeFile(
    join(repo, 'src/example.ts'),
    'first\nsecond\nconst selected = 3;\nconst fourth = 4;\n'
  );
  await writeFile(
    join(repo, 'src/space 日本語.txt'),
    'space first\nspace second\n'
  );
  await writeFile(join(repo, 'README.md'), '# Title\nmarkdown selected\n');
  await writeFile(join(scratch, 'secret.txt'), 'OUTSIDE_SECRET_DO_NOT_READ');
  await symlink(join(scratch, 'secret.txt'), join(repo, 'escape.txt'));
  server = spawn(resolve('target/debug/server'), [], {
    cwd: process.cwd(),
    env: {
      ...process.env,
      LVK_INSTANCE_DIR: instance,
      LVK_ACCESS_DEPLOYMENT: 'local',
      LVK_PUBLIC_URL: '',
      LVK_AUTH_MODE: 'local',
      VK_ALLOWED_ORIGINS: '',
      VK_PREVIEW_DOMAIN: '',
      HOST: '127.0.0.1',
      PORT: '0',
      BACKEND_PORT: '0',
      PREVIEW_PROXY_PORT: '0',
      DISABLE_WORKTREE_CLEANUP: '1',
      RUST_LOG: 'info',
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  server.stdout.on('data', (data) => {
    log += data;
  });
  server.stderr.on('data', (data) => {
    log += data;
  });
  server.on('error', (error) => {
    log += error.message;
    spawnError = error;
  });
  let base;
  for (let attempt = 0; attempt < 180; attempt++) {
    if (spawnError) throw spawnError;
    if (server.exitCode !== null) throw new Error(`server exited: ${log}`);
    try {
      const ports = JSON.parse(
        await readFile(join(instance, 'ports/vibe-kanban.port'), 'utf8')
      );
      base = `http://127.0.0.1:${ports.main_port}`;
      if ((await fetch(`${base}/api/info`)).ok) break;
    } catch {}
    base = undefined;
    await new Promise((r) => setTimeout(r, 500));
  }
  assert.ok(base, `server did not become ready: ${log}`);
  console.log('Isolated backend ready:', base);
  const dbFiles = await readdir(join(instance, 'data'));
  const db = dbFiles.find((name) => name === 'db.v2.sqlite');
  assert.ok(db, `database missing: ${dbFiles}`);
  // Seed only this disposable DB. execution_only uses existing inspection roots,
  // so no agent, worktree setup script or repository checkout is launched.
  execFileSync('python3', [
    '-c',
    `
import sqlite3,sys,uuid
c=sqlite3.connect(sys.argv[1])
w,r,f=[uuid.UUID(v).bytes for v in sys.argv[2:5]]
root=sys.argv[5]
c.execute("INSERT INTO workspaces(id,container_ref,branch,name,usage,created_at,updated_at) VALUES(?,?, 'main','file links E2E','execution_only',datetime('now'),datetime('now'))",(w,root))
c.execute("INSERT INTO repos(id,path,name,display_name,created_at,updated_at) VALUES(?,?, 'project','Project',datetime('now'),datetime('now'))",(r,root+'/project'))
c.execute("INSERT INTO repos(id,path,name,display_name,created_at,updated_at) VALUES(?,?, 'foreign','Foreign',datetime('now'),datetime('now'))",(f,root+'/foreign'))
c.execute("INSERT INTO workspace_repos(id,workspace_id,repo_id,target_branch,created_at,updated_at) VALUES(?,?,?,'main',datetime('now'),datetime('now'))",(uuid.uuid4().bytes,w,r))
c.commit()
`,
    join(instance, 'data', db),
    workspaceId,
    repoId,
    foreignRepoId,
    workspace,
  ]);

  if (nativeLayout) {
    await writeFile(
      join(repo, 'src/example.ts'),
      Array.from({ length: 160 }, (_, i) =>
        i === 119 ? 'const nativeSelected = 120;' : `// native line ${i + 1}`
      ).join('\n')
    );
    execFileSync('git', ['init', '-b', 'main', repo]);
    execFileSync('git', ['-C', repo, 'add', '.']);
    execFileSync('git', [
      '-C',
      repo,
      '-c',
      'user.name=E2E',
      '-c',
      'user.email=e2e@example.invalid',
      'commit',
      '-m',
      'Native file link fixture',
    ]);
    const seed = JSON.parse(
      execFileSync(
        'python3',
        [
          'tests/chat/file-links.native-seed.py',
          join(instance, 'data', db),
          workspaceId,
          workspace,
        ],
        { encoding: 'utf8' }
      )
    );
    browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
    const context = await browser.newContext({
      viewport: { width: 1440, height: 1000 },
    });
    const page = await context.newPage();
    page.setDefaultTimeout(30_000);
    const errors = [];
    page.on('response', (response) => {
      if (response.url().includes('/files/'))
        console.log('Native file response:', response.status(), response.url());
    });
    page.on('pageerror', (error) => errors.push(error.message));
    const { verifyNativeFileLink } = await import('./file-links.native.mjs');
    try {
      await verifyNativeFileLink(
        page,
        base,
        workspaceId,
        repoId,
        seed.session_id
      );
      assert.deepEqual(errors, []);
    } catch (error) {
      console.error('Native page:', await page.locator('body').innerText());
      console.error('Browser errors:', errors);
      throw error;
    }
    console.log(
      JSON.stringify({
        result: 'passed',
        mode: 'native-layout',
        backend: base,
        chromium: browser.version(),
        mocks: 'none; dedicated DB conversation seed only',
      })
    );
  } else {
    const result = await build({
      entryPoints: ['tests/chat/file-links.fixture.tsx'],
      bundle: true,
      write: false,
      format: 'iife',
      globalName: 'fileLinks',
      jsx: 'automatic',
      nodePaths: [resolve('packages/web-core/node_modules')],
      alias: { '@': resolve('packages/web-core/src') },
      define: {
        'process.env.NODE_ENV': '"production"',
        'import.meta.env': '{}',
        'import.meta.hot': 'undefined',
      },
      loader: { '.css': 'empty', '.woff2': 'dataurl', '.wasm': 'dataurl' },
    });
    browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
    const context = await browser.newContext();
    const page = await context.newPage();
    page.setDefaultTimeout(30_000);
    const errors = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await page.goto(base, { waitUntil: 'networkidle' });
    assert.match(await page.title(), /Lucky Vibe Kanban/i);
    assert.ok(
      await page.locator('#root').innerText(),
      'built application renders'
    );
    console.log('Built application rendered');
    // Mount production chat + preview components on the real backend origin.
    // Only text/layout are synthetic; API requests are not intercepted or mocked.
    await page.addScriptTag({ content: result.outputFiles[0].text });
    await page.evaluate(
      ({ workspaceId, workspace }) =>
        window.fileLinks.mount(workspaceId, workspace),
      { workspaceId, workspace }
    );
    const chat = page.getByRole('region', { name: 'Chat' });
    const preview = page.getByRole('region', { name: 'File preview' });
    await chat.getByRole('link', { name: 'relative', exact: true }).waitFor();
    await page.waitForFunction(() =>
      document.querySelector('section a[tabindex="0"]')
    );
    for (const [label, text] of [
      ['relative', 'const selected = 3;'],
      ['absolute', 'const fourth = 4;'],
      ['spaces', 'space second'],
      ['encoded', 'space second'],
      ['markdown', 'markdown selected'],
    ]) {
      const link = chat.getByRole('link', { name: label, exact: true });
      if (label === 'absolute') {
        await link.focus();
        await page.keyboard.press('Enter');
      } else await link.click();
      await preview
        .locator('[aria-current="location"]')
        .filter({ hasText: text })
        .waitFor();
      assert.equal(context.pages().length, 1);
      console.log('Opened chat file link:', label);
    }
    for (const label of ['missing', 'symlink']) {
      await chat.getByRole('link', { name: label, exact: true }).click();
      await preview
        .getByText('Could not load preview', { exact: true })
        .waitFor();
      assert.ok(!(await preview.innerText()).includes('OUTSIDE_SECRET'));
    }
    for (const label of [
      'outside',
      'traversal',
      'encoded-traversal',
      'dangerous',
    ]) {
      const link = chat.getByRole('link', { name: label, exact: true });
      assert.equal(await link.getAttribute('aria-disabled'), 'true');
      assert.equal(await link.getAttribute('href'), null);
    }
    const external = chat.getByRole('link', { name: 'external', exact: true });
    assert.equal(
      await external.getAttribute('href'),
      'https://example.com/docs'
    );
    assert.equal(await external.getAttribute('target'), '_blank');
    assert.equal(await external.getAttribute('rel'), 'noopener noreferrer');

    // Real backend boundary regressions, deliberately bypassing frontend guards.
    const contentUrl = (path, id = repoId, ws = workspaceId) =>
      `${base}/api/workspaces/${ws}/files/content?${new URLSearchParams({ repo_id: id, path })}`;
    const good = await fetch(contentUrl('src/example.ts'));
    assert.equal(good.status, 200);
    assert.match(JSON.stringify(await good.json()), /const selected = 3/);
    for (const [path, status, id, ws] of [
      ['../secret.txt', 400],
      ['/etc/passwd', 400],
      ['C:\\Windows\\win.ini', 400],
      ['escape.txt', 404],
      ['missing.txt', 404],
      ['src/example.ts', 404, foreignRepoId],
      ['src/example.ts', 404, repoId, randomUUID()],
    ]) {
      for (const endpoint of ['content', 'raw']) {
        const response = await fetch(
          contentUrl(path, id, ws).replace(
            '/files/content?',
            `/files/${endpoint}?`
          )
        );
        assert.equal(
          response.status,
          status,
          `${endpoint} must reject ${path} ${id ?? ''}`
        );
        assert.ok(!(await response.text()).includes('OUTSIDE_SECRET'));
      }
    }
    assert.deepEqual(errors, []);
    console.log(
      JSON.stringify(
        {
          result: 'passed',
          backend: base,
          chromium: browser.version(),
          cases: [
            'built app render',
            'relative :line',
            'absolute #L keyboard',
            'spaces and Unicode',
            'encoded filename',
            'markdown line',
            'missing error',
            'symlink confinement',
            'disabled unsafe links',
            'HTTPS native attributes',
            'real API traversal/absolute/repo membership/workspace isolation',
          ],
          mocks: 'none; seeded DB and synthetic conversation layout only',
        },
        null,
        2
      )
    );
  }
} catch (error) {
  console.error(log.slice(-8000));
  throw error;
} finally {
  await cleanup();
}
