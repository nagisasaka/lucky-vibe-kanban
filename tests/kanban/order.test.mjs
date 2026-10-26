import { build } from 'esbuild';
import { test } from 'node:test';
import assert from 'node:assert/strict';
const { outputFiles } = await build({ entryPoints: ['packages/web-core/src/features/kanban/ui/kanbanOrder.ts'], bundle: true, write: false, format: 'esm' });
const { dropRank, dropRankAfterActivity } = await import(`data:text/javascript;base64,${Buffer.from(outputFiles[0].text).toString('base64')}`);
const rank = (input) => dropRank({descending: false, promoted: false, destinationRanks: [], ...input});
test('manual insertion keeps every sibling rank and supports descending/end/empty', () => {
  assert.equal(rank({before: 10, after: 20}), 15);
  assert.equal(rank({after: 10}), 9);
  assert.equal(rank({before: 20}), 21);
  assert.equal(rank({}), 0);
  assert.equal(rank({descending: true, after: 20}), 21);
  assert.equal(rank({descending: true, before: 10}), 9);
});
test('activity received during drag is placed at destination head after drop', () => {
  const others = [-7, 10, 20];
  assert.equal(rank({before: 10, after: 20, promoted: true, destinationRanks: others}), -8);
  assert.equal(rank({descending: true, promoted: true, destinationRanks: others}), 21);
  assert.deepEqual(others, [-7, 10, 20]);
});

test('promoted neighbours do not invert frozen drop boundaries', () => {
  const startOrders = {A:0, B:1, C:2, D:3};
  const resolve = (currentOrders, ids = ['A','D','B','C'], index = 1, descending = false) => dropRankAfterActivity({ids,index,moved:'D',startOrders,currentOrders,descending,manual:true});
  const order = (current, rank) => Object.entries({...current,D:rank}).sort((a,b)=>a[1]-b[1]).map(([id])=>id);
  const one = {A:0,B:-1,C:2,D:3};
  assert.deepEqual(order(one, resolve(one)), ['B','A','D','C']);
  const multiple = {A:-2,B:-1,C:2,D:3};
  assert.deepEqual(order(multiple, resolve(multiple)), ['A','B','D','C']);
  assert.deepEqual(order(one, resolve(one,['D','A','B','C'],0)), ['B','D','A','C']);
  const dragged = {...one,D:-3};
  assert.deepEqual(order(dragged, resolve(dragged)), ['D','B','A','C']);
  const desc = dropRankAfterActivity({ids:['A','D','B','C'],index:1,moved:'D',startOrders:{A:3,B:2,C:1,D:0},currentOrders:{A:3,B:4,C:1,D:0},descending:true,manual:true});
  assert.equal(desc,2);
});

test('an event arriving from another column retains its prefix ahead of a top drop', () => {
  const rank = dropRankAfterActivity({ids:['D','A','B','C'],index:0,moved:'D',startOrders:{A:0,B:1,C:2,D:3,E:-10},currentOrders:{A:0,B:1,C:2,D:3,E:-1},descending:false,manual:true});
  assert.equal(rank,-0.5);
});
