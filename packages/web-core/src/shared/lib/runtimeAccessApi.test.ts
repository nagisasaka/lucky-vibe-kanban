import { describe, it, expect, vi } from 'vitest';
import { runtimeAccessSettings } from './runtimeAccessApi';
import { makeLocalApiRequest } from './localApiTransport';
vi.mock('./localApiTransport', () => ({ makeLocalApiRequest: vi.fn() }));
describe('runtime access settings', () => {
  it('preserves the revision and never turns permission failures into settings', async () => {
    vi.mocked(makeLocalApiRequest).mockResolvedValueOnce(
      new Response('Administrator required', { status: 403 })
    );
    await expect(runtimeAccessSettings()).rejects.toThrow(
      'Administrator required'
    );
    const input = { revision: 7, settings: { enabled: false, grants: [] } };
    vi.mocked(makeLocalApiRequest).mockResolvedValueOnce(
      Response.json({ ...input, revision: 8, source: 'saved' })
    );
    expect((await runtimeAccessSettings(input)).revision).toBe(8);
    expect(makeLocalApiRequest).toHaveBeenLastCalledWith(
      '/api/runtime-access-settings',
      expect.objectContaining({ method: 'PUT', body: JSON.stringify(input) })
    );
  });
});
