import { useMemo } from 'react';
import { useQueries } from '@tanstack/react-query';
import { readUtility } from '@/shared/lib/projectUtilitiesApi';
export function useWikiAvailability(repoIds: string[]) {
  const queries = useQueries({
    queries: repoIds.map((id) => ({
      queryKey: ['repository-wiki-enabled', id],
      queryFn: () =>
        readUtility<{ enabled: boolean }>(`/api/repos/${id}/memory`),
      staleTime: 30000,
      retry: false,
    })),
  });
  const enabled = repoIds
    .filter((_, i) => !queries[i].isError && queries[i].data?.enabled === true)
    .join(',');
  return useMemo(() => (enabled ? enabled.split(',') : []), [enabled]);
}
