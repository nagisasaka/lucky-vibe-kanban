// Fresh SQLite and real API; source frontend via Vite. No production data/model calls.
import { chromium, expect } from '@playwright/test';
import { createServer } from 'vite';
import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { createWriteStream } from 'node:fs';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { once } from 'node:events';
import path from 'node:path';
import { createServer as createTcpServer } from 'node:net';
const root = process.cwd();
const startedAt = new Date().toISOString();
const output = path.join(
  root,
  `.build/kanban-animation-${process.env.VITE_KANBAN_MOTION || 'crisp'}`
);
const binary = process.env.LVK_TEST_SERVER;
assert.ok(binary, 'Set LVK_TEST_SERVER to a compatible prebuilt backend');
await mkdir(output, { recursive: true });
const instance = await mkdtemp(path.join(output, 'instance-'));
const env = {
  ...process.env,
  LVK_INSTANCE_DIR: instance,
  HOST: '127.0.0.1',
  BACKEND_PORT: '0',
  PREVIEW_PROXY_PORT: '0',
  LVK_ACCESS_DEPLOYMENT: 'local',
  LVK_AUTH_MODE: 'local',
  DISABLE_WORKTREE_CLEANUP: '1',
  RUST_LOG: 'warn',
};
for (const key of [
  'LVK_PUBLIC_URL',
  'LVK_MCP_CONFIG',
  'VIBE_BACKEND_URL',
  'VK_ALLOWED_ORIGINS',
])
  delete env[key];
const server = spawn(binary, [], {
  cwd: root,
  env,
  stdio: ['ignore', 'pipe', 'pipe'],
});
server.stdout.pipe(createWriteStream(path.join(output, 'server.stdout.log')));
server.stderr.pipe(createWriteStream(path.join(output, 'server.stderr.log')));
let browser, page, vite, base, context;

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
        binary,
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
    for (let i = 0; i < (column === 3 ? 0 : 6); i++)
      issues.push(
        (
          await api('/api/local/v1/issues', 'POST', {
            project_id: project,
            status_id: status.id,
            title: `Card ${column}-${i} 長いタイトル ${'readable title '.repeat(10)}`,
            description: 'Specification body\n'.repeat((i % 3) + 1),
            sort_order: i * 1024,
            extension_metadata: {},
          })
        ).data
      );
  }
  // Diagnose the existing fractional-rank API issue separately from animation.
  // Spaced integer fixtures below still exercise real persistence, without
  // changing production sorting/API code or masking the rejected .5 request.
  const fractionalRequest = {
    updates: [{ id: issues[0].id, expected_sort_order: 0, sort_order: 0.5 }],
  };
  const probe = await fetch(base + '/api/local/v1/issues/bulk', {
    method: 'POST',
    redirect: 'error',
    headers: { 'content-type': 'application/json', 'x-lvk-csrf': 'local' },
    body: JSON.stringify(fractionalRequest),
  });
  evidence.push({
    label:
      'Existing fractional-rank API compatibility probe (not an animation acceptance)',
    request: fractionalRequest,
    status: probe.status,
    response: await probe.text(),
  });
  if (probe.ok)
    await api('/api/local/v1/issues/bulk', 'POST', {
      updates: [{ id: issues[0].id, sort_order: 0 }],
    });
  // A real linked Workspace gives the source card a different height.
  const workspace = (
    await api('/api/workspaces', 'POST', {
      name: 'Animation fixture workspace',
    })
  ).data.id;
  await api(`/api/workspaces/${workspace}/links`, 'POST', {
    project_id: project,
    issue_id: issues[0].id,
  });
  process.env.BACKEND_PORT = new URL(base).port;
  process.chdir(path.join(root, 'packages/local-web'));
  // Vite treats port=0 as its fixed default. Choose an OS-assigned free port
  // first; Vite's normal non-strict binding handles a reservation race.
  const reservation = createTcpServer();
  await new Promise((resolve, reject) => {
    reservation.once('error', reject);
    reservation.listen(0, '127.0.0.1', resolve);
  });
  const frontendPort = reservation.address().port;
  await new Promise((resolve, reject) =>
    reservation.close((error) => (error ? reject(error) : resolve()))
  );
  vite = await createServer({
    configFile: path.join(root, 'packages/local-web/vite.config.ts'),
    server: { port: frontendPort, host: '127.0.0.1', open: false },
  });
  await vite.listen();
  const url = `http://127.0.0.1:${vite.httpServer.address().port}`;
  browser = await chromium.launch({ headless: true, args: ['--no-sandbox'] });
  context = await browser.newContext({
    viewport: { width: 2560, height: 1440 },
  });
  await context.tracing.start({
    screenshots: true,
    snapshots: true,
    sources: true,
  });
  page = await context.newPage();
  page.setDefaultTimeout(20000);
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  page.on('response', (response) => {
    if (response.url().endsWith('/v1/issues/bulk')) {
      evidence.push({
        label: 'Real browser bulk update',
        status: response.status(),
        request: response.request().postData(),
      });
    }
  });
  await page.addInitScript(() => {
    window.__landings = [];
    const animate = Element.prototype.animate;
    Element.prototype.animate = function (keyframes, options) {
      if (this.querySelector('[data-kanban-ghost]')) {
        window.__landings.push({ keyframes, options });
      }
      return animate.call(this, keyframes, options);
    };
  });
  const preset = process.env.VITE_KANBAN_MOTION || 'crisp';
  const easing =
    preset === 'bouncy'
      ? 'cubic-bezier(0.34, 1.56, 0.64, 1)'
      : 'cubic-bezier(0.2, 0, 0, 1)';
  const card = (id) => page.locator(`[data-kanban-card="${id}"]`);
  const columns = page.locator('[data-kanban-column]');
  const ids = (column = 0) =>
    columns
      .nth(column)
      .locator('[data-kanban-card]')
      .evaluateAll((cards) => cards.map((c) => c.dataset.kanbanCard));
  const persisted = async () =>
    (await api(`/api/local/v1/fallback/issues?project_id=${project}`)).issues;
  async function checkSaved(column) {
    const displayed = await ids(column);
    await expect
      .poll(async () =>
        (await persisted())
          .filter((i) => i.status_id === statuses[column].id)
          .sort((a, b) => a.sort_order - b.sort_order)
          .map((i) => i.id)
      )
      .toEqual(displayed);
  }
  async function startPointer(id) {
    const box = await card(id).boundingBox();
    await page.mouse.move(box.x + 25, box.y + 20);
    await page.mouse.down();
    await page.mouse.move(box.x + 40, box.y + 20, { steps: 5 });
    await expect(page.locator('[data-kanban-ghost]')).toBeVisible();
  }
  async function checkLanding(before) {
    await expect
      .poll(() => page.evaluate(() => window.__landings.length))
      .toBeGreaterThan(before);
    const landing = await page.evaluate(() => window.__landings.at(-1));
    assert.equal(landing.options.duration, 200);
    assert.equal(landing.options.easing, easing);
    assert.notEqual(
      landing.keyframes[0].transform,
      landing.keyframes[1].transform
    );
    evidence.push({ label: 'Overlay landing Web Animation', landing });
    await expect(page.locator('[data-kanban-ghost]')).toHaveCount(0);
  }
  async function reloadBoard(width) {
    await page.reload();
    await expect(columns).toHaveCount(4);
    await expect(
      columns.nth(0).locator('[data-kanban-card]').first()
    ).toBeVisible();
    if (width)
      await columns.evaluateAll(
        (cols, w) =>
          cols.forEach((c) => {
            c.parentElement.style.width = `${w}px`;
          }),
        width
      );
  }
  await page.goto(`${url}/projects/${project}`);
  await expect(columns).toHaveCount(4, { timeout: 90000 });
  await expect(card(issues[0].id)).toBeVisible({ timeout: 30000 });
  const priority = card(issues[0].id).getByRole('button', {
    name: 'Priority',
    exact: true,
  });
  const control = await priority.boundingBox();
  await page.mouse.move(
    control.x + control.width / 2,
    control.y + control.height / 2
  );
  await page.mouse.down();
  await page.mouse.move(
    control.x + control.width / 2 + Math.min(8, control.width / 2 - 1),
    control.y + control.height / 2,
    { steps: 5 }
  );
  await expect(page.locator('[data-kanban-ghost]')).toHaveCount(0);
  await page.mouse.up();
  await page.keyboard.press('Escape');
  evidence.push({ label: 'Priority button does not start a drag' });
  await page.goto(`${url}/projects/${project}`);
  await expect(card(issues[0].id)).toBeVisible();
  await expect(page).toHaveURL(`${url}/projects/${project}`);
  await expect(
    card(issues[0].id).getByText('Animation fixture workspace', {
      exact: true,
    })
  ).toBeVisible();
  for (const width of [360, 600]) {
    // Resize the existing container, preserving the production layout strategy.
    await columns.evaluateAll(
      (cols, width) =>
        cols.forEach((c) => {
          c.parentElement.style.width = `${width}px`;
        }),
      width
    );
    assert.equal(
      await columns
        .nth(0)
        .evaluate(
          (c) => getComputedStyle(c).gridTemplateColumns.split(' ').length
        ),
      width === 600 ? 2 : 1
    );
    const order = await ids();
    console.log('CHECK', preset, width);
    const moving = order[0],
      target = order[3];
    const originalSize = await card(target).boundingBox();
    const destination = await card(target).boundingBox();
    await startPointer(moving);
    await page.mouse.move(destination.x + 35, destination.y + 30, {
      steps: 20,
    });
    await expect
      .poll(() => card(target).evaluate((c) => c.style.transform))
      .not.toBe('');
    const motion = await card(target).evaluate((c) => ({
      transform: c.style.transform,
      transition: c.style.transition,
      easing: getComputedStyle(c).transitionTimingFunction,
      duration: getComputedStyle(c).transitionDuration,
    }));
    assert.notEqual(motion.transform, 'translate3d(0px, 0px, 0)');
    assert.equal(motion.duration, '0.2s');
    assert.equal(motion.easing, easing);
    await page.waitForTimeout(250);
    const displaced = await card(target).boundingBox();
    assert.equal(
      displaced.width,
      originalSize.width,
      'translate must preserve width'
    );
    assert.equal(
      displaced.height,
      originalSize.height,
      'uneven cards must not scale'
    );
    assert.ok(
      Math.abs(displaced.x - originalSize.x) > 1 ||
        Math.abs(displaced.y - originalSize.y) > 1
    );
    await page.screenshot({
      path: path.join(output, `displacement-${width}.png`),
    });
    const overlap = await columns
      .nth(0)
      .locator('[data-kanban-card]')
      .evaluateAll((cards, moving) => {
        const rects = cards
          .filter((c) => c.dataset.kanbanCard !== moving)
          .map((c) => {
            const r = c.getBoundingClientRect();
            return {
              id: c.dataset.kanbanCard,
              x: r.x,
              y: r.y,
              right: r.right,
              bottom: r.bottom,
              height: r.height,
            };
          });
        return {
          heights: rects.map((r) => r.height),
          intersections: rects.flatMap((a, i) =>
            rects
              .slice(i + 1)
              .filter(
                (b) =>
                  Math.min(a.right, b.right) - Math.max(a.x, b.x) > 1 &&
                  Math.min(a.bottom, b.bottom) - Math.max(a.y, b.y) > 1
              )
              .map((b) => [a.id, b.id])
          ),
        };
      }, moving);
    evidence.push({
      label: `${width}px unequal-height displacement geometry (existing rectSortingStrategy)`,
      ...overlap,
    });
    const before = await page.evaluate(() => window.__landings.length);
    await page.mouse.up();
    await expect
      .poll(ids)
      .toEqual([order[1], order[2], order[3], order[0], ...order.slice(4)]);
    await checkLanding(before);
    await checkSaved(0);
    evidence.push({
      label: `${width}px pointer reorder and actual displacement`,
      motion,
      originalSize,
      displaced,
    });

    // Read the saved board in a new document before the next independent gesture.
    // The existing fallback collection refreshes every 30s; its stale snapshot
    // can overwrite rapid consecutive gestures. This test does not repair it.
    await reloadBoard(width);
    // Keyboard reorder and focus return, plus cancellation without persistence.
    let keyboardOrder = await ids();
    const keyboardId = keyboardOrder[0];
    await card(keyboardId).focus();
    await page.keyboard.press('Space', { delay: 100 });
    await page.keyboard.press(width === 600 ? 'ArrowRight' : 'ArrowDown', {
      delay: 100,
    });
    await page.keyboard.press('Space', { delay: 100 });
    await expect
      .poll(ids)
      .toEqual([keyboardOrder[1], keyboardOrder[0], ...keyboardOrder.slice(2)]);
    await expect(card(keyboardId)).toBeFocused();
    await expect(page.locator('[data-kanban-ghost]')).toHaveCount(0);
    await checkSaved(0);
    await reloadBoard(width);
    await card(keyboardId).focus();
    keyboardOrder = await ids();
    await page.keyboard.press('Space', { delay: 100 });
    await page.keyboard.press('ArrowDown', { delay: 100 });
    await page.keyboard.press('Escape', { delay: 100 });
    await expect(page.locator('[data-kanban-ghost]')).toHaveCount(0);
    assert.deepEqual(await ids(), keyboardOrder);
    await expect(card(keyboardId)).toBeFocused();
    await checkSaved(0);
    evidence.push({
      label: `${width}px keyboard reorder/focus and Escape cancellation`,
    });
  }
  // Cross-column destinations are unchanged; phase two displacement is excluded.
  await columns.evaluateAll((cols) =>
    cols.forEach((c) => c.parentElement.style.removeProperty('width'))
  );
  for (const column of [1, 3]) {
    const moving = (await ids())[0];
    const destination = await columns.nth(column).boundingBox();
    await startPointer(moving);
    await page.mouse.move(destination.x + 40, destination.y + 40, {
      steps: 25,
    });
    if (column === 3)
      await expect(columns.nth(column)).toHaveClass(/bg-brand\/5/);
    await page.screenshot({
      path: path.join(output, `cross-column-${column}.png`),
    });
    const before = await page.evaluate(() => window.__landings.length);
    await page.mouse.up();
    await expect(
      columns.nth(column).locator(`[data-kanban-card="${moving}"]`)
    ).toHaveCount(1);
    await checkLanding(before);
    await expect
      .poll(
        async () => (await persisted()).find((i) => i.id === moving).status_id
      )
      .toBe(statuses[column].id);
    await checkSaved(column);
    evidence.push({
      label: `Real API status move to ${column === 3 ? 'empty' : 'populated'} column`,
    });
    await reloadBoard();
  }
  await page.reload();
  await expect(columns.nth(3).locator('[data-kanban-card]')).toHaveCount(1);
  for (const column of [0, 1, 3]) await checkSaved(column);
  // Existing click-to-detail path remains usable after drag and overlay cleanup.
  const clickId = (await ids())[0];
  await card(clickId).click({ position: { x: 25, y: 20 } });
  await expect(page).toHaveURL(new RegExp(`/issues/${clickId}`));
  assert.deepEqual(errors, []);
  await writeFile(
    path.join(output, 'results.json'),
    JSON.stringify(
      {
        startedAt,
        endedAt: new Date().toISOString(),
        preset,
        evidence,
        errors,
        scope:
          'real application, fresh SQLite and real API; synthetic Issues/Workspace, no mocked responses',
      },
      null,
      2
    )
  );
  await rm(path.join(output, 'failure.png'), { force: true });
  console.log(
    `PASS ${preset}: real-app Chromium animation E2E (integer-rank fixtures; fractional-rank probe recorded separately)`
  );
} catch (error) {
  console.error('ANIMATION E2E FAILED', error);
  if (page)
    await page
      .screenshot({ path: path.join(output, 'failure.png') })
      .catch(() => {});
  throw error;
} finally {
  await context?.tracing
    .stop({ path: path.join(output, 'trace.zip') })
    .catch(() => {});
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
  console.log(
    'CLEANUP: Chromium, Vite, isolated backend stopped; temporary instance removed'
  );
}
