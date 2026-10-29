import { useTranslation } from 'react-i18next';
import { quotaView } from '../lib/codexQuota';
import { Popover, PopoverContent, PopoverTrigger } from './Popover';
const percent = (v: number | null) => (v === null ? '—' : `${Math.floor(v)}%`);
export function CodexQuota({
  data,
  unavailable,
  now,
  refresh,
  fetching,
  canRefresh,
}: {
  data: unknown;
  unavailable: boolean;
  now: number;
  refresh: () => void;
  fetching: boolean;
  canRefresh: boolean;
}) {
  const { t, i18n } = useTranslation('common');
  const time = (v: number | null) =>
    v === null
      ? t('codexQuota.unknownTime')
      : new Date(v).toLocaleString(i18n.language);
  const view = quotaView(data, unavailable, now);
  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          data-testid="codex-quota"
          className="shrink-0 rounded-sm text-xs text-normal focus-visible:ring-2 focus-visible:ring-brand"
          aria-label={t('codexQuota.detailsLabel', {
            value: percent(view.remaining),
          })}
        >
          {t('codexQuota.remaining', { value: percent(view.remaining) })}
        </button>
      </PopoverTrigger>
      <PopoverContent
        aria-label={t('codexQuota.title')}
        className="max-w-[calc(100vw-1rem)] max-h-[70vh] overflow-y-auto text-xs text-normal"
      >
        <p>
          {t('codexQuota.statusLabel', {
            status: t(`codexQuota.status.${view.status}`),
          })}
        </p>
        <p>{t('codexQuota.observed', { time: time(view.observed) })}</p>
        <p>{t('codexQuota.scope')}</p>
        <p>{t('codexQuota.selection')}</p>
        {view.rows.map((row, i) => (
          <div key={i} className="mt-base">
            <p className="break-all">
              {row.representative
                ? t('codexQuota.defaultBucket')
                : t('codexQuota.additionalBucket', { index: row.index })}
              {row.name && ` · ${row.name}`}
              {row.id && ` (${row.id})`}
            </p>
            {row.windows.map((w) => (
              <p key={w.key}>
                {t('codexQuota.window', {
                  kind: w.key,
                  period: w.period
                    ? t(`codexQuota.period.${w.period.unit}`, {
                        count: w.period.count,
                      })
                    : t('codexQuota.unknownPeriod'),
                  value: percent(w.remaining),
                  reset: time(w.reset),
                })}
              </p>
            ))}
            {row.unlimitedCredits && <p>{t('codexQuota.unlimitedCredits')}</p>}
          </div>
        ))}
        <button
          type="button"
          className="mt-base rounded-sm border border-border p-base focus-visible:ring-2 focus-visible:ring-brand"
          disabled={!canRefresh}
          onClick={refresh}
        >
          {t(
            fetching
              ? 'codexQuota.refreshing'
              : canRefresh
                ? 'codexQuota.refresh'
                : 'codexQuota.cooldown'
          )}
        </button>
      </PopoverContent>
    </Popover>
  );
}
