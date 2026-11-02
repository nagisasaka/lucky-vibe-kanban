import { createHash } from 'node:crypto';
import { afterEach, expect, it, vi } from 'vitest';
import {
  queuePushLocaleUpdate,
  synchronizeWebPushLocale,
} from './useWebPushLocale';
afterEach(() => vi.unstubAllGlobals());

it('updates only authenticated registrations matching this browser endpoint', async () => {
  const endpoint = 'https://fcm.googleapis.com/fcm/send/fixture';
  const hash = createHash('sha256').update(endpoint).digest('hex');
  vi.stubGlobal('window', { isSecureContext: true, Notification: {} });
  vi.stubGlobal('Notification', { permission: 'granted' });
  vi.stubGlobal('navigator', {
    serviceWorker: {
      getRegistration: async () => ({
        pushManager: { getSubscription: async () => ({ endpoint }) },
      }),
    },
  });
  const fetch = vi.fn(async (url: string) =>
    url === '/api/web-push'
      ? {
          ok: true,
          json: async () => ({
            registrations: [
              { id: 'one', endpoint_hash: hash },
              { id: 'two', endpoint_hash: 'other-device' },
              { id: 'three', endpoint_hash: hash },
            ],
          }),
        }
      : { ok: true }
  );
  vi.stubGlobal('fetch', fetch);
  await synchronizeWebPushLocale('ja-JP');
  expect(fetch.mock.calls.map((call) => call[0])).toEqual([
    '/api/web-push',
    '/api/web-push/one/locale',
    '/api/web-push/three/locale',
  ]);
  expect(fetch.mock.calls[1][1]).toMatchObject({
    method: 'POST',
    credentials: 'same-origin',
    body: '{"locale":"ja-JP"}',
  });
});

it('does not request permission, subscribe or fetch when permission is absent', async () => {
  const fetch = vi.fn();
  vi.stubGlobal('window', { isSecureContext: true, Notification: {} });
  vi.stubGlobal('navigator', { serviceWorker: {} });
  vi.stubGlobal('Notification', { permission: 'default' });
  vi.stubGlobal('fetch', fetch);
  await synchronizeWebPushLocale('ja');
  expect(fetch).not.toHaveBeenCalled();
});

it('settles old writes before new ones and recovers the queue after a failed request', async () => {
  const calls: string[] = [];
  let finish!: () => void;
  const pending = new Promise<void>((resolve) => {
    finish = resolve;
  });
  const first = queuePushLocaleUpdate(async () => {
    calls.push('old-start');
    await pending;
    calls.push('old-end');
  });
  const second = queuePushLocaleUpdate(async () => {
    calls.push('new');
  });
  await vi.waitFor(() => expect(calls).toEqual(['old-start']));
  finish();
  await Promise.all([first, second]);
  expect(calls).toEqual(['old-start', 'old-end', 'new']);
  await expect(
    queuePushLocaleUpdate(async () => {
      throw new Error('fixture');
    })
  ).rejects.toThrow('fixture');
  await queuePushLocaleUpdate(async () => {
    calls.push('recovered');
  });
  expect(calls.at(-1)).toBe('recovered');
});
