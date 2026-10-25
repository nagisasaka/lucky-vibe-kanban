// Test-owned SQLite/ports/browser only. Never accepts an existing instance URL.
import { chromium, expect } from '@playwright/test';
import { createServer } from 'vite';
import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { createWriteStream } from 'node:fs';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { once } from 'node:events';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { isolatedEnvironment } from '../server/fixtures/issue-comments-env.mjs';
const root = process.cwd();
const output = path.join(
  root,
  process.env.LVK_TEST_OUTPUT || '.build/issue-archive-validation'
);
await mkdir(output, { recursive: true });
const instance = await mkdtemp(path.join(output, 'instance-'));
const binary = path.join(root, 'target/debug/server');
const env = isolatedEnvironment(process.env, instance, root);
const fixtureBin = path.join(instance, 'fixture-bin');
await mkdir(fixtureBin);
for (const command of ['codex', 'claude'])
  await writeFile(path.join(fixtureBin, command), '#!/bin/sh\nexit 1\n', {
    mode: 0o755,
  });
env.PATH = `${fixtureBin}:${env.PATH}`;
const server = spawn(binary, [], {
  cwd: root,
  env,
  stdio: ['ignore', 'pipe', 'pipe'],
});
server.stdout.pipe(createWriteStream(path.join(output, 'server.stdout.log')));
server.stderr.pipe(createWriteStream(path.join(output, 'server.stderr.log')));
let browser, page, vite, base;
const evidence = [];
async function api(url, method = 'GET', body) {
  const response = await fetch(base + url, {
    method,
    redirect: 'error',
    headers: { 'content-type': 'application/json', 'x-lvk-csrf': 'local' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const result = await response.json();
  assert.equal(response.status, 200, `${url}: ${JSON.stringify(result)}`);
  assert.notEqual(result.success, false);
  return result;
}
try {
  for (let i = 0; i < 120; i++) {
    assert.equal(server.exitCode, null, 'isolated backend exited');
    try {
      const ports = JSON.parse(
        await readFile(path.join(instance, 'ports/vibe-kanban.port'), 'utf8')
      );
      base = `http://127.0.0.1:${ports.main_port}`;
      if ((await fetch(base + '/api/info')).ok) break;
    } catch {}
    await new Promise((r) => setTimeout(r, 500));
  }
  assert.ok(base, 'backend startup');
  const info = (await api('/api/info')).data;
  await writeFile(
    path.join(output, 'revision.json'),
    JSON.stringify(
      {
        source: execFileSync('git', ['rev-parse', 'HEAD'], {
          encoding: 'utf8',
        }).trim(),
        dirty: execFileSync('git', ['status', '--porcelain'], {
          encoding: 'utf8',
        }),
        binary,
        sha256: createHash('sha256')
          .update(await readFile(binary))
          .digest('hex'),
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
      organization_id: '00000000-0000-0000-0000-000000000002',
      name: 'Issue archive fixture',
      color: '#6366f1',
    })
  ).data.id;
  const project2 = (
    await api('/api/local/v1/projects', 'POST', {
      organization_id: '00000000-0000-0000-0000-000000000002',
      name: 'Reset target',
      color: '#6366f1',
    })
  ).data.id;
  const statuses = (
    await api(`/api/local/v1/fallback/project_statuses?project_id=${project}`)
  ).project_statuses.sort((a, b) => a.sort_order - b.sort_order);
  const issues = [];
  for (const [i, status] of statuses.entries())
    issues.push(
      (
        await api('/api/local/v1/issues', 'POST', {
          project_id: project,
          status_id: status.id,
          title: `Archive fixture ${i}`,
          description: 'Retained issue body',
          sort_order: 1000 + i,
          extension_metadata: {},
        })
      ).data
    );
  const ws = (
    await api('/api/workspaces', 'POST', { name: 'Retained child fixture' })
  ).data.id;
  await api(`/api/workspaces/${ws}/links`, 'POST', {
    project_id: project,
    issue_id: issues[0].id,
  });
  const prepare = (
    await api(`/api/board/issues/${issues[3].id}/archive/prepare`, 'POST', {
      project_id: project,
      archive_workspaces: false,
    })
  ).data;
  assert.equal(
    (
      await api(`/api/board/issues/${issues[3].id}/archive/confirm`, 'POST', {
        project_id: project,
        confirmation_token: prepare.confirmation_token,
        request_id: crypto.randomUUID(),
      })
    ).data.status,
    'succeeded'
  );
  // Real stdio-facing REST path: filter before offset/count, including hidden statuses.
  for (const archived of [undefined, false, true, 'all']) {
    const expected = issues.filter((_, i) =>
      archived === 'all' || (i === 3) === (archived === true)
    );
    const ids = [];
    for (let offset = 0; offset <= expected.length; offset++) {
      const result = (await api('/api/board/issues/filter', 'POST', {
        project_id: project, archived, search: 'ARCHIVE FIXTURE',
        sort_field: 'title', sort_direction: 'asc', limit: 1, offset,
      })).data;
      assert.equal(result.total_count, expected.length);
      assert.equal(result.offset, offset);
      assert.equal(result.limit, 1);
      ids.push(...result.issues.map((i) => i.id));
    }
    assert.deepEqual(ids, expected.map((i) => i.id));
    const exact = (await api('/api/board/issues/filter', 'POST', {
      project_id: project, archived, simple_id: issues[3].simple_id.toLowerCase(),
      status: statuses[3].id, search: 'retained ISSUE body',
    })).data;
    assert.equal(exact.total_count, archived === true || archived === 'all' ? 1 : 0);
  }
  evidence.push('PASS shared MCP REST: archive default/false/true/all AND search/ID/status before offset/count; hidden included');
  process.env.BACKEND_PORT = new URL(base).port;
  process.chdir(path.join(root, 'packages/local-web'));
  vite = await createServer({
    configFile: path.join(root, 'packages/local-web/vite.config.ts'),
    server: { port: 0, host: '127.0.0.1', open: false },
  });
  await vite.listen();
  const url = `http://127.0.0.1:${vite.httpServer.address().port}`;
  browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
  page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  page.setDefaultTimeout(20000);
  const pageErrors = [];
  page.on('pageerror', (error) => pageErrors.push(error.message));
  await page.goto(`${url}/projects/${project}`);
  await expect(page.locator('[data-kanban-column]')).toHaveCount(4, {
    timeout: 90000,
  });
  const grid = () => page.locator('[data-kanban-card]');
  const list = () => page.locator('[data-issue-list-row]');
  async function ids(locator) {
    return (
      await locator.evaluateAll((nodes) =>
        nodes.map((n) => n.dataset.kanbanCard ?? n.dataset.issueListRow)
      )
    ).sort();
  }
  const expected = { Active: [0, 1, 2], Archived: [3], All: [0, 1, 2, 3] };
  for (const [tab, indices] of Object.entries(expected)) {
    await page.getByRole('button', { name: tab, exact: true }).click();
    await expect(grid()).toHaveCount(indices.length);
    const boardIds = await ids(grid());
    assert.deepEqual(boardIds, indices.map((i) => issues[i].id).sort());
    await page
      .getByRole('button', { name: 'Switch to list view', exact: true })
      .click();
    await expect(list()).toHaveCount(indices.length);
    assert.deepEqual(await ids(list()), boardIds);
    // Target tab changes must preserve list mode.
    await page
      .getByRole('button', {
        name: tab === 'All' ? 'Active' : 'All',
        exact: true,
      })
      .click();
    await expect(
      page.getByRole('button', { name: 'Switch to kanban view', exact: true })
    ).toBeVisible();
    await page.getByRole('button', { name: tab, exact: true }).click();
    await page
      .getByRole('button', { name: 'Switch to kanban view', exact: true })
      .click();
    evidence.push(
      `PASS ${tab}: board/list parity, hidden Cancelled excluded, tab preserves display`
    );
  }
  await page.getByRole('button', { name: 'Active', exact: true }).click();
  await page
    .locator(`[data-kanban-card="${issues[0].id}"]`)
    .click({ modifiers: ['Control'] });
  await page
    .locator(`[data-kanban-card="${issues[1].id}"]`)
    .click({ modifiers: ['Control'] });
  await expect(page.getByText('2 selected', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Archived', exact: true }).click();
  await expect(page.getByText('2 selected', { exact: true })).not.toBeVisible();
  await page.getByRole('button', { name: 'Active', exact: true }).click();
  await page
    .locator(`[data-kanban-card="${issues[0].id}"]`)
    .click({ modifiers: ['Control'] });
  await page
    .locator(`[data-kanban-card="${issues[1].id}"]`)
    .click({ modifiers: ['Control'] });
  await expect(page.getByText('2 selected', { exact: true })).toBeVisible();
  await page.getByPlaceholder('Search issues...').fill('Archive fixture 1');
  await expect(grid()).toHaveCount(1);
  await expect(page.getByText('2 selected', { exact: true })).not.toBeVisible();
  await page.getByPlaceholder('Search issues...').fill('');
  await page.getByRole('button', { name: 'Archived', exact: true }).click();
  await page.getByRole('button', { name: 'Active', exact: true }).click();
  evidence.push('PASS tab and search changes prune selection and bulk counts');
  const card = page.locator(`[data-kanban-card="${issues[0].id}"]`);
  await card.hover();
  await card.getByRole('button', { name: 'More actions', exact: true }).click();
  await page.getByRole('option', { name: 'アーカイブ', exact: true }).click();
  const dialog = page
    .getByRole('dialog')
    .filter({ has: page.getByRole('heading', { name: 'Issueをアーカイブ' }) });
  await expect(dialog.getByRole('checkbox')).toBeChecked();
  await expect(dialog).toContainText('ワークツリーを物理削除');
  await dialog.getByRole('checkbox').uncheck();
  await expect(
    dialog.getByRole('button', { name: 'アーカイブ', exact: true })
  ).toBeEnabled();
  await dialog.getByRole('button', { name: 'アーカイブ', exact: true }).click();
  await expect(dialog).not.toBeVisible();
  const saved = (await api(`/api/board/issues/${issues[0].id}`)).data;
  assert.equal(saved.archived, true);
  assert.equal(saved.status_id, issues[0].status_id);
  // API/MCP-backed creation admission must reject before any Workspace/model is created.
  const denied = await fetch(base + '/api/workspaces/start', {
    method: 'POST',
    headers: { 'content-type': 'application/json', 'x-lvk-csrf': 'local' },
    body: JSON.stringify({
      mode: 'worktree',
      repos: [],
      linked_issue: { remote_project_id: project, issue_id: issues[0].id },
      executor_config: { executor: 'CODEX' },
      prompt: 'fixture must refuse before execution',
    }),
  });
  assert.equal(denied.status, 409, await denied.text());
  assert.equal((await api(`/api/workspaces/${ws}`)).data.archived, false);
  await page.getByRole('button', { name: 'Archived', exact: true }).click();
  await expect(grid()).toHaveCount(2);
  await page.locator(`[data-kanban-card="${issues[0].id}"]`).click();
  await expect(
    page.getByText('アーカイブ解除すると利用できます', { exact: true }).first()
  ).toBeVisible();
  await page.screenshot({ path: path.join(output, 'archived-issue-dark.png') });
  // Detail menu shares the restore action. List menu has the same action path.
  await page
    .getByRole('button', { name: 'Close panel', exact: true })
    .locator('..')
    .getByRole('button', { name: 'More actions', exact: true })
    .click();
  await page
    .getByRole('option', { name: 'アーカイブ解除', exact: true })
    .click();
  await expect
    .poll(
      async () => (await api(`/api/board/issues/${issues[0].id}`)).data.archived
    )
    .toBe(false);
  assert.equal((await api(`/api/workspaces/${ws}`)).data.archived, false);
  await page.getByRole('button', { name: 'Active', exact: true }).click();
  await page
    .getByRole('button', { name: 'Switch to list view', exact: true })
    .click();
  await page
    .locator(`[data-issue-list-row="${issues[0].id}"]`)
    .getByRole('button', { name: 'Issue actions' })
    .click();
  await page.getByRole('option', { name: 'アーカイブ', exact: true }).click();
  await expect(dialog.getByRole('checkbox')).toBeChecked();
  await dialog.getByRole('button', { name: 'キャンセル', exact: true }).click();
  evidence.push(
    'PASS UI confirmation defaults ON every time; OFF leaves child active; status preserved; disabled creation guidance; detail restore and list action'
  );
  await page
    .locator(`[data-issue-list-row="${issues[0].id}"]`)
    .getByRole('button', { name: 'Issue actions' })
    .click();
  await page.getByRole('option', { name: 'アーカイブ', exact: true }).click();
  await expect(dialog.getByRole('checkbox')).toBeChecked();
  await dialog.getByRole('checkbox').uncheck();
  const lostResponse = '**/api/board/issues/*/archive/confirm';
  let confirmationCalls = 0;
  await page.route(lostResponse, async (route) => {
    confirmationCalls++;
    await route.fetch();
    await route.abort('failed');
  });
  await expect(
    dialog.getByRole('button', { name: 'アーカイブ', exact: true })
  ).toBeEnabled();
  await dialog.getByRole('button', { name: 'アーカイブ', exact: true }).click();
  await expect(dialog.getByRole('alert')).toContainText('結果不明');
  await dialog.getByRole('button', { name: '結果を確認' }).click();
  await expect(dialog).not.toBeVisible();
  assert.equal(confirmationCalls, 1);
  await page.unroute(lostResponse);
  await page.getByRole('button', { name: 'Archived', exact: true }).click();
  await expect(list()).toHaveCount(2);
  await page
    .locator(`[data-issue-list-row="${issues[0].id}"]`)
    .getByRole('button', { name: 'Issue actions' })
    .click();
  await page
    .getByRole('option', { name: 'アーカイブ解除', exact: true })
    .click();
  await expect(list()).toHaveCount(1);
  evidence.push(
    'PASS lost confirmation response recovers the original receipt without another archive attempt'
  );
  // Search filters inside current target, never pulls active matches into Archived.
  await page.getByRole('button', { name: 'Archived', exact: true }).click();
  await page.getByPlaceholder('Search issues...').fill('Archive fixture 0');
  await expect(list()).toHaveCount(0);
  await page.getByPlaceholder('Search issues...').fill('');
  await page.reload();
  await expect(
    page.getByRole('button', { name: 'Switch to list view', exact: true })
  ).toBeVisible();
  await expect(grid()).toHaveCount(3);
  await page.getByRole('button', { name: 'Archived', exact: true }).click();
  await page
    .getByRole('button', { name: 'Switch to list view', exact: true })
    .click();
  await page.locator('button[aria-label="Reset target"]').click();
  await expect(
    page.getByRole('button', { name: 'Switch to list view', exact: true })
  ).toBeVisible();
  await page
    .locator('button[aria-label="Issue archive fixture"]')
    .click();
  await expect(grid()).toHaveCount(3);
  evidence.push(
    'PASS reload/project switch reset Active+Kanban; search restricted to selected target'
  );
  // A shown Cancelled status appears consistently in both displays.
  await api(`/api/local/v1/project_statuses/${statuses[4].id}`, 'PATCH', {
    hidden: false,
  });
  await page.reload();
  await expect(grid()).toHaveCount(4);
  await page
    .getByRole('button', { name: 'Switch to list view', exact: true })
    .click();
  await expect(list()).toHaveCount(4);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({ path: path.join(output, 'active-list-mobile.png') });
  assert.deepEqual(pageErrors, []);
  await writeFile(
    path.join(output, 'results.json'),
    JSON.stringify(evidence, null, 2)
  );
  console.log(evidence.join('\n'));
} catch (error) {
  if (page)
    await page
      .screenshot({ path: path.join(output, 'failure.png') })
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
  console.log('CLEANUP isolated backend/Vite/browser settled; fixture removed');
}
