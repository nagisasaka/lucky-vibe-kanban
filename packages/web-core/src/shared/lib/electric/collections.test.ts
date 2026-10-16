import { afterEach, expect, it, vi } from 'vitest';
import type { MutationDefinition, ShapeDefinition } from 'shared/remote-types';
import { createShapeCollection, refreshShapeFallback } from './collections';

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
