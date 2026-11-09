// Isolated component/API fixtures only: never connects to a running LVK server.
import assert from 'node:assert/strict';
import { build } from 'esbuild';
import { chromium, expect } from '@playwright/test';
import { createServer } from 'node:http';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
const root = process.cwd();
const output = path.resolve(
  process.env.RESOURCE_EVIDENCE_DIR || 'test-results/codex-quota'
);
await mkdir(output, { recursive: true });
const scratch = await mkdtemp(path.join(tmpdir(), 'lvk-resources-'));
let browser, server;
const results = { cases: [], errors: [] };
const known = (value) => ({ status: 'known', value });
const fixture = (cpu = 42, ram = 42, disk = 42) => ({
  timestamp: new Date().toISOString(),
  system_visible: {
    memory_total_bytes: known(1000),
    memory_available_bytes: known(1000 - ram * 10),
    cpu_usage: { percent: known(cpu), sampling_duration_ms: 501 },
  },
  disks: [
    {
      target: 'worktree_base',
      observation: known({
        used_percent: disk,
        total_bytes: 1000e9,
        used_bytes: disk * 10e9,
        available_bytes: (100 - disk) * 5e9,
        filesystem_id: '202',
      }),
    },
  ],
});
try {
  const js = await build({
    entryPoints: ['tests/codex-quota/browser.fixture.tsx'],
    bundle: true,
    write: false,
    format: 'iife',
    jsx: 'automatic',
    alias: { '@': path.join(root, 'packages/web-core/src') },
    nodePaths: [path.join(root, 'packages/web-core/node_modules')],
  });
  execFileSync(
    path.join(root, 'packages/local-web/node_modules/.bin/tailwindcss'),
    [
      '-c',
      'tailwind.new.config.js',
      '--content',
      './src/**/*.{ts,tsx},../web-core/src/**/*.{ts,tsx},../ui/src/**/*.{ts,tsx},../../tests/codex-quota/*.tsx',
      '-i',
      '../web-core/src/app/styles/new/index.css',
      '-o',
      path.join(scratch, 'style.css'),
    ],
    { cwd: path.join(root, 'packages/local-web'), stdio: 'pipe' }
  );
  const css = await readFile(path.join(scratch, 'style.css'));
  server = createServer((req, res) => {
    if (req.url === '/app.js') {
      res.setHeader('Content-Type', 'text/javascript');
      res.end(js.outputFiles[0].text);
    } else if (req.url === '/style.css') {
      res.setHeader('Content-Type', 'text/css');
      res.end(css);
    } else {
      res.setHeader('Content-Type', 'text/html');
      res.end(
        '<!doctype html><html><head><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/style.css"></head><body><div id="root"></div><script src="/app.js"></script></body></html>'
      );
    }
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const base = `http://127.0.0.1:${server.address().port}`;
  browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({
    viewport: { width: 1024, height: 360 },
  });
  page.on('pageerror', (e) => results.errors.push(e.message));
  let requests = 0;
  let mode = 'fresh';
  await page.route('**/api/**', async (route) => {
    if (route.request().url().endsWith('/api/resource-observation'))
      return route.fulfill({ json: { success: true, data: fixture() } });
    requests++;
    assert.equal(route.request().method(), 'POST');
    const provider = route.request().postDataJSON().provider;
    assert.ok(['codex', 'claude_code'].includes(provider));
    assert.deepEqual(route.request().postDataJSON(), {
      provider,
      account_context: 'server_default',
      include_usage: false,
    });
    const observedNow = await page.evaluate(() => Date.now());
    const result =
      mode === 'unsupported'
        ? { status: 'unsupported' }
        : {
            quota: {
              status: 'fresh',
              observedAt: new Date(
                mode === 'stale' ? observedNow - 700000 : observedNow - 2000
              ).toISOString(),
              data: {
                rateLimits: {
                  primary: {
                    usedPercent: 17,
                    windowDurationMins: mode === 'readable' ? 300 : 73,
                    resetsAt:
                      Math.floor(observedNow / 1000) +
                      (mode === 'expired' ? -1 : 600),
                  },
                  secondary: {
                    usedPercent: 40,
                    windowDurationMins: mode === 'readable' ? 10080 : 10000,
                  },
                },
              },
            },
          };
    if (
      [
        'both-null',
        'primary-null',
        'secondary-null',
        'missing-period',
        'missing-time',
      ].includes(mode)
    ) {
      const bucket = result.quota.data.rateLimits;
      if (mode === 'both-null' || mode === 'primary-null')
        bucket.primary = null;
      if (mode === 'both-null' || mode === 'secondary-null')
        bucket.secondary = null;
      if (mode === 'missing-period') delete bucket.primary.windowDurationMins;
      if (mode === 'missing-time') delete bucket.primary.resetsAt;
    }
    if (mode === 'buckets') {
      result.quota.data.rateLimits.limitId = 'standard';
      result.quota.data.rateLimitsByLimitId = {
        standard: { limitId: 'standard', primary: { usedPercent: 99 } },
        other: {
          limitId: 'other',
          limitName: 'Public <model>',
          primary: { usedPercent: 25 },
        },
      };
    }
    if (mode === 'no-default') {
      result.quota.data.rateLimitsByLimitId = {
        other: {
          limitId: 'other',
          limitName: 'Public <model>',
          primary: { usedPercent: 25 },
        },
      };
      result.quota.data.rateLimits = null;
    }
    if (provider === 'claude_code' && result.quota) {
      const bucket = result.quota.data.rateLimits;
      const window = (value) =>
        value == null
          ? null
          : {
              utilization: value.usedPercent,
              resets_at:
                value.resetsAt == null
                  ? null
                  : new Date(value.resetsAt * 1000).toISOString(),
            };
      result.quota.data = {
        five_hour: window(bucket.primary),
        seven_day: window(bucket.secondary),
      };
    }
    if (provider === 'claude_code' && ['in_progress', 'cleanup_unconfirmed', 'timeout'].includes(mode)) {
      await route.fulfill({ json: { success: true, data: { result: {
        status: 'unavailable', code: mode, reason: 'Read did not complete', data: null,
      } } } });
      return;
    }
    await route.fulfill({
      status: mode === 'error' ? 503 : 200,
      json: { success: true, data: { result } },
    });
  });
  // Focused Claude cases use the real hook/component with an isolated HTTP fixture.
  for (const [code, text] of [
    ['in_progress', '取得中'],
    ['cleanup_unconfirmed', '回収を確認できない'],
    ['timeout', 'タイムアウト'],
  ]) {
    mode = code;
    await page.goto(`${base}/?sidebar&dual&lang=ja&width=300`);
    const panel = page.getByTestId('environment-quota-claude');
    await expect(panel.getByTestId('quota-reason')).toContainText(text);
    await expect(panel.getByRole('status')).toHaveText(code === 'in_progress' ? '取得中' : '取得不可');
    assert.equal(await panel.locator('dd').count(), 0);
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
    await page.screenshot({ path: path.join(output, `claude-${code}.png`) });
    results.cases.push(`Claude ${code}: reason visible, no invented quota`);
  }
  mode = 'fresh';
  for (const width of [320, 390, 768, 1280]) {
    requests = 0;
    mode = 'fresh';
    await page.setViewportSize({ width, height: 720 });
    await page.goto(base);
    const trigger = page.getByTestId('codex-quota');
    await expect(trigger).toHaveText('Codex残り 60%');
    assert.equal(requests, 1, 'duplicate subscribers share one request');
    if (width >= 1280) {
      const resourceBox = await page
        .getByTestId('resource-summary')
        .boundingBox();
      const quotaBox = await trigger.boundingBox();
      assert.ok(
        Math.abs(
          resourceBox.y +
            resourceBox.height / 2 -
            quotaBox.y -
            quotaBox.height / 2
        ) < 5 && quotaBox.x >= resourceBox.x + resourceBox.width,
        `desktop resources and quota share one row: ${JSON.stringify({ resourceBox, quotaBox })}`
      );
    }
    const quotaBounds = await trigger.boundingBox();
    for (const button of await page.locator('nav button').all()) {
      if ((await button.getAttribute('data-testid')) === 'codex-quota')
        continue;
      const b = await button.boundingBox();
      if (b)
        assert.ok(
          b.x + b.width <= quotaBounds.x ||
            b.x >= quotaBounds.x + quotaBounds.width ||
            b.y + b.height <= quotaBounds.y ||
            b.y >= quotaBounds.y + quotaBounds.height,
          'quota must not overlap navbar controls'
        );
    }
    await page.screenshot({ path: path.join(output, `summary-${width}.png`) });
    await trigger.focus();
    await page.keyboard.press('Enter');
    await expect(
      page.getByText('primary・73分', { exact: false })
    ).toBeVisible();
    await expect(
      page.getByText('secondary・10000分', { exact: false })
    ).toBeVisible();
    await expect(
      page.getByRole('button', { name: '更新（30秒間隔）', exact: true })
    ).toBeDisabled();
    assert.equal(requests, 1, 'manual refresh cooldown');
    await page.getByRole('dialog').focus();
    await page.waitForTimeout(200);
    await page.keyboard.press('Escape');
    await expect(trigger).toBeFocused();
    await trigger.click();
    await expect(page.getByText('Codex利用枠：取得済み')).toBeVisible();
    assert.equal(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth
      ),
      true
    );
    await page.waitForTimeout(200);
    await page.screenshot({ path: path.join(output, `width-${width}.png`) });
    results.cases.push(`layout, keyboard, tap, dedup ${width}`);
  }
  await page.evaluate(() => document.documentElement.classList.add('dark'));
  await page.getByTestId('codex-quota').click();
  await page.getByTestId('codex-quota').click();
  await page.waitForTimeout(200);
  await page.screenshot({ path: path.join(output, 'dark-details.png') });
  results.cases.push('dark details');
  for (const state of ['unsupported', 'stale', 'expired', 'error']) {
    mode = state;
    await page.goto(base);
    await expect(page.getByTestId('codex-quota')).toHaveText('Codex残り —');
    await page.getByTestId('codex-quota').click();
    await expect(
      page.getByText('Codex利用枠：', { exact: false })
    ).toContainText(
      state === 'unsupported'
        ? '非対応'
        : state === 'error'
          ? '取得失敗'
          : state === 'stale'
            ? '期限切れ'
            : 'リセット経過'
    );
    results.cases.push(state);
  }
  for (const state of ['buckets', 'no-default']) {
    mode = state;
    await page.goto(base);
    await expect(page.getByTestId('codex-quota')).toHaveText(
      state === 'buckets' ? 'Codex残り 60%' : 'Codex残り —'
    );
    await page.getByTestId('codex-quota').click();
    await expect(
      page.getByText('Public <model>', { exact: false })
    ).toBeVisible();
    assert.equal(await page.locator('[role="dialog"] model').count(), 0);
    if (state === 'buckets')
      await expect(page.getByText('(standard)', { exact: false })).toHaveCount(
        1
      );
    results.cases.push(state);
  }
  for (const [lang, title, duration] of [
    ['en', 'Codex left 60%', 'primary · 5 hours'],
    ['ja', 'Codex残り 60%', 'primary・5時間'],
  ]) {
    mode = 'readable';
    await page.goto(base + '/?lang=' + lang);
    await expect(page.getByTestId('codex-quota')).toHaveText(title);
    await page.getByTestId('codex-quota').click();
    await expect(page.getByText(duration, { exact: false })).toBeVisible();
    results.cases.push(`i18n ${lang}`);
  }
  for (const lang of ['en', 'ja']) {
    for (const dark of [false, true]) {
      for (const state of [
        'both-null',
        'primary-null',
        'secondary-null',
        'missing-period',
        'missing-time',
        'fresh',
      ]) {
        mode = state;
        await page.setViewportSize({ width: 320, height: 720 });
        await page.goto(base + '/?sidebar&lang=' + lang);
        await page.evaluate(
          (dark) => document.documentElement.classList.toggle('dark', dark),
          dark
        );
        const panel = page.getByTestId('environment-quota-codex');
        await expect(panel.locator('[data-quota-window]')).toHaveCount(
          state === 'both-null' ? 0 : state.endsWith('-null') ? 1 : 2
        );
        if (state !== 'both-null') {
          await expect(
            panel
              .getByText(lang === 'ja' ? '利用枠の残り' : 'Quota remaining', {
                exact: true,
              })
              .first()
          ).toBeVisible();
          await expect(
            panel
              .getByText(lang === 'ja' ? 'リセットまで' : 'Resets in', {
                exact: false,
              })
              .first()
          ).toBeVisible();
          const primary = panel.locator('[data-quota-window="primary"]');
          if (state !== 'primary-null') {
            await expect(primary.locator('dd')).toHaveText('83%');
            assert.equal(
              await primary
                .locator('[aria-hidden] > div')
                .evaluate((el) => el.style.width),
              '83%'
            );
            if (state === 'missing-period') {
              await expect(
                primary.getByText(
                  lang === 'ja' ? '期間不明' : 'Unknown period',
                  { exact: true }
                )
              ).toHaveCount(0);
              await expect(panel.locator('details')).toHaveCount(0);
            }
            if (state === 'missing-time')
              await expect(
                primary.getByLabel(
                  lang === 'ja' ? 'リセットまで: —' : 'Resets in: —',
                  { exact: true }
                )
              ).toBeVisible();
          }
        }
        const bounds = await panel.boundingBox();
        assert.equal(bounds.width, 300);
        const overflows = await panel
          .locator('[data-quota-window]')
          .evaluateAll(
            (cells) =>
              cells.filter((cell) => {
                const b = cell.getBoundingClientRect();
                return (
                  cell.scrollWidth > cell.clientWidth ||
                  [...cell.querySelectorAll('*')].some((el) => {
                    if (el.classList.contains('sr-only')) return false;
                    const r = el.getBoundingClientRect();
                    return r.left < b.left - 0.5 || r.right > b.right + 0.5;
                  })
                );
              }).length
          );
        assert.equal(overflows, 0, `${lang}/${state} cell contents fit`);
        if (state === 'missing-period')
          await page.screenshot({
            path: path.join(
              output,
              `sidebar-${lang}-${dark ? 'dark' : 'light'}.png`
            ),
          });
        results.cases.push(
          `300px sidebar ${lang} ${dark ? 'dark' : 'light'} ${state}`
        );
      }
    }
  }
  for (const lang of ['en', 'ja']) {
    for (const dark of [false, true]) {
      for (const state of ['readable', 'both-null', 'error']) {
        mode = state;
        const width = dark ? 280 : 300;
        await page.setViewportSize({ width: 320, height: 720 });
        await page.goto(`${base}/?sidebar&dual&lang=${lang}&width=${width}`);
        await expect(
          page.locator('[data-testid^="environment-quota-"]')
        ).toHaveCount(2);
        await page.evaluate(
          (dark) => document.documentElement.classList.toggle('dark', dark),
          dark
        );
        for (const provider of ['codex', 'claude']) {
          const panel = page.getByTestId(`environment-quota-${provider}`);
          await expect(panel.getByRole('status')).not.toBeEmpty();
          assert.equal((await panel.boundingBox()).width, width);
          assert.equal(
            await panel.evaluate((el) => el.scrollWidth > el.clientWidth),
            false
          );
          if (state === 'both-null')
            await expect(panel.getByRole('status')).toHaveText(
              lang === 'ja'
                ? '不明 · たった今取得'
                : 'Unknown · Fetched just now'
            );
          if (state === 'error')
            await expect(panel.getByRole('status')).toHaveText(
              lang === 'ja' ? '取得不可' : 'Unavailable'
            );
          if (state === 'readable') {
            await expect(panel.locator('dd').first()).toHaveText('83%');
            await expect(panel.getByRole('status')).toHaveText(
              lang === 'ja' ? 'たった今取得' : 'Fetched just now'
            );
            const cell = panel.locator('[data-quota-window]').first();
            const fonts = await cell.evaluate((el) =>
              [el.querySelector('dd'), el.querySelector('p')].map((node) => {
                const s = getComputedStyle(node);
                return [s.fontSize, s.fontWeight];
              })
            );
            assert.deepEqual(
              fonts[0],
              fonts[1],
              'remaining and countdown share numeric typography'
            );
          }
        }
        await page.screenshot({
          path: path.join(
            output,
            `pair-${lang}-${dark ? 'dark' : 'light'}-${state}.png`
          ),
        });
        results.cases.push(
          `paired visual ${lang} ${dark ? 'dark' : 'light'} ${state} ${width}px`
        );
      }
    }
  }
  for (const lang of ['ja', 'en']) {
    mode = 'readable';
    await page.goto(`${base}/?sidebar&lang=${lang}`);
    const compactPanel = page.getByTestId('environment-quota-codex');
    const period = lang === 'ja' ? '7日' : '7 days';
    await expect(
      compactPanel.getByText(period, { exact: false })
    ).not.toBeVisible();
    const compactBounds = await compactPanel.boundingBox();
    results.cases.push(
      `compact ${lang}: ${compactBounds.width}px wide, ${compactBounds.height}px high (2 windows)`
    );
    await page.screenshot({ path: path.join(output, `compact-${lang}.png`) });
    await expect(compactPanel.locator('details,summary')).toHaveCount(0);
    mode = 'fresh';
    for (const [age, ja, en] of [
      ['0', 'たった今取得', 'Fetched just now'],
      ['59999', 'たった今取得', 'Fetched just now'],
      ['60000', '1分前に取得', 'Fetched 1 min ago'],
      ['3599999', '59分前に取得', 'Fetched 59 min ago'],
      ['3600000', '1時間前に取得', 'Fetched 1 hr ago'],
      ['7200000', '2時間前に取得', 'Fetched 2 hr ago'],
      ['missing', '取得時刻不明', 'Fetch time unknown'],
      ['invalid', '取得時刻不明', 'Fetch time unknown'],
      ['-60000', '取得時刻不明', 'Fetch time unknown'],
    ]) {
      await page.goto(`${base}/?sidebar&lang=${lang}&age=${age}`);
      const panel = page.getByTestId('environment-quota-codex');
      await expect(panel.getByRole('status')).toHaveText(
        lang === 'ja' ? ja : en
      );
    }
    for (const [state, expected] of [
      ['stale', lang === 'ja' ? '期限切れ' : 'Stale'],
      ['error', lang === 'ja' ? '取得不可' : 'Unavailable'],
    ]) {
      await page.goto(`${base}/?sidebar&lang=${lang}&age=60000&state=${state}`);
      await expect(
        page.getByTestId('environment-quota-codex').getByRole('status')
      ).toHaveText(
        `${expected} · ${lang === 'ja' ? '1分前に取得' : 'Fetched 1 min ago'}`
      );
    }
    results.cases.push(
      `relative observation boundaries and stale/error ${lang}`
    );
  }
  mode = 'fresh';
  await page.goto(base);
  await expect(page.getByTestId('codex-quota')).toHaveText('Codex残り 60%');
  await page.clock.install();
  await page.evaluate(() => {
    Object.defineProperty(document, 'visibilityState', {
      configurable: true,
      value: 'hidden',
    });
    document.dispatchEvent(new Event('visibilitychange'));
  });
  const before = requests;
  await page.clock.fastForward(600001);
  assert.equal(requests, before);
  results.cases.push('hidden suppresses polling');
  await page.evaluate(() => {
    Object.defineProperty(document, 'visibilityState', {
      configurable: true,
      value: 'visible',
    });
    document.dispatchEvent(new Event('visibilitychange'));
  });
  await page.clock.runFor(1000);
  await expect.poll(() => requests).toBe(before + 1);
  await expect(page.getByTestId('codex-quota')).toHaveText('Codex残り 60%');
  await page.clock.runFor(31_000);
  await page.getByTestId('codex-quota').click();
  await page.getByRole('button', { name: '更新', exact: true }).click();
  await expect.poll(() => requests).toBe(before + 2);
  await expect(
    page.getByRole('button', { name: '更新（30秒間隔）', exact: true })
  ).toBeDisabled();
  await page.waitForTimeout(200);
  await page.clock.runFor(301_000);
  await expect.poll(() => requests).toBe(before + 3);
  results.cases.push('visible resume, manual refresh, five minute polling');
  assert.deepEqual(results.errors, []);
  results.status = 'passed';
} catch (error) {
  results.status = 'failed';
  results.error = error.stack;
  throw error;
} finally {
  await browser?.close();
  if (server) await new Promise((resolve) => server.close(resolve));
  await rm(scratch, { recursive: true, force: true });
  await writeFile(
    path.join(output, 'results.json'),
    JSON.stringify(results, null, 2)
  );
  console.log(JSON.stringify(results, null, 2));
}
