/* No fetch handler: authenticated pages and API responses are never cached. */
self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', (event) => event.waitUntil(self.clients.claim()));
let pending = Promise.resolve();
self.addEventListener('push', (event) => {
  pending = pending.catch(() => {}).then(async () => {
    if (!event.data) return;
    const data = event.data.json();
    if (data.title !== 'LVK' || !/^lvk:\d+$/.test(data.tag)) return;
    const url = new URL(data.url, self.location.origin);
    if (url.origin !== self.location.origin || !/^\/projects\/[0-9a-f-]+\/issues\/[0-9a-f-]+\/workspaces\/[0-9a-f-]+$/.test(url.pathname)) return;
    const cache = await caches.open('lvk-push-receipts-v1');
    const key = new Request(`${self.location.origin}/__lvk_push_receipt__/${data.tag}`);
    if (await cache.match(key)) return;
    await self.registration.showNotification('LVK', {
      body: data.body, tag: data.tag, renotify: false,
      data: { url: url.href }, icon: '/apple-touch-icon.png',
    });
    await cache.put(key, new Response('seen'));
    const keys = await cache.keys();
    for (const old of keys.slice(0, Math.max(0, keys.length - 256))) await cache.delete(old);
  });
  event.waitUntil(pending);
});
self.addEventListener('notificationclick', (event) => {
  event.notification.close();
  event.waitUntil((async () => {
    const url = new URL(event.notification.data?.url || '/', self.location.origin);
    if (url.origin !== self.location.origin) return;
    const windows = await self.clients.matchAll({ type: 'window', includeUncontrolled: true });
    const existing = windows.find((client) => client.url === url.href);
    if (existing) await existing.focus();
    else await self.clients.openWindow(url.href);
  })());
});
