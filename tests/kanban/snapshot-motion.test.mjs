import { build } from "esbuild";
import { test } from "node:test";
import assert from "node:assert/strict";
const { outputFiles } = await build({
  entryPoints: [
    "packages/web-core/src/shared/lib/electric/snapshotActivity.ts",
  ],
  bundle: true,
  write: false,
  format: "esm",
});
const { SnapshotActivity, SnapshotMotionGate } = await import(
  `data:text/javascript;base64,${Buffer.from(outputFiles[0].text).toString("base64")}`
);
test("cache is not fresh; first snapshot establishes baseline, later snapshots animate", () => {
  const activity = new SnapshotActivity(),
    gate = new SnapshotMotionGate();
  assert.equal(gate.allows(activity.state), false);
  activity.report(1, true);
  gate.observe(activity.state);
  assert.equal(gate.allows(activity.state), false);
  activity.report(2, true);
  gate.observe(activity.state);
  assert.equal(gate.allows(activity.state), true);
});
test("return from hidden/offline rejects pre-return requests and recovery baseline", () => {
  const activity = new SnapshotActivity(),
    gate = new SnapshotMotionGate();
  activity.report(1, true);
  gate.observe(activity.state);
  gate.suspend(10);
  activity.report(9, true);
  gate.observe(activity.state);
  assert.equal(gate.allows(activity.state), false);
  activity.report(11, true);
  gate.observe(activity.state);
  assert.equal(gate.allows(activity.state), false);
  activity.report(12, true);
  gate.observe(activity.state);
  assert.equal(gate.allows(activity.state), true);
  activity.report(13, false);
  gate.observe(activity.state);
  assert.equal(gate.allows(activity.state), false);
  activity.report(14, true);
  gate.observe(activity.state);
  assert.equal(gate.allows(activity.state), false);
  activity.report(15, true);
  gate.observe(activity.state);
  assert.equal(gate.allows(activity.state), true);
});
test("activity subscribers unsubscribe and failures are observable without row changes", () => {
  const activity = new SnapshotActivity();
  let calls = 0;
  const stop = activity.subscribe(() => calls++);
  activity.report(1, false);
  assert.equal(calls, 1);
  stop();
  activity.report(2, true);
  assert.equal(calls, 1);
});
