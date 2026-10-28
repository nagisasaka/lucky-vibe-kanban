import { build } from "esbuild";
import { test } from "node:test";
import assert from "node:assert/strict";
const { outputFiles } = await build({
  entryPoints: ["packages/web-core/src/features/kanban/ui/kanbanProjection.ts"],
  bundle: true,
  write: false,
  format: "esm",
});
const { projectKanbanItems, kanbanInsertionIndex } = await import(
  `data:text/javascript;base64,${Buffer.from(outputFiles[0].text).toString("base64")}`
);
const rect = (left, top, width = 100, height = 80) => ({
  left,
  top,
  width,
  height,
});
const slots = {
  ids: ["a", "b", "c"],
  overId: "B",
  columns: 1,
  rects: new Map([
    ["a", rect(0, 0)],
    ["b", rect(0, 90)],
    ["c", rect(0, 180)],
  ]),
};
test("one-column leading/trailing halves and gaps share the drop slot", () => {
  assert.equal(kanbanInsertionIndex({ ...slots, point: { x: 30, y: 100 } }), 1);
  assert.equal(kanbanInsertionIndex({ ...slots, point: { x: 30, y: 160 } }), 2);
  assert.equal(kanbanInsertionIndex({ ...slots, point: { x: 30, y: 85 } }), 1);
  assert.equal(kanbanInsertionIndex({ ...slots, point: { x: 30, y: 500 } }), 3);
});
test("two-column order follows leading/trailing horizontal halves, including uneven heights", () => {
  const input = {
    ...slots,
    columns: 2,
    rects: new Map([
      ["a", rect(0, 0)],
      ["b", rect(110, 0, 100, 140)],
      ["c", rect(0, 150)],
    ]),
  };
  assert.equal(kanbanInsertionIndex({ ...input, point: { x: 120, y: 20 } }), 1);
  assert.equal(kanbanInsertionIndex({ ...input, point: { x: 200, y: 20 } }), 2);
  assert.equal(kanbanInsertionIndex({ ...input, point: { x: 20, y: 160 } }), 2);
});
test("empty and keyboard targets have deterministic slots", () => {
  assert.equal(
    kanbanInsertionIndex({ ...slots, ids: [], point: { x: 10, y: 10 } }),
    0,
  );
  assert.equal(kanbanInsertionIndex({ ...slots, overId: "b", point: null }), 1);
  assert.equal(kanbanInsertionIndex({ ...slots, point: null }), 3);
});
test("projection leaves canonical membership intact and renders the moved ID exactly once", () => {
  const original = { A: ["a", "b"], B: ["c", "d"], C: [] };
  const before = structuredClone(original);
  const next = projectKanbanItems(original, "a", {
    droppableId: "B",
    index: 1,
  });
  assert.deepEqual(original, before);
  assert.deepEqual(next, { A: ["b"], B: ["c", "a", "d"], C: [] });
  assert.equal(
    Object.values(next)
      .flat()
      .filter((id) => id === "a").length,
    1,
  );
  assert.deepEqual(
    projectKanbanItems(original, "a", { droppableId: "C", index: 0 }).C,
    ["a"],
  );
});
test("return to source is projected from canonical order, not a stale hover arrangement", () => {
  const original = { A: ["a", "b"], B: ["c", "d"] };
  assert.deepEqual(
    projectKanbanItems(original, "a", { droppableId: "A", index: 1 }),
    { A: ["b", "a"], B: ["c", "d"] },
  );
  assert.equal(
    projectKanbanItems(original, "missing", { droppableId: "B", index: 1 }),
    original,
  );
  assert.equal(
    projectKanbanItems(original, "a", { droppableId: "missing", index: 1 }),
    original,
  );
});
