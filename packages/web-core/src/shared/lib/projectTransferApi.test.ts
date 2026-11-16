import { expect, it, vi, beforeEach } from 'vitest';
import { makeLocalApiRequest } from './localApiTransport';
import { projectTransferApi } from './projectTransferApi';
vi.mock('./localApiTransport', () => ({ makeLocalApiRequest: vi.fn() }));
vi.mock('./api', () => ({
  handleApiResponse: async (response: Response) => (await response.json()).data,
}));
beforeEach(() => vi.mocked(makeLocalApiRequest).mockReset());
it('previews and imports the exact selected bytes with the preview digest', async () => {
  const bytes = new Uint8Array([0, 255, 1, 128]);
  vi.mocked(makeLocalApiRequest).mockImplementation(
    async () => new Response(JSON.stringify({ data: { sha256: 'digest' } }))
  );
  const preview = await projectTransferApi.preview(bytes);
  expect(vi.mocked(makeLocalApiRequest).mock.calls[0][1]?.body).toBe(bytes);
  await projectTransferApi.import(bytes, preview.sha256, 'NEW', {
    source: 'destination',
  });
  const body = JSON.parse(
    String(vi.mocked(makeLocalApiRequest).mock.calls[1][1]?.body)
  );
  expect(Array.from(atob(body.data_base64), (c) => c.charCodeAt(0))).toEqual(
    Array.from(bytes)
  );
  expect(body.expected_sha256).toBe('digest');
  expect(body.repo_map).toEqual({ source: 'destination' });
});
it('sends an explicit opt-in decision, including exclusion', async () => {
  vi.mocked(makeLocalApiRequest).mockImplementation(
    async () => new Response('sqlite')
  );
  await projectTransferApi.export('project', false);
  await projectTransferApi.export('project', true);
  expect(
    vi
      .mocked(makeLocalApiRequest)
      .mock.calls.map(
        (call) => JSON.parse(String(call[1]?.body)).include_history
      )
  ).toEqual([false, true]);
});
