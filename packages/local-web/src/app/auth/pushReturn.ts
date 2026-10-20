const key = 'lvk:push-login-return';
const uuid = '[0-9a-fA-F-]{36}';
const allowed = new RegExp(
  `^/(?:workspaces/${uuid}|projects/${uuid}/issues/${uuid}/workspaces/${uuid})$`
);

export function safePushReturn(value: string): string | null {
  if (!value.startsWith('/') || value.startsWith('//') || value.includes('\\'))
    return null;
  const url = new URL(value, 'https://lvk.invalid');
  if (url.origin !== 'https://lvk.invalid' || !allowed.test(url.pathname))
    return null;
  const result = new URL(url.pathname, url.origin);
  for (const name of ['session_id', 'agent_run_id']) {
    const id = url.searchParams.get(name);
    if (id && new RegExp(`^${uuid}$`).test(id))
      result.searchParams.set(name, id);
  }
  return result.pathname + result.search;
}
export function rememberPushReturn(
  storage: Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>,
  value: string
) {
  try {
    const path = safePushReturn(value);
    if (path) storage.setItem(key, JSON.stringify({ path, at: Date.now() }));
    else storage.removeItem(key);
  } catch {
    /* Storage is optional; the original notification can be reopened. */
  }
}
export function takePushReturn(
  storage: Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>
): string | null {
  try {
    const value = JSON.parse(storage.getItem(key) ?? 'null');
    storage.removeItem(key);
    if (
      !value ||
      typeof value.path !== 'string' ||
      typeof value.at !== 'number' ||
      Date.now() - value.at > 600_000
    )
      return null;
    return safePushReturn(value.path);
  } catch {
    return null;
  }
}
