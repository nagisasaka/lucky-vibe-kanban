import { useTranslation } from 'react-i18next';
import { useCallback, useEffect, useState } from 'react';
import { currentLocalUserId } from '@/shared/lib/localIdentity';

type Registration = {
  id: string;
  project_id: string;
  endpoint_hash: string;
  event_mask: number;
};
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
  const { t, i18n } = useTranslation('settings');
  const pushEvents = [
    {
      bit: 1,
      label: t('notifications.web.question.label'),
      description: t('notifications.web.question.description'),
    },
    {
      bit: 2,
      label: t('notifications.web.approval.label'),
      description: t('notifications.web.approval.description'),
    },
    {
      bit: 4,
      label: t('notifications.web.success.label'),
      description: t('notifications.web.success.description'),
    },
    {
      bit: 8,
      label: t('notifications.web.failure.label'),
      description: t('notifications.web.failure.description'),
    },
    {
      bit: 16,
      label: t('notifications.web.canceled.label'),
      description: t('notifications.web.canceled.description'),
    },
  ];

  const supported =
    window.isSecureContext &&
    'serviceWorker' in navigator &&
    'PushManager' in window &&
    'Notification' in window;
  const [status, setStatus] = useState<Status>();
  const [project, setProject] = useState('');
  const [hash, setHash] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<
    'loadFailed' | 'actionUnconfirmed' | 'permissionDenied' | null
  >(null);
  const [errorDetail, setErrorDetail] = useState<string | null>(null);
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
    void refresh().catch((error) => {
      setErrorDetail(error instanceof Error ? error.message : String(error));
      setError('loadFailed');
    });
  }, [refresh]);
  const enable = async () => {
    setBusy(true);
    setError(null);
    setErrorDetail(null);
    try {
      // Must precede awaits that lose the user gesture on Safari.
      const permission = await Notification.requestPermission();
      if (permission !== 'granted') {
        setError('permissionDenied');
        return;
      }
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
        locale: i18n.resolvedLanguage ?? i18n.language,
        subscription: subscription.toJSON(),
      });
      localStorage.setItem(ownerKey, owner);
      await refresh();
    } catch (e) {
      setErrorDetail(e instanceof Error ? e.message : String(e));
      setError('actionUnconfirmed');
    } finally {
      setBusy(false);
    }
  };
  const disable = async (item: Registration) => {
    setBusy(true);
    setError(null);
    setErrorDetail(null);
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
      setErrorDetail(e instanceof Error ? e.message : String(e));
      setError('actionUnconfirmed');
    } finally {
      setBusy(false);
    }
  };
  const saveEvents = async (item: Registration, event_mask: number) => {
    setBusy(true);
    setError(null);
    setErrorDetail(null);
    try {
      await request(`/${item.id}/events`, { event_mask });
      await refresh();
    } catch (e) {
      setErrorDetail(e instanceof Error ? e.message : String(e));
      setError('actionUnconfirmed');
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="space-y-base">
      <p className="text-high">{t('notifications.web.title')}</p>
      <p className="text-sm text-low">{t('notifications.web.scope')}</p>
      {!supported ? (
        <p>{t('notifications.web.unsupported')}</p>
      ) : !status?.public_key ? (
        <p>{t('notifications.web.serverRequired')}</p>
      ) : (
        <div className="flex flex-wrap items-center gap-base">
          <select
            aria-label={t('notifications.web.project')}
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
            {t('notifications.web.enable')}
          </button>
        </div>
      )}
      {status?.registrations.map((r) => (
        <div key={r.id} className="space-y-base rounded border p-base text-sm">
          <span>
            {status.projects.find((p) => p.id === r.project_id)?.name ??
              r.project_id}{' '}
            —{' '}
            {r.endpoint_hash === hash
              ? t('notifications.web.thisDevice')
              : t('notifications.web.registeredDevice', {
                  id: r.id.slice(0, 8),
                })}
          </span>
          <button
            type="button"
            disabled={busy}
            onClick={() => void disable(r)}
            className="rounded border bg-secondary text-high px-base py-half"
          >
            {t('notifications.web.disable')}
          </button>
          <fieldset className="space-y-base" disabled={busy}>
            <legend className="text-high">
              {t('notifications.web.events')}
            </legend>
            {pushEvents.map((event) => (
              <label key={event.bit} className="flex items-start gap-base">
                <input
                  type="checkbox"
                  className="mt-half shrink-0"
                  checked={(r.event_mask & event.bit) !== 0}
                  onChange={(e) =>
                    void saveEvents(
                      r,
                      e.target.checked
                        ? r.event_mask | event.bit
                        : r.event_mask & ~event.bit
                    )
                  }
                />
                <span className="min-w-0">
                  <span className="block text-high">{event.label}</span>
                  <span className="block text-low">{event.description}</span>
                </span>
              </label>
            ))}
          </fieldset>
          <p className="text-low">{t('notifications.web.allOffHint')}</p>
        </div>
      ))}
      <p className="text-sm text-low">{t('notifications.web.deviceHint')}</p>
      {error && (
        <p role="alert" className="text-error">
          {t(`notifications.web.${error}`)} {errorDetail}
        </p>
      )}
    </div>
  );
}
