import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { InfoIcon } from '@phosphor-icons/react';
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@vibe/ui/components/Popover';
import { resourceTone, resourceView } from '@vibe/ui/lib/resourceObservation';
import { quotaView } from '@vibe/ui/lib/codexQuota';
import { useResourceObservation } from '@/shared/hooks/useResourceObservation';
import { useCodexQuota } from '@/shared/hooks/useCodexQuota';
import { claudeQuotaView } from '@/shared/lib/claudeQuotaView';
import { readUtility } from '@/shared/lib/projectUtilitiesApi';
import { EnvironmentQuota } from './EnvironmentQuota';

type EnvironmentInfo = {
  hostname: string | null;
  container: boolean;
  uptime_seconds: number | null;
  running_sessions: number | null;
  observed_at: string;
};

export function EnvironmentPanel() {
  const { t, i18n } = useTranslation('common');
  const resources = useResourceObservation();
  const environment = useQuery({
    queryKey: ['environment-info'],
    queryFn: () => readUtility<EnvironmentInfo>('/api/environment-info'),
    enabled: resources.visible,
    refetchInterval: resources.visible ? 15000 : false,
    retry: false,
  });
  const observed = environment.data
    ? Date.parse(environment.data.observed_at)
    : NaN;
  const environmentFresh =
    !environment.isError &&
    resources.visible &&
    Number.isFinite(observed) &&
    observed <= resources.now + 5000 &&
    resources.now - observed < 45000 &&
    resources.now - environment.dataUpdatedAt < 45000;
  const view = resourceView(
    resources.data,
    resources.unavailable,
    resources.now
  );
  const codex = useCodexQuota(resources.visible);
  const claude = useCodexQuota(resources.visible, 'claude_code');
  const cv = quotaView(codex.data, codex.unavailable, resources.now);
  const cl = claudeQuotaView(claude.data, claude.unavailable, resources.now);
  const percent = (value: number | null) =>
    value === null ? '—' : `${Math.floor(value)}%`;
  const bytes = (value: number | null) =>
    value === null
      ? '—'
      : value === 0
        ? '0 B'
        : `${new Intl.NumberFormat(i18n.language, { maximumFractionDigits: 2 }).format(value / (value >= 2 ** 30 ? 2 ** 30 : 2 ** 20))} ${value >= 2 ** 30 ? 'GiB' : 'MiB'}`;
  const time = (value: number) =>
    Number.isFinite(value)
      ? new Date(value).toLocaleString(i18n.language)
      : '—';
  const count = (value: number | null | undefined) =>
    environmentFresh &&
    typeof value === 'number' &&
    Number.isFinite(value) &&
    value >= 0
      ? new Intl.NumberFormat(i18n.language).format(value)
      : '—';
  const periods: Record<string, string> = {
    five_hour: t('environment.five_hour'),
    seven_day: t('environment.seven_day'),
    seven_day_oauth_apps: t('environment.seven_day_oauth_apps'),
    seven_day_opus: t('environment.seven_day_opus'),
    seven_day_sonnet: t('environment.seven_day_sonnet'),
  };
  const metrics = [
    { label: 'CPU', value: view.cpu, used: null, total: null },
    { label: 'RAM', value: view.ram, ...view.memory },
    { label: 'SWAP', value: view.swap.percent, ...view.swap },
    { label: 'DISK', value: view.disk.percent, ...view.disk },
  ];

  return (
    <section
      aria-label={t('environment.title')}
      className="border-b bg-primary text-sm text-normal"
      data-testid="environment-panel"
    >
      <div className="flex items-center gap-base px-base py-plusfifty">
        <div
          data-pet-placeholder
          className="flex h-16 w-16 shrink-0 items-center justify-center"
        >
          <img src="/lvk-clover.svg" alt="LVK" className="h-12 w-12" />
        </div>
        <div className="min-w-0 flex-1 space-y-half">
          <p
            className="break-all font-medium leading-snug text-high"
            data-testid="environment-hostname"
          >
            {environment.data?.hostname ?? '—'}
          </p>
          <div className="flex flex-wrap items-center gap-x-base gap-y-half text-xs text-low">
            <span className="inline-flex items-center gap-half">
              <span
                aria-hidden="true"
                className={`size-1.5 rounded-full ${environmentFresh ? 'bg-success' : 'bg-panel'}`}
              />
              {t(
                environmentFresh
                  ? 'utilityPanel.responding'
                  : 'utilityPanel.unconfirmed'
              )}
            </span>
            <span
              title={t('environment.sessions')}
              data-testid="environment-sessions"
            >
              {t('utilityPanel.sessions', {
                value: count(environment.data?.running_sessions),
              })}
            </span>
          </div>
        </div>
        <Popover>
          <PopoverTrigger asChild>
            <button
              type="button"
              aria-label={t('utilityPanel.environmentDetails')}
              className="shrink-0 rounded-sm p-half text-low hover:bg-secondary hover:text-high focus-visible:ring-2 focus-visible:ring-brand"
            >
              <InfoIcon className="size-icon-sm" />
            </button>
          </PopoverTrigger>
          <PopoverContent
            align="end"
            collisionPadding={8}
            className="max-h-[70vh] max-w-[calc(100vw-1rem)] overflow-auto bg-primary text-xs text-normal"
            aria-label={t('utilityPanel.environmentDetails')}
          >
            <h3 className="mb-base font-medium text-high">
              {t('environment.title')}
            </h3>
            <dl className="space-y-base [&>div]:space-y-half [&_dt]:text-low [&_dd]:break-words">
              <div>
                <dt>{t('utilityPanel.status')}</dt>
                <dd>
                  {t(
                    environmentFresh
                      ? 'environment.online'
                      : 'environment.unavailable'
                  )}
                </dd>
              </div>
              <div>
                <dt>{t('utilityPanel.scope')}</dt>
                <dd>
                  {t(
                    environment.data?.container
                      ? 'utilityPanel.container'
                      : 'utilityPanel.scopeUnknown'
                  )}
                </dd>
              </div>
              <div>
                <dt>{t('environment.lastCheck')}</dt>
                <dd>
                  {time(observed)}
                  {Number.isFinite(observed) &&
                    ` (${t('environment.secondsAgo', { count: Math.max(0, Math.floor((resources.now - observed) / 1000)) })})`}
                </dd>
              </div>
              <div>
                <dt>{t('utilityPanel.uptime')}</dt>
                <dd>
                  {count(environment.data?.uptime_seconds)}{' '}
                  {environmentFresh && environment.data?.uptime_seconds != null
                    ? t('utilityPanel.seconds')
                    : ''}
                </dd>
              </div>
              <div>
                <dt>{t('environment.sessions')}</dt>
                <dd>{count(environment.data?.running_sessions)}</dd>
              </div>
            </dl>
          </PopoverContent>
        </Popover>
      </div>
      <div
        className="grid grid-cols-4 gap-half px-base pb-plusfifty"
        data-testid="environment-metrics"
      >
        {metrics.map((metric) => (
          <Popover key={metric.label}>
            <PopoverTrigger asChild>
              <button
                type="button"
                aria-label={t('utilityPanel.metricDetails', {
                  metric: metric.label,
                })}
                className="group min-w-0 rounded-sm px-half py-half text-left hover:bg-secondary focus-visible:ring-2 focus-visible:ring-brand"
              >
                <span className="flex items-baseline justify-between gap-half whitespace-nowrap">
                  <span className="text-xs text-low">{metric.label}</span>
                  <span
                    className={`text-xs font-medium tabular-nums ${resourceTone(metric.value)}`}
                  >
                    {percent(metric.value)}
                  </span>
                </span>
                <span
                  aria-hidden="true"
                  className="mt-half block h-0.5 overflow-hidden rounded-full bg-panel"
                >
                  {metric.value !== null && (
                    <span
                      className={`block h-full bg-current ${resourceTone(metric.value)}`}
                      style={{ width: `${metric.value}%` }}
                    />
                  )}
                </span>
              </button>
            </PopoverTrigger>
            <PopoverContent
              align="end"
              collisionPadding={8}
              className="max-w-[calc(100vw-1rem)] bg-primary text-xs text-normal"
              aria-label={t('utilityPanel.metricDetails', {
                metric: metric.label,
              })}
            >
              <h3 className="mb-base flex items-baseline justify-between font-medium text-high">
                <span>{metric.label}</span>
                <span className={resourceTone(metric.value)}>
                  {percent(metric.value)}
                </span>
              </h3>
              {metric.label !== 'CPU' ? (
                <dl className="space-y-half">
                  <div className="flex justify-between gap-base">
                    <dt className="text-low">{t('utilityPanel.used')}</dt>
                    <dd>{bytes(metric.used)}</dd>
                  </div>
                  <div className="flex justify-between gap-base">
                    <dt className="text-low">{t('utilityPanel.total')}</dt>
                    <dd>{bytes(metric.total)}</dd>
                  </div>
                </dl>
              ) : (
                <p>
                  {t('utilityPanel.sampleDuration', {
                    value:
                      !view.stale && view.duration !== null
                        ? new Intl.NumberFormat(i18n.language).format(
                            view.duration
                          )
                        : '—',
                  })}
                </p>
              )}
              <p className="mt-base text-low">
                {t(
                  metric.label === 'DISK'
                    ? 'utilityPanel.diskScope'
                    : 'utilityPanel.systemScope'
                )}
              </p>
              <p className="mt-half text-low">
                {t('utilityPanel.observed', {
                  time:
                    view.timestamp === 'unknown'
                      ? '—'
                      : time(Date.parse(view.timestamp)),
                })}
              </p>
            </PopoverContent>
          </Popover>
        ))}
      </div>
      <EnvironmentQuota
        name="Codex"
        status={cv.status}
        observed={cv.observed}
        now={resources.now}
        {...codex}
        rows={cv.rows.map((row) => ({
          ...row,
          windows: row.windows.map((window) => ({
            ...window,
            label: window.period
              ? t(`codexQuota.period.${window.period.unit}`, {
                  count: window.period.count,
                })
              : t('codexQuota.unknownPeriod'),
          })),
        }))}
      />
      <EnvironmentQuota
        name="Claude"
        status={cl.status}
        observed={cl.observed}
        now={resources.now}
        {...claude}
        rows={
          cl.windows.length
            ? [
                {
                  name: '',
                  windows: cl.windows.map((window) => ({
                    ...window,
                    label:
                      periods[window.key] ??
                      (window.label || t('codexQuota.unknownPeriod')),
                  })),
                },
              ]
            : []
        }
      />
    </section>
  );
}
