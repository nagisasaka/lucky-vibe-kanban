import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import type { Repo, SharedStorageKind } from 'shared/types';
import type { MachineClient } from '@/shared/lib/machineClient';
import {
  sharedStorageKey,
  storageBytes,
} from '@/shared/components/SharedStorageBreakdown';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { SettingsCard } from './SettingsComponents';

export function SharedStorageSettings({
  repo,
  client,
}: {
  repo: Repo;
  client: MachineClient;
}) {
  const { t } = useTranslation('common');
  const queryClient = useQueryClient();
  const query = useQuery({
    queryKey: sharedStorageKey(repo.id),
    queryFn: () => client.getSharedStorage(repo.id),
    staleTime: 60000,
    retry: false,
    refetchOnWindowFocus: false,
  });
  const [selected, setSelected] = useState<SharedStorageKind | null>(null);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<'cleared' | 'clearFailed' | null>(null);
  const clear = async () => {
    if (!selected || busy) return;
    setBusy(true);
    setResult(null);
    try {
      const usage = await client.clearSharedStorage(repo.id, selected);
      queryClient.setQueryData(sharedStorageKey(repo.id), usage);
      setSelected(null);
      setResult('cleared');
    } catch {
      setResult('clearFailed');
      // Cleanup can fail after deleting some entries: refresh even on failure.
      await queryClient.invalidateQueries({
        queryKey: sharedStorageKey(repo.id),
      });
    } finally {
      setBusy(false);
    }
  };
  return (
    <SettingsCard
      title={t('sharedStorage.title')}
      description={repo.display_name}
    >
      <p className="break-all text-sm text-low">{repo.path}</p>
      <p className="text-sm text-normal">{t('sharedStorage.impact')}</p>
      <p className="text-sm text-low">{t('sharedStorage.activity')}</p>
      {(['cache', 'tmp'] as const).map((kind) => (
        <div key={kind} className="space-y-half border-t pt-base">
          <div className="flex flex-wrap items-center justify-between gap-base">
            <span className="font-medium">
              {t('sharedStorage.kind', { kind })}
            </span>
            <span className="text-sm">
              {query.isError || query.data?.[kind].error
                ? t('sharedStorage.failed')
                : query.isPending
                  ? t('sharedStorage.measuring')
                  : storageBytes(query.data?.[kind].bytes)}
            </span>
            <PrimaryButton
              type="button"
              variant="tertiary"
              className="border border-error/50 !text-error focus-visible:ring-2 focus-visible:ring-brand disabled:opacity-50"
              disabled={busy}
              onClick={() => {
                setSelected(kind);
                setResult(null);
              }}
            >
              {t('sharedStorage.clear', { kind })}
            </PrimaryButton>
          </div>
          <p className="text-sm text-low">
            {t(`sharedStorage.${kind}Description`)}
          </p>
        </div>
      ))}
      {selected && (
        <div
          className="space-y-base rounded-sm border p-base"
          role="group"
          aria-label={t('sharedStorage.confirm', {
            name: repo.display_name,
            kind: selected,
          })}
        >
          <p className="text-sm">
            {t('sharedStorage.confirm', {
              name: repo.display_name,
              kind: selected,
            })}
          </p>
          <div className="flex gap-base">
            <PrimaryButton
              type="button"
              variant="tertiary"
              className="border border-error/50 !text-error focus-visible:ring-2 focus-visible:ring-brand disabled:opacity-50"
              disabled={busy}
              onClick={() => void clear()}
            >
              {t('sharedStorage.confirmClear')}
            </PrimaryButton>
            <PrimaryButton
              type="button"
              variant="tertiary"
              className="border border-border focus-visible:ring-2 focus-visible:ring-brand disabled:opacity-50"
              disabled={busy}
              onClick={() => setSelected(null)}
            >
              {t('buttons.cancel')}
            </PrimaryButton>
          </div>
        </div>
      )}
      {result && (
        <p
          role={result === 'clearFailed' ? 'alert' : 'status'}
          className={
            result === 'clearFailed'
              ? 'text-sm text-error'
              : 'text-sm text-normal'
          }
        >
          {t(`sharedStorage.${result}`)}
        </p>
      )}
      <PrimaryButton
        type="button"
        variant="tertiary"
        className="border border-border focus-visible:ring-2 focus-visible:ring-brand disabled:opacity-50"
        disabled={busy || query.isFetching}
        onClick={() => void query.refetch()}
      >
        {t('sharedStorage.refresh')}
      </PrimaryButton>
    </SettingsCard>
  );
}
