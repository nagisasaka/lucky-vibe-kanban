import { setLocalServerUserId } from '@/shared/lib/localIdentity';
export interface AccessStatus {
  mode: 'locked' | 'local' | 'basic' | 'auth0';
  deployment: 'local' | 'public';
  authenticated: boolean;
  management: boolean;
  limited: boolean;
  csrf: string | null;
  public_url: string | null;
  forced_mode: string | null;
  configuration_error: boolean;
  user: { id: string; admin: boolean } | null;
}
let status: AccessStatus | undefined;
const rawFetch = window.fetch.bind(window);

export async function loadAccessStatus(): Promise<AccessStatus> {
  const response = await rawFetch('/api/access/status', {
    credentials: 'same-origin',
    cache: 'no-store',
  });
  if (!response.ok)
    throw new Error(
      '認証状態を取得できません。サーバーの配置設定を確認してください。'
    );
  status = (await response.json()) as AccessStatus;
  if (status.user) setLocalServerUserId(status.user.id);
  return status;
}
export async function currentServerUser() {
  const current = status ?? (await loadAccessStatus());
  if (!current.user) throw new Error('LVKへのログインが必要です');
  return { user_id: current.user.id };
}

/** Add CSRF only to this server's API requests; never to a remote OAuth/API. */
export function installAccessFetch() {
  window.fetch = async (input, init) => {
    const url = new URL(
      input instanceof Request ? input.url : String(input),
      window.location.href
    );
    const method = (
      init?.method ?? (input instanceof Request ? input.method : 'GET')
    ).toUpperCase();
    const localApi =
      url.origin === window.location.origin && url.pathname.startsWith('/api/');
    let options = init;
    if (
      localApi &&
      !['GET', 'HEAD', 'OPTIONS'].includes(method) &&
      status?.csrf
    ) {
      const headers = new Headers(
        input instanceof Request ? input.headers : undefined
      );
      new Headers(init?.headers).forEach((value, key) =>
        headers.set(key, value)
      );
      if (!headers.has('x-lvk-csrf')) headers.set('x-lvk-csrf', status.csrf);
      options = { ...init, headers };
    }
    const response = await rawFetch(input, options);
    if (
      localApi &&
      !url.pathname.startsWith('/api/access/') &&
      [401, 423].includes(response.status)
    ) {
      window.dispatchEvent(new Event('lvk-access-expired'));
    }
    return response;
  };
}

export async function accessRequest<T>(
  path: string,
  value?: unknown,
  headers?: HeadersInit
): Promise<T> {
  const requestHeaders = new Headers(headers);
  requestHeaders.set('content-type', 'application/json');
  const response = await fetch(`/api/access/${path}`, {
    method: value === undefined ? 'GET' : 'POST',
    credentials: 'same-origin',
    headers: requestHeaders,
    ...(value === undefined ? {} : { body: JSON.stringify(value) }),
  });
  if (!response.ok) {
    const error = await response.json().catch(() => ({}));
    throw new Error(
      typeof error.error === 'string' ? error.error : `HTTP ${response.status}`
    );
  }
  return (response.status === 204 ? undefined : await response.json()) as T;
}
