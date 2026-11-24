import type {
  SavedFilter,
  SavedFilterEvaluation,
  ListSavedFiltersResponse,
  CreateSavedFilterRequest,
  UpdateSavedFilterRequest,
  DeleteSavedFilterRequest,
  EvaluateSavedFilterRequest,
} from 'shared/remote-types';
import { makeLocalApiRequest } from '@/shared/lib/localApiTransport';

export const savedFilterKeys = {
  project: (projectId: string) => ['saved-filters', projectId] as const,
  list: (projectId: string) => ['saved-filters', projectId, 'list'] as const,
  definition: (projectId: string, id: string) =>
    ['saved-filters', projectId, 'definition', id] as const,
  evaluation: (projectId: string, id: string) =>
    ['saved-filters', projectId, 'evaluation', id] as const,
};
const base = (projectId: string) =>
  `/api/projects/${encodeURIComponent(projectId)}/saved-filters`;
export class SavedFilterRequestError extends Error {
  constructor(
    message: string,
    public status: number
  ) {
    super(message);
  }
}
async function request<T>(url: string, init?: RequestInit): Promise<T> {
  const response = await makeLocalApiRequest(url, {
    ...init,
    headers: { 'Content-Type': 'application/json' },
    cache: 'no-store',
  });
  const body = await response.json();
  if (!response.ok || !body.success)
    throw new SavedFilterRequestError(
      body.message ?? `HTTP ${response.status}`,
      response.status
    );
  return body.data as T;
}

// Retry a moving snapshot a bounded number of times. No partial collection is
// returned, so React Query keeps the previous complete result during retries.
export async function collectEvaluationPages(
  read: (offset: number, revision?: string) => Promise<SavedFilterEvaluation>
) {
  for (let attempt = 0; ; attempt++) {
    try {
      return await collectSnapshot(read);
    } catch (error) {
      if (
        !(error instanceof SavedFilterRequestError) ||
        error.status !== 409 ||
        attempt >= 2
      )
        throw error;
    }
  }
}
async function collectSnapshot(
  read: (offset: number, revision?: string) => Promise<SavedFilterEvaluation>
) {
  const first = await read(0);
  const ids = [...first.issue_ids];
  let next = first.next_offset;
  let offset = 0;
  while (next !== null) {
    if (next <= offset) throw new Error('Invalid saved filter pagination');
    offset = next;
    const page = await read(offset, first.evaluation_revision);
    if (page.evaluation_revision !== first.evaluation_revision)
      throw new SavedFilterRequestError(
        'Saved filter changed while loading results. Refresh to retry.',
        409
      );
    if (
      page.definition_revision !== first.definition_revision ||
      page.total_count !== first.total_count ||
      page.state !== first.state ||
      page.stale !== first.stale ||
      page.saved_filter_id !== first.saved_filter_id ||
      page.project_id !== first.project_id
    )
      throw new Error(
        'Saved filter changed while loading results. Refresh to retry.'
      );
    ids.push(...page.issue_ids);
    next = page.next_offset;
  }
  if (new Set(ids).size !== first.total_count)
    throw new Error('Incomplete saved filter result. Refresh to retry.');
  return { ...first, issue_ids: ids, next_offset: null };
}
export const savedFiltersApi = {
  async list(projectId: string, signal?: AbortSignal) {
    const filters: SavedFilter[] = [];
    let offset = 0;
    for (;;) {
      const page = await request<ListSavedFiltersResponse>(
        `${base(projectId)}?offset=${offset}&limit=500`,
        { signal }
      );
      filters.push(...page.saved_filters);
      if (page.next_offset === null) return filters;
      if (page.next_offset <= offset)
        throw new Error('Invalid saved filter pagination');
      offset = page.next_offset;
    }
  },
  get: (projectId: string, id: string, signal?: AbortSignal) =>
    request<SavedFilter>(`${base(projectId)}/${encodeURIComponent(id)}`, {
      signal,
    }),
  create: (projectId: string, body: CreateSavedFilterRequest) =>
    request<SavedFilter>(base(projectId), {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  update: (projectId: string, id: string, body: UpdateSavedFilterRequest) =>
    request<SavedFilter>(`${base(projectId)}/${encodeURIComponent(id)}`, {
      method: 'PUT',
      body: JSON.stringify(body),
    }),
  remove: (projectId: string, id: string, body: DeleteSavedFilterRequest) =>
    request<unknown>(`${base(projectId)}/${encodeURIComponent(id)}`, {
      method: 'DELETE',
      body: JSON.stringify(body),
    }),
  evaluate: (
    projectId: string,
    id: string,
    signal?: AbortSignal,
    retryFailed = false
  ) => {
    let firstRequest = true;
    return collectEvaluationPages((offset, revision) => {
      if (offset === 0) {
        const body: EvaluateSavedFilterRequest = {
          offset,
          limit: 500,
          expected_evaluation_revision: null,
          retry_failed: firstRequest && retryFailed,
        };
        firstRequest = false;
        return request<SavedFilterEvaluation>(
          `${base(projectId)}/${encodeURIComponent(id)}/evaluate`,
          { method: 'POST', body: JSON.stringify(body), signal }
        );
      }
      return request<SavedFilterEvaluation>(
        `${base(projectId)}/${encodeURIComponent(id)}/results?offset=${offset}&limit=500&expected_evaluation_revision=${encodeURIComponent(revision!)}`,
        { signal }
      );
    });
  },
};
