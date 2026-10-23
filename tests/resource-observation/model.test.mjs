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
      target: "lvk_data",
      observation: known({
        used_percent: 70,
        available_bytes: 200,
        filesystem_id: "1",
      }),
    },
    {
      target: "worktree_base",
      observation: known({
        used_percent: 80,
        available_bytes: 100,
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
test("CPU sample, available-based RAM and distinct filesystem values", () => {
  const view = resourceView(fixture(), false, now);
  assert.equal(view.cpu, 70);
  assert.equal(view.duration, 501);
  assert.equal(view.ram, 70);
  assert.deepEqual(
    view.disks.map((d) => [d.percent, d.available, d.filesystem]),
    [
      [70, 200, "1"],
      [80, 100, "2"],
    ],
  );
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
      assert.equal(v.disks[0].percent, null);
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
  data.disks[1].observation = { status: "unknown", error: "unreadable" };
  const v = resourceView(data, false, now);
  assert.equal(v.cpu, null);
  assert.equal(v.ram, null);
  assert.equal(v.disks[0].percent, 70);
  assert.equal(v.disks[1].percent, null);
  for (const bad of [null, -1, Infinity, NaN, "0"]) {
    data.system_visible.cpu_usage.percent = known(bad);
    assert.equal(resourceView(data, false, now).cpu, null);
  }
});
