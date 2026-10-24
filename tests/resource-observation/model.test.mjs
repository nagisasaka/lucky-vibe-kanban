import { test } from "node:test";
import assert from "node:assert/strict";
import { build } from "esbuild";
const bundle = await build({
  entryPoints: ["packages/ui/src/lib/resourceObservation.ts"],
  bundle: true,
  write: false,
  format: "esm",
  platform: "node",
});
const { resourceView, resourceTone, RESOURCE_STALE_MS } = await import(
  `data:text/javascript;base64,${Buffer.from(bundle.outputFiles[0].text).toString("base64")}`
);
const now = Date.now();
const known = (value) => ({ status: "known", value });
const fixture = () => ({
  timestamp: new Date(now).toISOString(),
  system_visible: {
    memory_total_bytes: known(1000),
    memory_available_bytes: known(300),
    cpu_usage: { percent: known(70), sampling_duration_ms: 501 },
  },
  disks: [
    {
      target: "worktree_base",
      observation: known({
        used_percent: 90,
        total_bytes: 1000e9,
        used_bytes: 900e9,
        available_bytes: 50e9,
        filesystem_id: "2",
      }),
    },
  ],
});
test("thresholds use exact 70/80, not rounded numbers", () => {
  for (const [n, color] of [
    [0, "success"],
    [69.99, "success"],
    [70, "orange-500"],
    [79.99, "orange-500"],
    [80, "error"],
    [100, "error"],
    [null, "low"],
  ])
    assert.equal(resourceTone(n), `text-${color}`);
});
test("CPU sample, available-based RAM and physical filesystem capacity", () => {
  const view = resourceView(fixture(), false, now);
  assert.equal(view.cpu, 70);
  assert.equal(view.duration, 501);
  assert.equal(view.ram, 70);
  assert.deepEqual(view.disk, { percent: 90, used: 900e9, total: 1000e9 });
});
test("failed, stale and absent observations cannot look like healthy zero", () => {
  for (const data of [undefined, {}, fixture()])
    for (const [fail, time] of [
      [true, now],
      [false, now + RESOURCE_STALE_MS],
    ]) {
      const v = resourceView(data, fail, time);
      assert.equal(v.cpu, null);
      assert.equal(v.ram, null);
      assert.equal(v.disk.percent, null);
    }
  assert.equal(
    resourceView(fixture(), false, now + RESOURCE_STALE_MS - 1).cpu,
    70,
  );
});
test("partial and malformed data retain good fields", () => {
  const data = fixture();
  data.system_visible.memory_available_bytes = known(1001);
  data.system_visible.cpu_usage.percent = known(101);
  data.disks[0].observation = { status: "unknown", error: "unreadable" };
  const v = resourceView(data, false, now);
  assert.equal(v.cpu, null);
  assert.equal(v.ram, null);
  assert.equal(v.disk.percent, null);
  for (const bad of [null, -1, Infinity, NaN, "0"]) {
    data.system_visible.cpu_usage.percent = known(bad);
    assert.equal(resourceView(data, false, now).cpu, null);
  }
});

test("disk rejects missing, duplicate, inconsistent and invalid capacity without harming CPU", () => {
  for (const patch of [
    { total_bytes: 0 },
    { used_bytes: -1 },
    { used_bytes: 1001e9 },
    { total_bytes: undefined },
    { used_bytes: NaN },
    { used_percent: 94.736842105 },
    { used_percent: undefined },
  ]) {
    const data = fixture();
    Object.assign(data.disks[0].observation.value, patch);
    const view = resourceView(data, false, now);
    assert.deepEqual(view.disk, { percent: null, used: null, total: null });
    assert.equal(view.cpu, 70);
  }
  for (const entries of [
    [],
    [fixture().disks[0], fixture().disks[0]],
    [{ target: "lvk_data", observation: fixture().disks[0].observation }],
  ]) {
    const data = fixture();
    data.disks = entries;
    assert.equal(resourceView(data, false, now).disk.percent, null);
  }
  for (const value of [0, 100]) {
    const data = fixture();
    Object.assign(data.disks[0].observation.value, {
      used_bytes: value * 10e9,
      used_percent: value,
    });
    assert.equal(resourceView(data, false, now).disk.percent, value);
  }
});
