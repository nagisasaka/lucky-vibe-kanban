import { describe, expect, it, vi } from 'vitest';
import {
  collectEvaluationPages,
  savedFiltersApi,
} from '@/shared/lib/savedFiltersApi';
import {
  emptyMachineFilter,
  hasEvaluationResult,
  savedFilterHref,
} from '@/shared/lib/savedFilters';
import type { SavedFilterEvaluation } from 'shared/remote-types';
vi.mock('@/shared/lib/localApiTransport', () => ({
  makeLocalApiRequest: vi.fn(),
}));
import { makeLocalApiRequest } from '@/shared/lib/localApiTransport';
const result = (
  patch: Partial<SavedFilterEvaluation> = {}
): SavedFilterEvaluation => ({
  saved_filter_id: 'filter',
  project_id: 'project',
  state: 'complete',
  issue_ids: ['a'],
  total_count: 1,
  offset: 0,
  limit: 500,
  next_offset: null,
  stale: false,
  error: null,
  definition_revision: 1,
  ...patch,
});
describe('saved filter evaluation and transport', () => {
  it('assembles all pages before exposing results', async () => {
    const read = vi
      .fn()
      .mockResolvedValueOnce(
        result({ issue_ids: ['a'], total_count: 2, next_offset: 1 })
      )
      .mockResolvedValueOnce(
        result({ issue_ids: ['b'], total_count: 2, offset: 1 })
      );
    expect((await collectEvaluationPages(read)).issue_ids).toEqual(['a', 'b']);
    expect(read.mock.calls).toEqual([[0], [1]]);
  });
  it.each([
    { definition_revision: 2 },
    { total_count: 3 },
    { state: 'evaluating' },
    { stale: true },
    { saved_filter_id: 'other' },
  ])('rejects changed page metadata %o', async (patch) => {
    const read = vi
      .fn()
      .mockResolvedValueOnce(result({ total_count: 2, next_offset: 1 }))
      .mockResolvedValueOnce(
        result({
          issue_ids: ['b'],
          total_count: 2,
          offset: 1,
          ...patch,
        } as Partial<SavedFilterEvaluation>)
      );
    await expect(collectEvaluationPages(read)).rejects.toThrow('changed');
  });
  it('rejects duplicate/partial results and repeated offsets', async () => {
    await expect(
      collectEvaluationPages(
        vi.fn().mockResolvedValue(result({ total_count: 2 }))
      )
    ).rejects.toThrow('Incomplete');
    await expect(
      collectEvaluationPages(
        vi.fn().mockResolvedValue(result({ next_offset: 0 }))
      )
    ).rejects.toThrow('pagination');
  });
  it('distinguishes initial failure and evaluating from a completed empty result', () => {
    expect(
      hasEvaluationResult(
        result({ state: 'failed', issue_ids: [], total_count: 0 })
      )
    ).toBe(false);
    expect(
      hasEvaluationResult(
        result({ state: 'evaluating', issue_ids: [], total_count: 0 })
      )
    ).toBe(false);
    expect(hasEvaluationResult(result({ issue_ids: [], total_count: 0 }))).toBe(
      true
    );
    expect(hasEvaluationResult(result({ state: 'failed', stale: true }))).toBe(
      true
    );
  });
  it('encodes stable IDs and defaults to active issues', () => {
    expect(savedFilterHref('project', 'filter')).toBe(
      '/projects/project?savedFilter=filter'
    );
    expect(emptyMachineFilter().archived).toBe('active');
  });
  it('uses the project scoped API, revision and stable request id', async () => {
    const transport = vi.mocked(makeLocalApiRequest);
    transport.mockResolvedValue(
      new Response(JSON.stringify({ success: true, data: { id: 'filter' } }))
    );
    const body = {
      request_id: 'request',
      expected_revision: 4,
      definition: {
        name: 'Renamed',
        machine: emptyMachineFilter(),
        natural_language: null,
        context: 'release',
      },
    };
    await savedFiltersApi.update('project', 'filter', body);
    expect(transport).toHaveBeenLastCalledWith(
      '/api/projects/project/saved-filters/filter',
      expect.objectContaining({ method: 'PUT', body: JSON.stringify(body) })
    );
  });
});

it('activates once, reads subsequent pages without activation, and retries failures only explicitly', async () => {
  const transport = vi.mocked(makeLocalApiRequest);
  transport.mockReset();
  transport.mockResolvedValueOnce(
    new Response(
      JSON.stringify({
        success: true,
        data: result({ total_count: 2, next_offset: 1 }),
      })
    )
  );
  transport.mockResolvedValueOnce(
    new Response(
      JSON.stringify({
        success: true,
        data: result({ issue_ids: ['b'], total_count: 2, offset: 1 }),
      })
    )
  );
  expect(
    (await savedFiltersApi.evaluate('project', 'filter')).issue_ids
  ).toEqual(['a', 'b']);
  expect(transport.mock.calls[0][1]?.body).toBe(
    JSON.stringify({ offset: 0, limit: 500, retry_failed: false })
  );
  expect(transport.mock.calls[1][0]).toBe(
    '/api/projects/project/saved-filters/filter/results?offset=1&limit=500'
  );
  expect(transport.mock.calls[1][1]?.method).toBeUndefined();
  transport.mockResolvedValueOnce(
    new Response(JSON.stringify({ success: true, data: result() }))
  );
  await savedFiltersApi.evaluate('project', 'filter', undefined, true);
  expect(transport.mock.calls[2][1]?.body).toBe(
    JSON.stringify({ offset: 0, limit: 500, retry_failed: true })
  );
});
