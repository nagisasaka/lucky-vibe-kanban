import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { readUtility } from '@/shared/lib/projectUtilitiesApi';
import type { Repo, SharedStorageUsage } from 'shared/types';

export const sharedStorageKey = (repoId: string) => ['shared-storage', repoId];
export function storageBytes(bytes: number | null | undefined): string {
  if (bytes == null) return '—';
  if (bytes < 1024) return `${bytes} B`;
  const power = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), 4);
  return `${(bytes / 1024 ** power).toFixed(1)} ${['B', 'KiB', 'MiB', 'GiB', 'TiB'][power]}`;
}
function RepositorySize({ repoId }: { repoId: string }) {
  const { t } = useTranslation('common');
  const repo = useQuery({
    queryKey: ['shared-storage-repo', repoId],
    queryFn: () => readUtility<Repo>(`/api/repos/${repoId}`),
    staleTime: 60000,
    retry: false,
  });
  const usage = useQuery({
    queryKey: sharedStorageKey(repoId),
    queryFn: () =>
      readUtility<SharedStorageUsage>(`/api/repos/${repoId}/shared-storage`),
    staleTime: 60000,
    retry: false,
    refetchOnWindowFocus: false,
  });
  return (
    <div className="mt-base border-t pt-base">
      <p className="break-all font-medium" title={repo.data?.path ?? repoId}>
        {repo.data?.display_name ?? repoId}{' '}
        <span className="text-low">({repoId.slice(0, 8)})</span>
      </p>
      {(['cache', 'tmp'] as const).map((kind) => (
        <div key={kind} className="flex justify-between gap-base">
          <span className="text-low">{t('sharedStorage.kind', { kind })}</span>
          <span>
            {usage.isError || usage.data?.[kind].error
              ? t('sharedStorage.failed')
              : usage.isPending
                ? t('sharedStorage.measuring')
                : storageBytes(usage.data?.[kind].bytes)}
          </span>
        </div>
      ))}
    </div>
  );
}
export function SharedStorageBreakdown({ repoIds }: { repoIds: string[] }) {
  return (
    <>
      {[...new Set(repoIds)].map((id) => (
        <RepositorySize key={id} repoId={id} />
      ))}
    </>
  );
}
