import { build } from "esbuild";
import { test } from "node:test";
import assert from "node:assert/strict";
const { outputFiles } = await build({
  entryPoints: ["packages/web-core/src/features/kanban/ui/kanbanDropSync.ts"],
  bundle: true,
  write: false,
  format: "esm",
});
const { KanbanDropSync } = await import(
  `data:text/javascript;base64,${Buffer.from(outputFiles[0].text).toString("base64")}`
);
const row = (rank, revision = rank + 10) => ({
  id: "a",
  status_id: "todo",
  sort_order: rank,
  updated_at: new Date(revision * 1000).toISOString(),
});
const drop = (rank, expected = 0) => ({
  id: "a",
  status_id: "todo",
  sort_order: rank,
  expected,
  crossColumn: false,
});
const deferred = () => {
  let resolve;
  const promise = new Promise((r) => (resolve = r));
  return { promise, resolve };
};
const setup = (save, read) => {
  const notices = [];
  let changes = 0,
    refreshes = 0;
  const sync = new KanbanDropSync(
    save,
    read,
    () => changes++,
    (n) => notices.push(n),
    () => refreshes++,
  );
  return { sync, notices, counts: () => ({ changes, refreshes }) };
};
test("HTTP success retains optimism until committed revision reaches subscription", async () => {
  const { sync } = setup(
    async () => {},
    async () => [row(2)],
  );
  await sync.enqueue(drop(2));
  assert.equal(sync.observe([row(0)]), true);
  assert.equal(sync.observe([row(2)]), false);
});
test("rapid same-card drops serialize writes and an older completion cannot retire latest intent", async () => {
  const gate = deferred(),
    calls = [];
  let current = row(0);
  const { sync } = setup(
    async (d) => {
      calls.push(d);
      if (calls.length === 1) await gate.promise;
      current = row(d.sort_order);
    },
    async () => [current],
  );
  const first = sync.enqueue(drop(1));
  await Promise.resolve();
  const second = sync.enqueue(drop(2, 1));
  assert.equal(calls.length, 1);
  gate.resolve();
  await first;
  assert.equal(sync.observe([row(1)]), true);
  assert.equal(sync.pending.get("a").sort_order, 2);
  await second;
  assert.equal(calls[1].expected, 1);
  assert.equal(sync.observe([row(2)]), false);
});
test("successful but guarded rank conflict is visible, then accepts canonical state", async () => {
  const { sync, notices } = setup(
    async () => {},
    async () => [row(-1, 20)],
  );
  await sync.enqueue(drop(3));
  assert.deepEqual(notices, ["conflict"]);
  assert.equal(sync.observe([row(0)]), true);
  assert.equal(sync.observe([row(-1, 20)]), false);
});
test("save failure notifies and releases optimism for resync", async () => {
  const { sync, notices, counts } = setup(
    async () => {
      throw Error("offline");
    },
    async () => {
      throw Error("must not read");
    },
  );
  await sync.enqueue(drop(3));
  assert.deepEqual(notices, ["save"]);
  assert.equal(sync.pending.size, 0);
  assert.equal(counts().refreshes, 1);
});
test("confirmation failure differs from save failure and can recover without repeating write", async () => {
  let reads = 0,
    writes = 0;
  const { sync, notices } = setup(
    async () => {
      writes++;
    },
    async () => {
      if (++reads === 1) throw Error("timeout");
      return [row(3)];
    },
  );
  await sync.enqueue(drop(3));
  assert.deepEqual(notices, ["sync"]);
  assert.equal(sync.observe([row(0)]), true);
  await sync.retry();
  assert.equal(sync.observe([row(3)]), false);
  assert.equal(writes, 1);
});
test("disposing ignores late completion", async () => {
  const gate = deferred();
  const { sync, notices, counts } = setup(
    async () => gate.promise,
    async () => [row(3)],
  );
  const done = sync.enqueue(drop(3));
  await Promise.resolve();
  sync.dispose();
  const before = counts();
  gate.resolve();
  await done;
  assert.deepEqual(counts(), before);
  assert.deepEqual(notices, []);
});
test("queued drop must not adopt an unseen promotion as its guard baseline", async () => {
  const gate = deferred(),
    calls = [];
  const { sync } = setup(
    async (d) => {
      calls.push(d);
      if (calls.length === 1) await gate.promise;
    },
    async () => [row(-10, 30)],
  );
  const first = sync.enqueue(drop(1));
  await Promise.resolve();
  const second = sync.enqueue(drop(2, 1));
  gate.resolve();
  await first;
  await second;
  assert.equal(
    calls[1].expected,
    1,
    "retain intended prior rank so server still protects promotion",
  );
});
test("old recovery read cannot acknowledge a write that was still in flight when it began", async () => {
  const saveGate = deferred(),
    retryGate = deferred(),
    confirmGate = deferred();
  let reads = 0;
  const { sync } = setup(
    async () => saveGate.promise,
    () => (++reads === 1 ? retryGate.promise : confirmGate.promise),
  );
  const saving = sync.enqueue(drop(3));
  await Promise.resolve();
  const recovering = sync.retry();
  saveGate.resolve();
  await Promise.resolve();
  await Promise.resolve();
  retryGate.resolve([row(0)]);
  await recovering;
  assert.equal(sync.observe([row(0)]), true);
  confirmGate.resolve([row(3)]);
  await saving;
  assert.equal(sync.observe([row(3)]), false);
});
test("explicit reconciliation releases a confirmed card deleted before subscription catches up", async () => {
  let reads = 0;
  const { sync, notices } = setup(
    async () => {},
    async () => (++reads === 1 ? [row(3)] : []),
  );
  await sync.enqueue(drop(3));
  assert.equal(sync.observe([]), true);
  await sync.retry();
  assert.equal(sync.pending.size, 0);
  assert.deepEqual(notices, ["conflict"]);
});
test("equal updated_at is insufficient when an activity rank or status is still stale", async () => {
  const confirmed = row(-1, 20);
  const { sync } = setup(
    async () => {},
    async () => [confirmed],
  );
  await sync.enqueue(drop(3));
  assert.equal(sync.observe([row(3, 20)]), true);
  assert.equal(sync.observe([{ ...confirmed, status_id: "old-column" }]), true);
  assert.equal(sync.observe([confirmed]), false);
});
test("leaving the board does not discard already queued drops", async () => {
  const writes = [];
  const { sync, counts } = setup(
    async (d) => {
      writes.push(d);
    },
    async () => {
      throw Error("disposed view must not confirm");
    },
  );
  const first = sync.enqueue(drop(1));
  const second = sync.enqueue(drop(2, 1));
  sync.dispose();
  const before = counts();
  await first;
  await second;
  assert.deepEqual(
    writes.map((d) => [d.sort_order, d.expected]),
    [
      [1, 0],
      [2, 1],
    ],
  );
  assert.deepEqual(counts(), before);
});
