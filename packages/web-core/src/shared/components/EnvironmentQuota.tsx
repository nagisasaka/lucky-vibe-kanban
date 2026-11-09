import { useTranslation } from 'react-i18next';
import { ArrowClockwiseIcon } from '@phosphor-icons/react';
import { resourceTone } from '@vibe/ui/lib/resourceObservation';

type QuotaWindow = {
  key: string;
  remaining: number | null;
  reset: number | null;
};
export type EnvironmentQuotaRow = {
  name: string;
  id?: string;
  representative?: boolean;
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
  reason,
}: {
  name: string;
  reason?: string | null;
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
    value === null || !Number.isFinite(value) || Math.abs(value) > 8.64e15
      ? '—'
      : new Date(value).toLocaleString(i18n.language);
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
  const observedAge =
    observed !== null &&
    Number.isFinite(observed) &&
    Math.abs(observed) <= 8.64e15 &&
    Number.isFinite(now) &&
    observed <= now
      ? Math.floor((now - observed) / 60000)
      : null;
  const fetchedLabel =
    observedAge === null
      ? t('utilityPanel.fetchedUnknown')
      : observedAge < 1
        ? t('utilityPanel.fetchedNow')
        : observedAge < 60
          ? t('utilityPanel.fetchedMinutes', { count: observedAge })
          : t('utilityPanel.fetchedHours', {
              count: Math.floor(observedAge / 60),
            });
  const resetPending = rows.some((row) =>
    row.windows.some((window) => window.reset !== null && window.reset <= now)
  );
  const exhausted = rows.some((row) =>
    row.windows.some((window) => window.remaining === 0)
  );
  const stateLabel =
    status !== 'fresh' && status !== 'unknown'
      ? t(`utilityPanel.quotaStatus.${status}`)
      : resetPending
        ? t('utilityPanel.awaitingRefresh')
        : exhausted
          ? t('utilityPanel.resetPending')
          : status === 'fresh'
            ? fetchedLabel
            : t(`utilityPanel.quotaStatus.${status}`);
  const stateTone =
    status === 'fresh' && !resetPending && !exhausted
      ? 'text-success'
      : status === 'error' || exhausted
        ? 'text-error'
        : 'text-low';

  return (
    <div
      className="space-y-half border-t px-base py-base"
      data-testid={`environment-quota-${name.toLowerCase()}`}
    >
      <div className="flex items-center justify-between gap-half">
        <h3 className="font-medium text-high">{name}</h3>
        <div className="flex min-w-0 items-center gap-base">
          <span
            className={`min-w-0 break-words text-right text-xs ${stateTone}`}
            role="status"
          >
            {status !== 'fresh' && observedAge !== null
              ? `${stateLabel} · ${fetchedLabel}`
              : stateLabel}
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
      {reason && (
        <p className="break-words text-xs text-low" data-testid="quota-reason">
          {reason}
        </p>
      )}
      {rows
        .filter((row) => row.windows.length > 0)
        .map((row, index) => (
          <div key={`${row.id ?? row.name}-${index}`} className="space-y-half">
            {(rows.length > 1 || row.name) && (
              <p className="break-words text-xs text-low">
                {row.name ||
                  (row.representative ? t('codexQuota.defaultBucket') : row.id)}
              </p>
            )}
            {row.windows.length > 0 && (
              <>
                <div
                  className="grid grid-cols-2 gap-x-base text-xs text-low"
                  aria-hidden="true"
                >
                  <span>{t('utilityPanel.quotaRemaining')}</span>
                  <span className="text-right">
                    {t('utilityPanel.resetCountdown')}
                  </span>
                </div>
                <dl className="space-y-half">
                  {row.windows.map((window, windowIndex) => (
                    <div
                      key={window.key}
                      data-quota-window={window.key}
                      className="grid min-w-0 grid-cols-2 items-center gap-x-base"
                    >
                      <div className="flex min-w-0 items-center gap-half">
                        <dt
                          className={
                            row.windows.length > 1
                              ? 'shrink-0 text-xs text-low'
                              : 'sr-only'
                          }
                        >
                          {t('utilityPanel.quotaIndex', {
                            index:
                              window.key === 'secondary' ? 2 : windowIndex + 1,
                          })}
                        </dt>
                        <dd
                          aria-label={t('utilityPanel.quotaRemaining')}
                          className={`shrink-0 font-medium tabular-nums ${resourceTone(window.remaining === null ? null : 100 - window.remaining)}`}
                        >
                          {percent(window.remaining)}
                        </dd>
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
                      </div>
                      <p
                        className="min-w-0 break-words text-right font-medium tabular-nums text-normal"
                        title={time(window.reset)}
                        aria-label={t('utilityPanel.resetsIn', {
                          time: countdown(window.reset),
                        })}
                      >
                        {countdown(window.reset)}
                      </p>
                    </div>
                  ))}
                </dl>
              </>
            )}
          </div>
        ))}
    </div>
  );
}
