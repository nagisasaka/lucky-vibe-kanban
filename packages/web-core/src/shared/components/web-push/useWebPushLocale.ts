import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';

// Keep writes ordered across locale changes and component remounts. An older
// in-flight request must settle before a newer locale can replace it.
let pendingUpdate: Promise<void> = Promise.resolve();
export function queuePushLocaleUpdate(update: () => Promise<void>) {
  const next = pendingUpdate.catch(() => undefined).then(update);
  pendingUpdate = next;
  return next;
}

export async function synchronizeWebPushLocale(locale: string) {
  if (
    !window.isSecureContext ||
    !('serviceWorker' in navigator) ||
    !('Notification' in window) ||
    Notification.permission !== 'granted'
  )
    return;
  const worker = await navigator.serviceWorker.getRegistration('/');
  const subscription = await worker?.pushManager.getSubscription();
  if (!subscription) return;
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(subscription.endpoint)
  );
  const hash = Array.from(new Uint8Array(digest))
    .map((byte) => byte.toString(16).padStart(2, '0'))
    .join('');
  const response = await fetch('/api/web-push', { credentials: 'same-origin' });
  if (!response.ok) return;
  const status: { registrations: { id: string; endpoint_hash: string }[] } =
    await response.json();
  for (const registration of status.registrations) {
    if (registration.endpoint_hash !== hash) continue;
    const updated = await fetch(
      `/api/web-push/${encodeURIComponent(registration.id)}/locale`,
      {
        method: 'POST',
        credentials: 'same-origin',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ locale }),
      }
    );
    if (!updated.ok) throw new Error();
  }
}

export function useWebPushLocale(enabled: boolean) {
  const { i18n } = useTranslation();
  useEffect(() => {
    if (!enabled) return;
    let active = true;
    let revision = 0;
    const update = () => {
      const current = ++revision;
      const locale = i18n.resolvedLanguage ?? i18n.language;
      void queuePushLocaleUpdate(async () => {
        if (active && current === revision)
          await synchronizeWebPushLocale(locale);
      }).catch(() => undefined); // Retry on the next language/visibility/online event.
    };
    const visible = () => {
      if (document.visibilityState === 'visible') update();
    };
    i18n.on('languageChanged', update);
    window.addEventListener('online', update);
    document.addEventListener('visibilitychange', visible);
    update();
    return () => {
      active = false;
      i18n.off('languageChanged', update);
      window.removeEventListener('online', update);
      document.removeEventListener('visibilitychange', visible);
    };
  }, [enabled, i18n]);
}
