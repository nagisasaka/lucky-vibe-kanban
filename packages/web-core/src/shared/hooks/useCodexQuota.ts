import { useQuery } from '@tanstack/react-query';
import { CODEX_POLL_MS } from '@vibe/ui/lib/codexQuota';
import { makeLocalApiRequest } from '@/shared/lib/localApiTransport';

export function useCodexQuota(visible: boolean) {
  const query = useQuery({
    queryKey: ['codex-account-quota'],
    queryFn: async () => {
      const response = await makeLocalApiRequest('/api/provider-usage/read', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          provider: 'codex',
          account_context: 'server_default',
          include_usage: false,
        }),
        signal: AbortSignal.timeout(20_000),
        cache: 'no-store',
      });
      if (!response.ok) throw new Error('Codex quota unavailable');
      const body = await response.json();
      if (!body.success) throw new Error('Codex quota unavailable');
      return body.data as unknown;
    },
    enabled: visible,
    staleTime: CODEX_POLL_MS,
    refetchInterval: visible ? CODEX_POLL_MS : false,
    refetchIntervalInBackground: false,
    refetchOnWindowFocus: false,
    retry: false,
  });
  const canRefresh =
    visible &&
    !query.isFetching &&
    Date.now() - Math.max(query.dataUpdatedAt, query.errorUpdatedAt) >= 30_000;
  return {
    canRefresh,
    data: query.data,
    unavailable: query.isError || !visible,
    fetching: query.isFetching,
    refresh: () => {
      if (canRefresh) void query.refetch();
    },
  };
}
