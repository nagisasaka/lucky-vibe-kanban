import { useTranslation } from 'react-i18next';
import { ArrowClockwiseIcon, CaretDownIcon } from '@phosphor-icons/react';
import { resourceTone } from '@vibe/ui/lib/resourceObservation';

type QuotaWindow = {
  key: string;
  label: string;
  remaining: number | null;
  reset: number | null;
};
export type EnvironmentQuotaRow = {
  name: string;
  id?: string;
  representative?: boolean;
  unlimitedCredits?: boolean;
  windows: QuotaWindow[];
};

export function EnvironmentQuota({
  name,
  status,
  observed,
  rows,
  now,
  refresh,
  fetching,
  canRefresh,
}: {
  name: string;
  status: 'fresh' | 'pending' | 'error' | 'unsupported' | 'stale' | 'unknown';
  observed: number | null;
  rows: EnvironmentQuotaRow[];
  now: number;
  refresh: () => void;
  fetching: boolean;
  canRefresh: boolean;
}) {
  const { t, i18n } = useTranslation('common');
  const percent = (value: number | null) =>
    value === null ? '—' : `${Math.floor(value)}%`;
  const time = (value: number | null) =>
    value === null ? '—' : new Date(value).toLocaleString(i18n.language);
  const countdown = (reset: number | null) => {
    if (reset === null) return '—';
    const minutes = Math.ceil((reset - now) / 60000);
    if (minutes <= 0) return t('utilityPanel.resetElapsed');
    if (minutes >= 1440)
      return t('utilityPanel.durationDays', {
        days: Math.floor(minutes / 1440),
        hours: Math.floor((minutes % 1440) / 60),
      });
    if (minutes >= 60)
      return t('utilityPanel.durationHours', {
        hours: Math.floor(minutes / 60),
        minutes: minutes % 60,
      });
    return t('utilityPanel.durationMinutes', { minutes });
  };
  const resetPending = rows.some((row) =>
    row.windows.some((window) => window.reset !== null && window.reset <= now)
  );
  const exhausted = rows.some((row) =>
    row.windows.some((window) => window.remaining === 0)
  );
  const stateLabel =
    resetPending && (status === 'unknown' || status === 'fresh')
      ? t('utilityPanel.awaitingRefresh')
      : exhausted
        ? t('utilityPanel.resetPending')
        : t(`utilityPanel.quotaStatus.${status}`);
  const stateTone =
    status === 'fresh' && !resetPending && !exhausted
      ? 'text-success'
      : status === 'error' || exhausted
        ? 'text-error'
        : 'text-low';

  return (
    <div
      className="space-y-base border-t px-base py-plusfifty"
      data-testid={`environment-quota-${name.toLowerCase()}`}
    >
      <div className="flex items-center justify-between gap-half">
        <h3 className="font-medium text-high">{name}</h3>
        <div className="flex min-w-0 items-center gap-base">
          <span
            className={`min-w-0 break-words text-right text-xs ${stateTone}`}
            role="status"
          >
            {stateLabel}
          </span>
          <button
            type="button"
            onClick={refresh}
            disabled={!canRefresh}
            aria-label={`${name}: ${t('codexQuota.refresh')}`}
            title={t(
              fetching
                ? 'codexQuota.refreshing'
                : canRefresh
                  ? 'codexQuota.refresh'
                  : 'codexQuota.cooldown'
            )}
            className="rounded-sm p-half text-low hover:bg-panel hover:text-high disabled:opacity-40 focus-visible:ring-2 focus-visible:ring-brand"
          >
            <ArrowClockwiseIcon
              className={`size-icon-xs ${fetching ? 'motion-safe:animate-spin' : ''}`}
            />
          </button>
        </div>
      </div>
      {rows.length === 0 && (
        <p className="text-xs text-low">{t(`codexQuota.status.${status}`)}</p>
      )}
      {rows.map((row, index) => (
        <div key={`${row.id ?? row.name}-${index}`} className="space-y-base">
          {(rows.length > 1 || row.name) && (
            <p className="break-words text-xs text-low">
              {row.name ||
                (row.representative ? t('codexQuota.defaultBucket') : row.id)}
            </p>
          )}
          <dl className="grid grid-cols-2 gap-x-base gap-y-base">
            {row.windows.map((window) => (
              <div
                key={window.key}
                className="space-y-half"
                data-quota-window={window.key}
              >
                <div className="flex items-baseline justify-between gap-base">
                  <dt className="min-w-0 break-words text-xs text-normal">
                    {window.label}
                  </dt>
                  <dd
                    className={`shrink-0 font-medium tabular-nums ${resourceTone(window.remaining === null ? null : 100 - window.remaining)}`}
                  >
                    {percent(window.remaining)}
                  </dd>
                </div>
                <div className="flex items-center gap-base">
                  <div
                    aria-hidden="true"
                    className="h-0.5 min-w-0 flex-1 overflow-hidden rounded-full bg-panel"
                  >
                    {window.remaining !== null && (
                      <div
                        className={`h-full bg-current ${resourceTone(100 - window.remaining)}`}
                        style={{ width: `${window.remaining}%` }}
                      />
                    )}
                  </div>
                  <span
                    className="min-w-0 break-words text-right text-xs tabular-nums text-low"
                    title={time(window.reset)}
                    aria-label={t('utilityPanel.resetsIn', {
                      time: countdown(window.reset),
                    })}
                  >
                    {countdown(window.reset)}
                  </span>
                </div>
              </div>
            ))}
          </dl>
        </div>
      ))}
      <details className="group text-xs text-low">
        <summary className="flex w-fit cursor-pointer list-none items-center gap-half rounded-sm focus-visible:ring-2 focus-visible:ring-brand [&::-webkit-details-marker]:hidden">
          {t('utilityPanel.details')}
          <CaretDownIcon className="size-icon-2xs transition-transform group-open:rotate-180 motion-reduce:transition-none" />
        </summary>
        <div className="mt-base space-y-base border-l pl-base">
          <p>
            {t('codexQuota.statusLabel', {
              status: t(`codexQuota.status.${status}`),
            })}
          </p>
          <p>{t('codexQuota.observed', { time: time(observed) })}</p>
          <p>{t('utilityPanel.accountSource')}</p>
          {rows.map((row, index) => (
            <div key={index} className="space-y-half break-words">
              {row.representative !== undefined && (
                <p>
                  {t(
                    row.representative
                      ? 'codexQuota.defaultBucket'
                      : 'codexQuota.additionalBucket',
                    { index: index + 1 }
                  )}
                </p>
              )}
              {(row.name || row.id) && (
                <p>{[row.name, row.id].filter(Boolean).join(' / ')}</p>
              )}
              {row.windows.map((window) => (
                <p key={window.key}>
                  {t('codexQuota.window', {
                    kind: window.key,
                    period: window.label,
                    value: percent(window.remaining),
                    reset: time(window.reset),
                  })}
                </p>
              ))}
              {row.unlimitedCredits && (
                <p>{t('codexQuota.unlimitedCredits')}</p>
              )}
            </div>
          ))}
        </div>
      </details>
    </div>
  );
}
