import { makeLocalApiRequest } from './localApiTransport';
import type { workspacesApi } from './api';
export async function readUtility<T>(url: string): Promise<T> {
  const response = await makeLocalApiRequest(url, {
    cache: 'no-store',
    signal: AbortSignal.timeout(15000),
  });
  if (!response.ok) throw new Error('Utility unavailable');
  const body = await response.json();
  if (!body.success) throw new Error(body.message ?? 'Utility unavailable');
  return body.data as T;
}
export interface MainRepo {
  id: string;
  name: string;
  path: string;
  wiki_enabled: boolean | null;
}
// Adapt the existing viewers to Project scope; never send this ID to Workspace routes.
export function projectUtilitiesApi(
  projectId: string
): typeof workspacesApi.files {
  const url = (
    kind: string,
    params?: { repoId: string; path?: string | null }
  ) => {
    const query = new URLSearchParams();
    if (params) {
      query.set('repo_id', params.repoId);
      if (params.path) query.set('path', params.path);
    }
    return `/api/projects/${projectId}/utilities/${kind}?${query}`;
  };
  return {
    tree: () => readUtility(url('tree')),
    directory: (_, params) => readUtility(url('directory', params)),
    content: (_, params) => readUtility(url('content', params)),
    rawUrl: (_, params) => url('raw', params),
  };
}
