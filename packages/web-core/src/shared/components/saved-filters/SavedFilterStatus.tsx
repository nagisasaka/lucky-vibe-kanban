import { SavedFilterRequestError } from '@/shared/lib/savedFiltersApi';
import { useTranslation } from 'react-i18next';
import type { SavedFilterView } from '@/shared/hooks/useSavedFilter';

export function SavedFilterStatus({
  view,
  count,
}: {
  view: Pick<
    SavedFilterView,
    'state' | 'stale' | 'ready' | 'missingIssues' | 'evaluation' | 'definition'
  >;
  count: number;
}) {
  const { t } = useTranslation('settings');
  if (view.definition.isError)
    return (
      <p role="alert" className="text-error">
        {t(
          view.definition.error instanceof SavedFilterRequestError &&
            [400, 404].includes(view.definition.error.status)
            ? 'savedFilters.unavailable'
            : 'savedFilters.error'
        )}
      </p>
    );
  return (
    <div
      role={view.state === 'failed' ? 'alert' : 'status'}
      aria-live="polite"
      className="space-y-half"
    >
      <p>
        {
          (
            {
              unevaluated: t('savedFilters.unevaluated'),
              evaluating: t('savedFilters.evaluating'),
              complete: t('savedFilters.complete'),
              failed: t('savedFilters.failed'),
            } as const
          )[view.state]
        }
        {view.stale && <> · {t('savedFilters.stale')}</>}
        {view.evaluation.isFetching && view.ready && (
          <> · {t('savedFilters.refreshing')}</>
        )}
      </p>
      {view.ready ? (
        <p>{t('savedFilters.count', { count })}</p>
      ) : (
        <p>
          {t(
            view.missingIssues
              ? 'savedFilters.missingIssues'
              : 'savedFilters.pending'
          )}
        </p>
      )}
      {view.evaluation.data?.error && (
        <p className="text-error">{view.evaluation.data.error}</p>
      )}
    </div>
  );
}
