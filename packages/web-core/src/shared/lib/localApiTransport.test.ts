import { afterEach, expect, it, vi } from 'vitest';
import {
  makeLocalApiRequest,
  openLocalApiWebSocket,
  setLocalApiTransport,
} from './localApiTransport';
vi.mock('@/shared/providers/HostIdProvider', () => ({
  getCurrentHostId: () => null,
}));
afterEach(() => setLocalApiTransport(null));
it('rejects retired host targeting for both HTTP and WebSocket before dispatch', async () => {
  const request = vi.fn();
  const openWebSocket = vi.fn();
  setLocalApiTransport({ request, openWebSocket });
  for (const options of [
    { hostScope: 'none' as const, relayHostId: 'old-host' },
    { hostScope: 'explicit' as const, hostId: 'old-host' },
  ]) {
    await expect(makeLocalApiRequest('/api/config', options)).rejects.toThrow(
      'Remote Access'
    );
    await expect(
      openLocalApiWebSocket('/api/terminal', options)
    ).rejects.toThrow('Remote Access');
  }
  expect(request).not.toHaveBeenCalled();
  expect(openWebSocket).not.toHaveBeenCalled();
  request.mockResolvedValue(Response.json({}));
  await makeLocalApiRequest('/api/info');
  expect(request).toHaveBeenCalledWith('/api/info', {});
});
