import { test } from 'node:test';
import assert from 'node:assert/strict';
import { build } from 'esbuild';
const bundle = await build({
  entryPoints: ['packages/ui/src/lib/codexQuota.ts'],
  bundle: true,
  write: false,
  format: 'esm',
  platform: 'node',
});
const { quotaView, CODEX_STALE_MS } = await import(
  `data:text/javascript;base64,${Buffer.from(bundle.outputFiles[0].text).toString('base64')}`
);
const now = Date.now();
const fixture = () => ({
  result: {
    quota: {
      status: 'fresh',
      observedAt: new Date(now).toISOString(),
      data: {
        rateLimits: {
          primary: {
            usedPercent: 17,
            windowDurationMins: 73,
            resetsAt: now / 1000 + 600,
          },
          secondary: { usedPercent: 40, windowDurationMins: 10000 },
        },
      },
    },
  },
});
test('representative minimum, actual durations; no sum across buckets or credits', () => {
  const f = fixture();
  f.result.quota.data.rateLimitsByLimitId = {
    other: { primary: { usedPercent: 99 } },
  };
  f.result.quota.data.rateLimits.credits = { unlimited: true };
  const v = quotaView(f, false, now);
  assert.equal(v.remaining, 60);
  assert.deepEqual(v.rows[0].windows[0].period, { unit: 'minutes', count: 73 });
  assert.equal(v.rows[1].windows[0].remaining, 1);
});
test('unknown, unsupported, stale, errors and expired reset never become zero/full', () => {
  assert.equal(quotaView(undefined, false, now).remaining, null);
  assert.equal(
    quotaView({ result: { status: 'unsupported' } }, false, now).status,
    'unsupported'
  );
  assert.equal(quotaView(fixture(), true, now).remaining, null);
  assert.equal(
    quotaView(fixture(), false, now + CODEX_STALE_MS).remaining,
    null
  );
  const f = fixture();
  f.result.quota.data.rateLimits.primary.resetsAt = now / 1000;
  assert.equal(quotaView(f, false, now).remaining, null);
});
test('invalid percentages, missing observations and disallowed ordinary usage', () => {
  for (const used of [-1, 101, NaN, Infinity, null, '20']) {
    const f = fixture();
    f.result.quota.data.rateLimits.primary.usedPercent = used;
    assert.equal(quotaView(f, false, now).remaining, null);
  }
  for (const used of [0, 100]) {
    const f = fixture();
    f.result.quota.data.rateLimits.secondary = null;
    f.result.quota.data.rateLimits.primary.usedPercent = used;
    assert.equal(quotaView(f, false, now).remaining, 100 - used);
  }
  const f = fixture();
  f.result.quota.data.ordinaryUsageAllowed = false;
  assert.equal(quotaView(f, false, now).remaining, null);
  f.result.quota.observedAt = null;
  assert.equal(quotaView(f, false, now).remaining, null);
});

test('no default means unknown summary; deduplicate public limit IDs and distinguish buckets', () => {
  const f = fixture();
  const d = f.result.quota.data;
  d.rateLimits.limitId = 'standard';
  d.rateLimits.limitName = 'Standard';
  d.rateLimitsByLimitId = {
    standard: { limitId: 'standard', primary: { usedPercent: 90 } },
    other: {
      limitId: 'other',
      limitName: 'Public model',
      primary: { usedPercent: 10 },
    },
  };
  let v = quotaView(f, false, now);
  assert.equal(v.rows.length, 2);
  assert.equal(v.rows[1].id, 'other');
  assert.equal(v.rows[1].name, 'Public model');
  assert.equal(v.remaining, 60);
  d.rateLimits = null;
  v = quotaView(f, false, now);
  assert.equal(v.remaining, null);
  assert.equal(v.rows.length, 2);
  assert.ok(v.rows.every((r) => !r.representative));
});
test('readable durations and safe public labels; account fields are not projected', () => {
  const f = fixture();
  const b = f.result.quota.data.rateLimits;
  b.primary.windowDurationMins = 300;
  b.secondary.windowDurationMins = 10080;
  b.limitName = 'Public\u202e\nname';
  b.limitId = 'x'.repeat(200);
  b.accountId = 'private';
  const v = quotaView(f, false, now);
  assert.deepEqual(v.rows[0].windows[0].period, { unit: 'hours', count: 5 });
  assert.deepEqual(v.rows[0].windows[1].period, { unit: 'days', count: 7 });
  assert.equal(v.rows[0].name, 'Publicname');
  assert.equal(v.rows[0].id.length, 120);
  assert.ok(!JSON.stringify(v).includes('private'));
});

test('absent windows are omitted; missing duration preserves an existing window', () => {
  for (const missing of [null, undefined]) {
    const f = fixture();
    const bucket = f.result.quota.data.rateLimits;
    bucket.primary = missing;
    bucket.secondary = missing;
    let view = quotaView(f, false, now);
    assert.deepEqual(view.rows[0].windows, []);
    assert.equal(view.remaining, null);
    bucket.secondary = { usedPercent: 40 };
    view = quotaView(f, false, now);
    assert.deepEqual(view.rows[0].windows, [
      { key: 'secondary', period: null, remaining: 60, reset: null },
    ]);
    assert.equal(view.remaining, 60);
    bucket.primary = { usedPercent: 17, windowDurationMins: 300 };
    bucket.secondary = missing;
    view = quotaView(f, false, now);
    assert.deepEqual(
      view.rows[0].windows.map((w) => w.key),
      ['primary']
    );
    assert.equal(view.remaining, 83);
  }
  const f = fixture();
  f.result.quota.data.rateLimits.primary = {};
  const view = quotaView(f, false, now);
  assert.equal(view.rows[0].windows.length, 2);
  assert.equal(view.rows[0].windows[0].period, null);
  assert.equal(view.rows[0].windows[0].remaining, null);
  assert.equal(view.remaining, null);
});
