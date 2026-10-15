import { beforeEach, expect, it, vi } from 'vitest';
import { createMachineClient } from './machineClient';

const request = vi.hoisted(() => vi.fn());
vi.mock('./localApiTransport', () => ({ makeLocalApiRequest: request }));
vi.mock('./api', () => ({
  handleApiResponse: (response: Response) => response.json(),
}));

beforeEach(() => {
  request.mockReset();
  request.mockResolvedValue(new Response('{}'));
});

it('denies mutations from unavailable or stale settings clients without making a request', async () => {
  let available = true;
  const client = createMachineClient(
    'local',
    { id: 'local', apiHostId: null, kind: 'local', label: 'LVK server' },
    () => available
  );
  await client.listRepos();
  expect(request.mock.calls[0][1]).toMatchObject({
    hostScope: 'none',
  });
  request.mockClear();
  available = false;
  await expect(
    client.saveAgentSettingsProfile({
      type: 'delete',
      data: { source: { id: 'profile', expected_revision: 'revision' } },
    })
  ).rejects.toThrow('Host');
  await expect(
    client.saveProfiles({
      content: '{}',
      expected_revision: 'test',
      confirmed_sensitive_read: true,
    })
  ).rejects.toThrow('Host');
  expect(request).not.toHaveBeenCalled();
});

it('keeps explicit local and remote routing separate from the ambient route', async () => {
  const local = createMachineClient('local', {
    id: 'local',
    apiHostId: null,
    kind: 'local',
    label: 'Local',
  });
  await local.listRepos();
  expect(request.mock.calls[0][1]).toMatchObject({ hostScope: 'none' });
  const remote = createMachineClient('remote', {
    id: 'remote-b',
    apiHostId: 'remote-b',
    kind: 'remote',
    label: 'B',
  });
  request.mockResolvedValue(new Response('{}'));
  request.mockClear();
  await expect(remote.listRepos()).rejects.toThrow('Remote Access');
  expect(request).not.toHaveBeenCalled();
});
