import { useCallback, useEffect, useState } from 'react';
import { currentLocalUserId } from '@/shared/lib/localIdentity';

type Registration = { id: string; project_id: string; endpoint_hash: string };
type Status = {
  public_key: string | null;
  registrations: Registration[];
  projects: { id: string; name: string }[];
};
async function request<T>(path = '', body?: unknown): Promise<T> {
  const response = await fetch(`/api/web-push${path}`, {
    method: body === undefined ? 'GET' : 'POST',
    headers: { 'Content-Type': 'application/json' },
    credentials: 'same-origin',
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  });
  if (!response.ok) throw new Error(`Web Push: HTTP ${response.status}`);
  return response.status === 204 ? (undefined as T) : response.json();
}
function decodeKey(value: string): Uint8Array<ArrayBuffer> {
  return Uint8Array.from(
    atob(value.replace(/-/g, '+').replace(/_/g, '/')),
    (c) => c.charCodeAt(0)
  );
}
async function endpointHash(endpoint: string) {
  return Array.from(
    new Uint8Array(
      await crypto.subtle.digest('SHA-256', new TextEncoder().encode(endpoint))
    )
  )
    .map((n) => n.toString(16).padStart(2, '0'))
    .join('');
}

export function WebPushSettings() {
  const supported =
    window.isSecureContext &&
    'serviceWorker' in navigator &&
    'PushManager' in window &&
    'Notification' in window;
  const [status, setStatus] = useState<Status>();
  const [project, setProject] = useState('');
  const [hash, setHash] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const refresh = useCallback(async () => {
    const next = await request<Status>();
    setStatus(next);
    setProject((p) => p || next.projects[0]?.id || '');
    if (supported) {
      const registration = await navigator.serviceWorker.getRegistration('/');
      const sub = await registration?.pushManager.getSubscription();
      setHash(sub ? await endpointHash(sub.endpoint) : '');
    }
  }, [supported]);
  useEffect(() => {
    void refresh().catch((e: Error) => setError(e.message));
  }, [refresh]);
  const enable = async () => {
    setBusy(true);
    setError('');
    try {
      // Must precede awaits that lose the user gesture on Safari.
      const permission = await Notification.requestPermission();
      if (permission !== 'granted')
        throw new Error(
          '通知が許可されていません。ブラウザのサイト設定から変更できます。'
        );
      await navigator.serviceWorker.register('/lvk-push-sw.js', {
        scope: '/',
        updateViaCache: 'none',
      });
      const registration = await navigator.serviceWorker.ready;
      let subscription = await registration.pushManager.getSubscription();
      const owner = currentLocalUserId();
      const ownerKey = 'lvk:web-push-owner';
      const key = decodeKey(status!.public_key!);
      const sameKey =
        subscription?.options.applicationServerKey &&
        Array.from(
          new Uint8Array(subscription.options.applicationServerKey)
        ).join(',') === Array.from(key).join(',');
      // A shared browser/account change or VAPID rotation needs fresh consent.
      if (
        subscription &&
        (localStorage.getItem(ownerKey) !== owner || !sameKey)
      ) {
        await subscription.unsubscribe();
        subscription = null;
      }
      subscription ??= await registration.pushManager.subscribe({
        userVisibleOnly: true,
        applicationServerKey: key,
      });
      await request('', {
        project_id: project,
        subscription: subscription.toJSON(),
      });
      localStorage.setItem(ownerKey, owner);
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  const disable = async (item: Registration) => {
    setBusy(true);
    setError('');
    try {
      // Server removal first; a failed API call must not pretend delivery stopped.
      await request(`/${item.id}`, {});
      const next = await request<Status>();
      if (
        item.endpoint_hash === hash &&
        !next.registrations.some((r) => r.endpoint_hash === hash)
      ) {
        const registration = await navigator.serviceWorker.getRegistration('/');
        await (
          await registration?.pushManager.getSubscription()
        )?.unsubscribe();
      }
      await refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="space-y-base">
      <p className="text-high">ブラウザのWeb Push通知</p>
      <p className="text-sm text-low">
        Projectごとに、この端末へCodexの質問・承認待ち・Run終了を通知します。Issue名や質問本文は通知に表示しません。通知を拒否してもLVKは使えます。
      </p>
      {!supported ? (
        <p>
          この環境では利用できません。HTTPSの対応ブラウザを使用してください。
        </p>
      ) : !status?.public_key ? (
        <p>サーバーのWeb Push設定が必要です。</p>
      ) : (
        <div className="flex flex-wrap items-center gap-base">
          <select
            aria-label="通知するProject"
            value={project}
            onChange={(e) => setProject(e.target.value)}
            className="rounded border bg-primary p-half"
          >
            {status.projects.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
          <button
            type="button"
            disabled={busy || !project}
            onClick={() => void enable()}
            className="rounded border bg-secondary text-high px-base py-half disabled:opacity-50"
          >
            この端末でON / 再登録
          </button>
        </div>
      )}
      {status?.registrations.map((r) => (
        <div
          key={r.id}
          className="flex flex-wrap items-center gap-base text-sm"
        >
          <span>
            {status.projects.find((p) => p.id === r.project_id)?.name ??
              r.project_id}{' '}
            —{' '}
            {r.endpoint_hash === hash
              ? 'この端末'
              : `登録端末 ${r.id.slice(0, 8)}`}
          </span>
          <button
            type="button"
            disabled={busy}
            onClick={() => void disable(r)}
            className="rounded border bg-secondary text-high px-base py-half"
          >
            OFF
          </button>
        </div>
      ))}
      <p className="text-sm text-low">
        iPhone/iPadはホーム画面に追加したWebアプリが必要です。省電力・集中モード・ブラウザ終了などで通知が遅延/不達になる場合があります。ログアウト後も通知登録は続きます。停止はOFF、詳細を見るにはログインが必要です。登録は90日で期限切れになります。
      </p>
      {error && (
        <p role="alert" className="text-error">
          {error}
        </p>
      )}
    </div>
  );
}
