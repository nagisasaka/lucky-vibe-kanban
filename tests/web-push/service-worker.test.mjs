import test from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { readFileSync } from 'node:fs';

function fixture() {
  const handlers = {}, shown = [], opened = [], focused = [];
  const receipts = new Map();
  const target = 'https://lvk.example/projects/aa/issues/bb/workspaces/cc?session_id=dd';
  const self = {
    location: { origin: 'https://lvk.example' },
    addEventListener: (name, fn) => { handlers[name] = fn; },
    skipWaiting: async () => {},
    registration: { showNotification: async (...args) => { shown.push(args); } },
    clients: {
      claim: async () => {},
      matchAll: async () => [{ url: target, focus: async () => focused.push(target) }],
      openWindow: async (url) => opened.push(url),
    },
  };
  vm.runInNewContext(readFileSync(new URL('../../packages/public/lvk-push-sw.js', import.meta.url), 'utf8'), {
    self, URL, Request, Response,
    caches: { open: async () => ({
      match: async (key) => receipts.get(key.url),
      put: async (key, value) => receipts.set(key.url, value),
      keys: async () => [...receipts.keys()].map((url) => new Request(url)),
      delete: async (key) => receipts.delete(key.url),
    }) },
  });
  const emit = async (name, event) => {
    let promise; handlers[name]({ ...event, waitUntil: (p) => { promise = p; } }); await promise;
  };
  return { shown, opened, focused, target, emit };
}
test('closed-tab push displays minimal text once across duplicate delivery', async () => {
  const f = fixture();
  const data = { title: 'LVK', body: 'Codex needs an answer.', tag: 'lvk:1', url: f.target };
  await Promise.all([f.emit('push', { data: { json: () => data } }), f.emit('push', { data: { json: () => data } })]);
  assert.equal(f.shown.length, 1);
  assert.equal(f.shown[0][1].renotify, false);
  assert.equal(f.opened.length, 0); // Push never opens/answers/approves on its own.
});
test('click focuses exact target or opens authenticated SPA path; external URL is rejected', async () => {
  const f = fixture();
  const click = (url) => f.emit('notificationclick', { notification: { close() {}, data: { url } } });
  await click(f.target);
  assert.equal(f.focused.length, 1);
  await click(f.target + '&agent_run_id=ee');
  assert.equal(f.opened.length, 1);
  await click('https://attacker.example/');
  assert.equal(f.opened.length, 1);
});
test('push refuses foreign origins, arbitrary paths and malformed tags', async () => {
  const f = fixture();
  for (const [url, tag] of [['https://evil.example/projects/aa/issues/bb/workspaces/cc', 'lvk:1'], ['/api/approvals/x/respond', 'lvk:2'], [f.target, 'bad']]) {
    await f.emit('push', { data: { json: () => ({ title: 'LVK', body: 'x', tag, url }) } });
  }
  assert.equal(f.shown.length, 0);
});
test('installed web app and Service Worker share the root scope', () => {
  const manifest=JSON.parse(readFileSync(new URL('../../packages/public/site.webmanifest',import.meta.url),'utf8'));
  assert.equal(manifest.scope,'/');
  assert.equal(manifest.start_url,'/');
  assert.equal(manifest.id,'/');
  assert.equal(manifest.display,'standalone');
});
