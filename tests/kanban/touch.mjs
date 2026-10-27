import { verifyBackendIdentity } from './backend-identity.mjs';
import { isolatedEnvironment } from '../server/fixtures/issue-comments-env.mjs';
import { createServer as createNetServer } from 'node:net';
import { touchChecks } from './touch-checks.mjs';
// Fresh SQLite and real API; source frontend via Vite. No production data/model calls.
import { chromium } from '@playwright/test';
import { createServer } from 'vite';
import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { createWriteStream, createReadStream, constants } from 'node:fs';
import { createHash } from 'node:crypto';
import {
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
  copyFile,
} from 'node:fs/promises';
import { once } from 'node:events';
import path from 'node:path';
const root = process.cwd();
const output = path.join(root, '.build/touch-validation');
await mkdir(output, { recursive: true });
const instance = await mkdtemp(path.join(output, 'instance-'));
const binary = process.env.LVK_TEST_SERVER;
assert.ok(binary, 'Set LVK_TEST_SERVER to a compatible prebuilt backend');
const env = isolatedEnvironment(process.env, instance, root);
// Background capability discovery must never launch installed agents.
const bin = path.join(instance, 'bin');
await mkdir(bin);
for (const name of ['codex', 'claude'])
  await writeFile(path.join(bin, name), '#!/bin/sh\nexit 1\n', { mode: 0o755 });
env.PATH = `${bin}:${env.PATH}`;
// Execute a private binary snapshot, never a managed instance's running file.
const executedBinary = path.join(instance, 'server');
await copyFile(binary, executedBinary, constants.COPYFILE_FICLONE);
async function sha256(file) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(file)) hash.update(chunk);
  return hash.digest('hex');
}
const binarySha256 = await sha256(binary);
assert.equal(await sha256(executedBinary), binarySha256);
const server = spawn(executedBinary, [], {
  cwd: root,
  env,
  stdio: ['ignore', 'pipe', 'pipe'],
});
server.stdout.pipe(createWriteStream(path.join(output, 'server.stdout.log')));
server.stderr.pipe(createWriteStream(path.join(output, 'server.stderr.log')));
let browser, page, vite, base;

async function api(url, method = 'GET', body) {
  const response = await fetch(base + url, {
    method,
    redirect: 'error',
    headers: { 'content-type': 'application/json', 'x-lvk-csrf': 'local' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const result = await response.json();
  assert.equal(response.status, 200, `${url}: ${JSON.stringify(result)}`);
  assert.notEqual(result.success, false, JSON.stringify(result));
  return result;
}

try {
  for (let i = 0; i < 120; i++) {
    assert.equal(server.exitCode, null, 'backend exited');
    try {
      const ports = JSON.parse(
        await readFile(path.join(instance, 'ports/vibe-kanban.port'), 'utf8')
      );
      base = `http://127.0.0.1:${ports.main_port}`;
      if ((await fetch(base + '/api/info')).ok) break;
    } catch {}
    await new Promise((r) => setTimeout(r, 500));
  }
  assert.ok(base, 'backend did not start');
  const info = (await api('/api/info')).data;
  const verifiedBackend = await verifyBackendIdentity(binary, info, root);
  await writeFile(
    path.join(output, 'revision.json'),
    JSON.stringify(
      {
        source: execFileSync('git', ['rev-parse', 'HEAD'], {
          cwd: root,
          encoding: 'utf8',
        }).trim(),
        dirty: execFileSync('git', ['status', '--porcelain'], {
          cwd: root,
          encoding: 'utf8',
        }),
        ...verifiedBackend,
        binary,
        executedBinary,
        binarySha256,
        build: info.build_info,
      },
      null,
      2
    )
  );
  await api('/api/config', 'PUT', {
    ...info.config,
    language: 'EN',
    theme: 'DARK',
    disclaimer_acknowledged: true,
    onboarding_acknowledged: true,
    remote_onboarding_acknowledged: true,
    telemetry_acknowledged: true,
    analytics_enabled: false,
    showcases: { seen_features: ['workspaces-guide'] },
  });
  const project = (
    await api('/api/local/v1/projects', 'POST', {
      key: 'LOCAL',
      organization_id: '00000000-0000-0000-0000-000000000002',
      name: 'Compact Kanban E2E',
      color: '#6366f1',
    })
  ).data.id;
  const statuses = (
    await api(`/api/local/v1/fallback/project_statuses?project_id=${project}`)
  ).project_statuses
    .filter((s) => !s.hidden)
    .sort((a, b) => a.sort_order - b.sort_order);
  assert.equal(statuses.length, 4);
  const issues = [];
  for (const [column, status] of statuses.entries()) {
    for (let i = 0; i < (column === 3 ? 0 : 10); i++)
      issues.push(
        (
          await api('/api/local/v1/issues', 'POST', {
            project_id: project,
            status_id: status.id,
            title: `Card ${column}-${i} 長いタイトル ${'readable title '.repeat(10)}`,
            description:
              'Specification body\nSecond line for expanded content.',
            sort_order: i,
            extension_metadata: {},
          })
        ).data
      );
  }
  const tag = (
    await api('/api/local/v1/tags', 'POST', {
      project_id: project,
      name: 'Touch tag',
      color: '120 60% 40%',
    })
  ).data;
  await api('/api/local/v1/issue_tags', 'POST', {
    issue_id: issues[0].id,
    tag_id: tag.id,
  });
  process.env.BACKEND_PORT = new URL(base).port;
  process.chdir(path.join(root, 'packages/local-web'));
  const portProbe = createNetServer();
  await new Promise((resolve) => portProbe.listen(0, '127.0.0.1', resolve));
  const frontendPort = portProbe.address().port;
  await new Promise((resolve) => portProbe.close(resolve));
  vite = await createServer({
    configFile: path.join(root, 'packages/local-web/vite.config.ts'),
    server: {
      port: frontendPort,
      strictPort: true,
      host: '127.0.0.1',
      open: false,
    },
  });
  await vite.listen();
  const url = `http://127.0.0.1:${vite.httpServer.address().port}/projects/${project}`;
  browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
  page = await browser.newPage({
    viewport: { width: 390, height: 844 },
    isMobile: true,
    hasTouch: true,
  });
  page.setDefaultTimeout(15000);
  await touchChecks({ browser, page, url, issues, statuses, api, output, tag });
  console.log('PASS touch real application E2E');
} catch (error) {
  await page
    ?.screenshot({ path: path.join(output, 'failure.png') })
    .catch(() => {});
  throw error;
} finally {
  await browser?.close();
  await vite?.close();
  if (server.exitCode === null && server.signalCode === null) {
    const exited = once(server, 'exit');
    server.kill('SIGTERM');
    const timer = setTimeout(() => server.kill('SIGKILL'), 10000);
    await exited;
    clearTimeout(timer);
  }
  await rm(instance, { recursive: true, force: true });
  process.chdir(root);
  await writeFile(
    path.join(output, 'cleanup.json'),
    JSON.stringify({
      browserClosed: true,
      viteClosed: true,
      backendStopped: true,
      instanceRemoved: true,
    })
  );
}
