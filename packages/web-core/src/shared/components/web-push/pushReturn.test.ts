import { describe, expect, it } from 'vitest';
import {
  safePushReturn,
  rememberPushReturn,
  takePushReturn,
} from '../../../../../local-web/src/app/auth/pushReturn';
const id = '11111111-1111-4111-8111-111111111111';
describe('notification login return', () => {
  it('retains only local workspace routes and exact identity search keys', () => {
    const path = `/projects/${id}/issues/${id}/workspaces/${id}?session_id=${id}&agent_run_id=${id}`;
    expect(safePushReturn(path + '&access=recovery#bad')).toBe(path);
    for (const bad of [
      '//evil.example/',
      '/\\evil.example/',
      'https://evil.example/',
      '/api/approvals/id/respond',
      '/?access=settings',
    ])
      expect(safePushReturn(bad)).toBeNull();
  });
  it('consumes the OAuth return once and expires stale navigation', () => {
    const values = new Map<string, string>();
    const storage = {
      getItem: (k: string) => values.get(k) ?? null,
      setItem: (k: string, v: string) => values.set(k, v),
      removeItem: (k: string) => values.delete(k),
    };
    rememberPushReturn(storage, `/workspaces/${id}?session_id=${id}`);
    expect(takePushReturn(storage)).toBe(`/workspaces/${id}?session_id=${id}`);
    expect(takePushReturn(storage)).toBeNull();
    values.set(
      'lvk:push-login-return',
      JSON.stringify({ path: `/workspaces/${id}`, at: 0 })
    );
    expect(takePushReturn(storage)).toBeNull();
  });
});
