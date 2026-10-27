import { isolatedEnvironment } from '../server/fixtures/issue-comments-env.mjs';
import { verifyBackendIdentity } from './backend-identity.mjs';
// Fresh SQLite and real API; source frontend via Vite. No production data/model calls.
import { chromium, expect } from '@playwright/test';
import { createServer } from 'vite';
import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { createWriteStream, createReadStream } from 'node:fs';
import { createHash } from 'node:crypto';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { once } from 'node:events';
import path from 'node:path';
const root = process.cwd();
const output = path.join(root, process.env.LVK_DND_BASELINE === '1' ? '.build/kanban-drop-baseline' : process.env.LVK_DND_REGRESSION === '1' ? '.build/kanban-drop-validation' : '.build/kanban-validation');
await mkdir(output, { recursive: true });
await mkdir(path.join(root, '.build/kanban-validation'), { recursive: true });
const instance = await mkdtemp(path.join(root, '.build/kanban-validation/instance-'));
const binary = process.env.LVK_TEST_SERVER;
assert.ok(binary, 'Set LVK_TEST_SERVER to a compatible prebuilt backend');
const env = isolatedEnvironment(process.env, instance, root);
const fixtureBin = path.join(instance, 'bin');
await mkdir(fixtureBin);
for (const name of ['codex', 'claude'])
  await writeFile(path.join(fixtureBin, name), '#!/bin/sh\nexit 1\n', { mode: 0o755 });
env.PATH = `${fixtureBin}:${env.PATH}`;
const server = spawn(binary, [], {
  cwd: root,
  env,
  stdio: ['ignore', 'pipe', 'pipe'],
});
server.stdout.pipe(createWriteStream(path.join(output, 'server.stdout.log')));
server.stderr.pipe(createWriteStream(path.join(output, 'server.stderr.log')));
let browser, page, vite, base;
let passed = false;
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
const evidence = [];
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
        kanbanContainerOverride: process.env.LVK_DND_BASELINE === '1' ? 'a3247dadd5b4385e07a7848fd0009aa07d8ffa71' : null,
        build: info.build_info,
      },
      null,
      2
    )
  );
  if (process.env.LVK_DND_REGRESSION === '1') {
    const hashes = {};
    for (const file of [
      binary,
      'crates/server/src/routes/local_remote.rs',
      'packages/web-core/src/features/kanban/ui/KanbanContainer.tsx',
      'packages/web-core/src/features/kanban/ui/kanbanDropSync.ts',
      'packages/web-core/src/shared/lib/remoteApi.ts',
    ]) {
      const hash = createHash('sha256');
      for await (const chunk of createReadStream(path.resolve(root, file))) hash.update(chunk);
      hashes[file] = hash.digest('hex');
    }
    await writeFile(path.join(output, 'tested-source-hashes.json'), JSON.stringify(hashes, null, 2));
  }
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
      key: "LOCAL",
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
  const workspaceIds = [];
  for (let i = 0; i < 2; i++) {
    const workspace = (
      await api('/api/workspaces', 'POST', { name: `Linked workspace ${i}` })
    ).data.id;
    workspaceIds.push(workspace);
    await api(`/api/workspaces/${workspace}/links`, 'POST', {
      project_id: project,
      issue_id: issues[0].id,
    });
  }
  execFileSync(
    'python3',
    [
      'tests/kanban/seed-diff-fixture.py',
      instance,
      JSON.stringify(workspaceIds),
    ],
    { cwd: root }
  );
  const summaries = (
    await api('/api/workspaces/summaries', 'POST', { archived: false })
  ).data.summaries;
  const changed = summaries.find((w) => w.workspace_id === workspaceIds[0]);
  const clean = summaries.find((w) => w.workspace_id === workspaceIds[1]);
  assert.deepEqual(
    [changed.files_changed, changed.lines_added, changed.lines_removed],
    [3, 3, 1]
  );
  assert.deepEqual(
    [clean.files_changed, clean.lines_added, clean.lines_removed],
    [0, 0, 0]
  );
  evidence.push({
    label:
      'Real summary API over isolated Git fixture: committed/uncommitted/untracked and clean',
    changed,
    clean,
  });
  process.env.BACKEND_PORT = new URL(base).port;
  process.chdir(path.join(root, 'packages/local-web'));
  vite = await createServer({
    configFile: path.join(root, 'packages/local-web/vite.config.ts'),
    plugins: process.env.LVK_DND_BASELINE === '1' ? [{
      name: 'original-kanban-drop-reproduction', enforce: 'pre',
      load(id) {
        if (id === path.join(root, 'packages/web-core/src/features/kanban/ui/KanbanContainer.tsx'))
          return execFileSync('git', ['show', 'a3247dadd5b4385e07a7848fd0009aa07d8ffa71:packages/web-core/src/features/kanban/ui/KanbanContainer.tsx'], { cwd: root, encoding: 'utf8' });
      },
    }] : [],
    server: { port: 0, host: '127.0.0.1', open: false },
  });
  await vite.listen();
  const url = `http://127.0.0.1:${vite.httpServer.address().port}`;
  browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
  const context = await browser.newContext({
    viewport: { width: 2560, height: 1440 },
    deviceScaleFactor: 1.5,
    hasTouch: true,
  });
  if (process.env.LVK_UI4_CONTINUOUS === '1')
    await context.tracing.start({ screenshots: true, snapshots: true, sources: true });
  page = await context.newPage();
  page.setDefaultTimeout(20000);
  page.on('pageerror', (e) => console.log('PAGEERROR', e.message));
  await page.goto(`${url}/projects/${project}`);
  const columns = page.locator('[data-kanban-column]');
  await expect(columns).toHaveCount(4, { timeout: 90000 });
  const card = (id) => page.locator(`[data-kanban-card="${id}"]`);
  await expect(card(issues[0].id)).toBeVisible();
  if (process.env.LVK_DND_REGRESSION === '1') {
    const { runDropRegression } = await import('./drop-regression.mjs');
    await runDropRegression({ page, columns, card, api, project, statuses, issues, output, evidence });
  } else {
  await expect(
    card(issues[0].id).getByText('3 files', { exact: true })
  ).toBeVisible();
  await expect(
    card(issues[0].id).getByText('0 files', { exact: true })
  ).toBeVisible();
  await expect(
    card(issues[0].id).getByText('+3', { exact: true })
  ).toBeVisible();
  await expect(
    card(issues[0].id).getByText('+0', { exact: true })
  ).toBeVisible();
  await expect(
    page.locator('[data-kanban-card]').getByText(/^Integration:/)
  ).toHaveCount(0);
  await expect(card(issues[0].id).locator('summary')).toContainText(
    'Workspaces: 2'
  );
  await expect(
    card(issues[0].id).getByTestId('issue-description-preview')
  ).toBeVisible();
  await expect(
    card(issues[0].id).getByText('Linked workspace 0', { exact: true })
  ).toBeVisible();
  await card(issues[0].id).locator('summary').click();
  await expect(
    card(issues[0].id).getByText('Linked workspace 0', { exact: true })
  ).toBeVisible();
  await expect(card(issues[0].id).locator('details p')).toContainText(
    'Specification body'
  );
  await expect(
    card(issues[0].id).getByTestId('issue-description-preview')
  ).toBeVisible();
  await page.screenshot({ path: path.join(output, 'expanded-2560.png') });
  await card(issues[0].id).locator('summary').click();
  const geometry = async (label) => {
    const data = await columns.evaluateAll((cols) =>
      cols.map((c) => ({
        width: c.getBoundingClientRect().width,
        columns: getComputedStyle(c).gridTemplateColumns.split(' ').length,
        cards: [...c.children].map((x) => ({
          id: x.dataset.kanbanCard,
          x: x.getBoundingClientRect().x,
          y: x.getBoundingClientRect().y,
          width: x.getBoundingClientRect().width,
          right: x.getBoundingClientRect().right,
          bottom: x.getBoundingClientRect().bottom,
        })),
      }))
    );
    for (const c of data) {
      assert.equal(c.columns, c.width >= 536 ? 2 : 1);
      for (const [i, card] of c.cards.entries()) {
        assert.ok(
          card.width >= (c.columns === 2 ? 256 : 240),
          `${label}: readable minimum width`
        );
        for (const other of c.cards.slice(i + 1))
          assert.ok(
            card.right <= other.x ||
              other.right <= card.x ||
              card.bottom <= other.y ||
              other.bottom <= card.y,
            `${label}: cards must not overlap`
          );
      }
      for (let i = 1; i < c.cards.length; i++) {
        assert.ok(c.cards[i].y >= c.cards[i - 1].y - 1);
        if (c.cards[i].y === c.cards[i - 1].y)
          assert.ok(c.cards[i].x > c.cards[i - 1].x);
      }
    }
    evidence.push({ label, data });
    console.log(
      'PASS',
      label,
      data.map((c) => [c.width, c.columns])
    );
  };
  await geometry('2560x1440 CSS, DPR 1.5 (3840x2160 at 150%)');
  await page.screenshot({ path: path.join(output, 'compact-2560.png') });
  // Resize each actual status container across the threshold in both directions.
  for (const width of [535, 536, 537, 536, 535]) {
    await columns.evaluateAll(
      (cols, width) =>
        cols.forEach((c) => (c.parentElement.style.width = `${width}px`)),
      width
    );
    await geometry(`column ${width}px`);
  }
  await columns.evaluateAll((cols) =>
    cols.forEach((c) => c.parentElement.style.removeProperty('width'))
  );
  const first = card(issues[0].id);
  await first.locator('summary').focus();
  await page.setViewportSize({ width: 1280, height: 900 });
  await geometry('1280x900');
  await expect(first.locator('summary')).toBeFocused();
  await page.screenshot({ path: path.join(output, 'compact-1280.png') });
  await page.setViewportSize({ width: 390, height: 844 });
  await geometry('390x844');
  await page.screenshot({ path: path.join(output, 'compact-390.png') });
  await page.setViewportSize({ width: 2560, height: 1440 });
  // Pointer status move to the empty Done column, using actual measured targets.
  const from = await first.boundingBox(),
    to = await columns.nth(3).boundingBox();
  await page.mouse.move(from.x + 30, from.y + 20);
  await page.mouse.down();
  await page.mouse.move(from.x + 45, from.y + 20, { steps: 5 });
  await page.mouse.move(to.x + 40, to.y + 40, { steps: 20 });
  await page.mouse.up();
  await expect(
    columns.nth(3).locator(`[data-kanban-card="${issues[0].id}"]`)
  ).toHaveCount(1);
  await expect
    .poll(
      async () =>
        (
          await api(`/api/local/v1/fallback/issues?project_id=${project}`)
        ).issues.find((i) => i.id === issues[0].id)?.status_id
    )
    .toBe(statuses[3].id);
  evidence.push({
    label: 'Pointer move to empty Done persists through real API',
  });
  await page.reload();
  await expect(
    columns.nth(3).locator(`[data-kanban-card="${issues[0].id}"]`)
  ).toHaveCount(1);

  // Keyboard row-major reorder and next-row movement, preserving focus.
  const ids = () =>
    columns
      .nth(0)
      .locator('[data-kanban-card]')
      .evaluateAll((cards) => cards.map((c) => c.dataset.kanbanCard));
  let order = await ids();
  const moving = order[0];
  await card(moving).focus();
  await page.keyboard.press('Space', { delay: 100 });
  await page.keyboard.press('ArrowRight', { delay: 100 });
  await page.keyboard.press('Space', { delay: 100 });
  await expect.poll(ids).toEqual([order[1], order[0], ...order.slice(2)]);
  await expect(card(moving)).toBeFocused();
  order = await ids();
  await page.keyboard.press('Space', { delay: 100 });
  await expect(card(moving)).toHaveAttribute('aria-pressed', 'true');
  await page.keyboard.press('ArrowDown', { delay: 100 });
  await page.keyboard.press('Space', { delay: 100 });
  await expect.poll(ids).not.toEqual(order);
  await expect(card(moving)).toBeFocused();
  evidence.push({
    label:
      'Keyboard Space/Right/Down/Space reorder across grid rows; focus retained',
  });
  // Escape cancels without moving.
  order = await ids();
  await page.keyboard.press('Space', { delay: 100 });
  await page.keyboard.press('ArrowLeft', { delay: 100 });
  await page.keyboard.press('Escape', { delay: 100 });
  assert.deepEqual(await ids(), order);
  // Pointer reordering uses the same row-major indices as the keyboard.
  const pointerSource = await card(order[0]).boundingBox();
  const pointerTarget = await card(order[3]).boundingBox();
  await page.mouse.move(pointerSource.x + 30, pointerSource.y + 20);
  await page.mouse.down();
  await page.mouse.move(pointerSource.x + 45, pointerSource.y + 20, {
    steps: 4,
  });
  await page.mouse.move(pointerTarget.x + 30, pointerTarget.y + 20, {
    steps: 15,
  });
  await page.mouse.up();
  const reordered = [...order.slice(1, 4), order[0], ...order.slice(4)];
  await expect.poll(ids).toEqual(reordered);
  await expect
    .poll(async () =>
      (await api(`/api/local/v1/fallback/issues?project_id=${project}`)).issues
        .filter((i) => i.status_id === statuses[0].id)
        .sort((a, b) => a.sort_order - b.sort_order)
        .map((i) => i.id)
    )
    .toEqual(reordered);
  evidence.push({ label: 'Pointer row-major reorder saved through real API' });
  // Keyboard cross-column move from Done back to In Review.
  await card(issues[0].id).focus();
  await page.keyboard.press('Space', { delay: 100 });
  await page.keyboard.press('ArrowLeft', { delay: 100 });
  await page.keyboard.press('Space', { delay: 100 });
  await expect(
    columns.nth(2).locator(`[data-kanban-card="${issues[0].id}"]`)
  ).toHaveCount(1);
  await expect(card(issues[0].id)).toBeFocused();
  evidence.push({
    label: 'Keyboard cross-column move and Escape cancellation',
  });
  // Fill Done after testing its empty drop target, then verify all four grids.
  for (let i = 0; i < 3; i++)
    await api('/api/local/v1/issues', 'POST', {
      project_id: project,
      status_id: statuses[3].id,
      title: `Done ${i}`,
      description: 'Done specification',
      sort_order: 4000 + i,
      extension_metadata: {},
    });
  await page.reload();
  await expect(columns.nth(3).locator('[data-kanban-card]')).toHaveCount(3, {
    timeout: 20000,
  });
  await geometry('all four statuses populated');
  // Actual application detail navigation stays separate from disclosure.
  await card(moving).focus();
  await page.keyboard.press('Enter', { delay: 100 });
  await expect(page).toHaveURL(new RegExp(moving));
  await page.goBack();
  await expect(columns).toHaveCount(4);
  // The source title typography is unchanged and long titles still ellipsize.
  const typography = await card(moving)
    .locator('span.text-base.truncate')
    .evaluate((el) => ({
      font: getComputedStyle(el).fontSize,
      overflow: getComputedStyle(el).textOverflow,
      truncated: el.scrollWidth > el.clientWidth,
    }));
  assert.deepEqual(typography, {
    font: '16px',
    overflow: 'ellipsis',
    truncated: true,
  });
  evidence.push({ label: 'Title typography', typography });
  await page
    .getByRole('button', { name: 'Integrate / Auto Merge', exact: true })
    .click();
  await expect(
    page.getByRole('heading', { name: 'Auto Merge', exact: true })
  ).toBeVisible();
  await page.screenshot({
    path: path.join(output, 'integration-navbar-entry.png'),
  });
  await page.keyboard.press('Escape');
  await expect(
    page.getByRole('heading', { name: 'Auto Merge', exact: true })
  ).toHaveCount(0);
  evidence.push({
    label:
      'Navbar Integration entry opens and closes existing panel; no card Integration lines',
  });
  // Fixture-only API response overlay: no agent/model execution. All other calls remain real.
  let phase = 0;
  await page.route('**/api/workspaces/summaries', async (route) => {
    const response = await route.fetch();
    const json = await response.json();
    json.data.summaries = json.data.summaries.map((w, i) => ({
      ...w,
      is_running: phase === 0,
      has_pending_approval: i === 0,
      has_unseen_turns: phase === 1,
      has_running_dev_server: i === 1,
      latest_process_status:
        phase === 0
          ? i === 0
            ? 'awaiting_approval'
            : 'running'
          : i === 0
            ? 'awaiting_input'
            : 'failed',
    }));
    await route.fulfill({ response, json });
  });
  await page.reload();
  const statusCard = card(issues[0].id);
  await expect(statusCard.locator('[aria-live]')).toHaveCount(0);
  await expect(
    statusCard.getByRole('img', { name: 'Approval required', exact: true }).first()
  ).toBeVisible();
  await expect(statusCard.getByRole('img', { name: 'Running', exact: true }).first()).toBeVisible();
  await expect(
    statusCard.getByRole('img', { name: 'Dev server running', exact: true }).first()
  ).toBeVisible();
  await statusCard.locator('summary').focus();
  await page.screenshot({
    path: path.join(output, 'fixture-running-approval.png'),
  });
  const before = await statusCard.boundingBox();
  phase = 1;
  await expect(
    statusCard.getByRole('img', { name: 'Input required', exact: true }).first()
  ).toBeVisible({ timeout: 30000 });
  await expect(statusCard.getByRole('img', { name: 'Failed', exact: true }).first()).toBeVisible();
  await expect(
    statusCard.getByRole('img', { name: 'Unseen activity', exact: true }).first()
  ).toBeVisible();
  await expect(statusCard.locator('summary')).toBeFocused();
  const after = await statusCard.boundingBox();
  assert.equal(after.x, before.x);
  assert.equal(after.y, before.y);
  await page.screenshot({ path: path.join(output, 'fixture-attention.png') });
  evidence.push({
    label:
      'Fixture summary overlay: running/approval/input/failure/unseen/dev server; update keeps focus and origin',
  });
  await page.setViewportSize({ width: 1280, height: 900 });
  await expect(
    statusCard.getByRole('img', { name: 'Approval required', exact: true }).first()
  ).toBeVisible();
  await expect(
    statusCard.getByRole('img', { name: 'Input required', exact: true }).first()
  ).toBeVisible();
  await page.screenshot({
    path: path.join(output, 'fixture-attention-1280.png'),
  });
  await page.setViewportSize({ width: 2560, height: 1440 });
  await page.emulateMedia({ colorScheme: 'light' });
  await api('/api/config', 'PUT', {
    ...(await api('/api/info')).data.config,
    theme: 'LIGHT',
    language: 'JA',
  });
  await page.reload();
  await expect(columns).toHaveCount(4);
  await expect(statusCard.getByText('承認待ち', { exact: true })).toBeVisible();
  await page.screenshot({ path: path.join(output, 'light-ja-2560.png') });
  }
  await writeFile(
    path.join(output, 'results.json'),
    JSON.stringify(evidence, null, 2)
  );
  passed = true;
  console.log('PASS real API browser checks');
} catch (error) {
  if (page)
    await page
      .screenshot({ path: path.join(output, 'failure.png') })
      .catch(() => {});
  throw error;
} finally {
  if (process.env.LVK_UI4_CONTINUOUS === '1' && page)
    await page.context().tracing.stop({ path: path.join(output, 'trace.zip') });
  await browser?.close();
  await vite?.close();
  if (server.exitCode === null && server.signalCode === null) {
    const exited = once(server, 'exit');
    server.kill('SIGTERM');
    const timer = setTimeout(() => server.kill('SIGKILL'), 10000);
    await exited;
    clearTimeout(timer);
  }
  if (passed && process.env.LVK_KEEP_TEST_INSTANCE === '1') {
    await writeFile(
      path.join(output, 'retained-instance.txt'),
      instance + '\n'
    );
    console.log(
      'Retained isolated fixture instance for locked preview; no test process remains'
    );
  } else {
    await rm(instance, { recursive: true, force: true });
  }
  process.chdir(root);
  console.log(
    'CLEANUP: browser, Vite, isolated backend stopped; instance removed unless explicitly retained after PASS'
  );
}
