import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { workspacesApi } from '@/shared/lib/api';

/** Read-only, scoped to the Workspace even when the selected Session changes. */
export function WorkspaceTokenSummary({
  workspaceId,
}: {
  workspaceId: string;
}) {
  const { t } = useTranslation('common');
  const { data, isError } = useQuery({
    queryKey: ['workspaceTokens', workspaceId],
    queryFn: () => workspacesApi.getTokens(workspaceId),
    // Match the existing React Query refresh mechanism; only while mounted and visible.
    refetchInterval: 5000,
  });
  const number = (value: number | bigint | null | undefined) =>
    value == null ? t('workspaceTokens.unknown') : value.toLocaleString();
  const coverage = (observed: number | bigint) =>
    t('workspaceTokens.breakdownCoverage', {
      observed: Number(observed),
      total: Number(data?.thread_count ?? 0),
    });
  return (
    <details
      className="px-base py-half text-xs text-low"
      data-testid="workspace-tokens"
    >
      <summary className="cursor-pointer w-fit focus-visible:outline focus-visible:outline-2">
        {t('workspaceTokens.label')}:{' '}
        {isError ? t('workspaceTokens.error') : number(data?.total_tokens)}
        {!isError &&
          data?.completeness === 'partial' &&
          ` · ${t('workspaceTokens.partial')}`}
      </summary>
      <div className="py-half space-y-half">
        <p>{t('workspaceTokens.scope')}</p>
        {data && !isError && (
          <>
            <p>
              {t('workspaceTokens.input')}: {number(data.input_tokens)} ·{' '}
              {t('workspaceTokens.output')}: {number(data.output_tokens)}
            </p>
            <p>
              {t('workspaceTokens.cached')}: {number(data.cached_input_tokens)}{' '}
              ({coverage(data.cached_input_observed_threads)})
            </p>
            <p>
              {t('workspaceTokens.reasoning')}: {number(data.reasoning_tokens)}{' '}
              ({coverage(data.reasoning_observed_threads)})
            </p>
            {data.updated_at && (
              <p>
                {t('workspaceTokens.updated')}: {data.updated_at}
              </p>
            )}
          </>
        )}
      </div>
    </details>
  );
}
