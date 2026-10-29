import { afterEach, expect, it, vi } from 'vitest';
import type { MutationDefinition, ShapeDefinition } from 'shared/remote-types';
import {
  buildSourceKey,
  createShapeCollection,
  refreshShapeFallback,
} from './collections';
import { snapshotActivity } from './snapshotActivity';

const request = vi.hoisted(() => vi.fn());
vi.mock('@/shared/lib/remoteApi', () => ({ makeRequest: request }));

type Row = { id: string; title: string };
const collections: ReturnType<typeof createShapeCollection>[] = [];
afterEach(async () => {
  await Promise.all(
    collections.splice(0).map((collection) => collection.cleanup())
  );
  request.mockReset();
});

it('keeps optimistic edits until a snapshot started after the mutation confirms them', async () => {
  const shape = {
    table: 'issues',
    params: [],
    fallbackUrl: '/v1/fallback/issues',
  } as ShapeDefinition<Row>;
  const mutation = { name: 'issues', url: '/v1/issues' } as MutationDefinition<
    Row,
    Row,
    Partial<Row>
  >;
  const response = (title: string) =>
    Response.json({ issues: [{ id: 'issue', title }] });
  let resolveStale!: (response: Response) => void;
  let resolveFresh!: (response: Response) => void;
  let gets = 0;
  request.mockImplementation((_path: string, options: RequestInit) => {
    if (options.method === 'PATCH') return Promise.resolve(Response.json({}));
    gets += 1;
    if (gets === 1) return Promise.resolve(response('Before'));
    return new Promise<Response>((resolve) => {
      if (gets === 2) resolveStale = resolve;
      else resolveFresh = resolve;
    });
  });
  const collection = createShapeCollection(shape, {}, undefined, mutation);
  collections.push(collection);
  await collection.preload();
  refreshShapeFallback(shape, {});
  await vi.waitFor(() => expect(gets).toBe(2));
  const transaction = collection.update('issue', (row) => {
    row.title = 'After';
  });
  await vi.waitFor(() =>
    expect(request).toHaveBeenCalledWith(
      '/v1/issues/issue',
      expect.objectContaining({ method: 'PATCH' })
    )
  );
  resolveStale(response('Before'));
  await vi.waitFor(() => expect(gets).toBe(3));
  expect(collection.get('issue')?.title).toBe('After');
  resolveFresh(response('After'));
  await transaction.isPersisted.promise;
  expect(collection.get('issue')?.title).toBe('After');
});

it('reports actual request freshness and failures independently of row changes', async () => {
  const shape = {
    table: 'issues',
    params: [],
    fallbackUrl: '/v1/fallback/issues',
  } as ShapeDefinition<Row>;
  const params = { project_id: 'motion-freshness' };
  const activity = snapshotActivity(buildSourceKey(shape.table, params));
  let resolve!: (response: Response) => void;
  request.mockImplementation(
    () =>
      new Promise<Response>((done) => {
        resolve = done;
      })
  );
  const collection = createShapeCollection(shape, params);
  collections.push(collection);
  const loaded = collection.preload();
  await vi.waitFor(() => expect(activity.pending).toBe(true));
  expect(activity.state.sequence).toBe(0);
  resolve(Response.json({ issues: [{ id: 'a', title: 'unchanged' }] }));
  await loaded;
  expect(activity.pending).toBe(false);
  expect(activity.state).toMatchObject({ sequence: 1, healthy: true });
  const startedAt = activity.state.startedAt;
  refreshShapeFallback(shape, params);
  await vi.waitFor(() => expect(activity.pending).toBe(true));
  expect(activity.state.sequence).toBe(1);
  resolve(Response.json({ message: 'offline' }, { status: 503 }));
  await vi.waitFor(() =>
    expect(activity.state).toMatchObject({ sequence: 2, healthy: false })
  );
  expect(activity.state.startedAt).toBeGreaterThanOrEqual(startedAt);
  refreshShapeFallback(shape, params);
  await vi.waitFor(() => expect(activity.pending).toBe(true));
  resolve(Response.json({ issues: [{ id: 'a', title: 'unchanged' }] }));
  await vi.waitFor(() =>
    expect(activity.state).toMatchObject({ sequence: 3, healthy: true })
  );
  expect(collection.get('a')?.title).toBe('unchanged');
});
