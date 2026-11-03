import { SidebarPet } from '@/features/pets/SidebarPet';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { resourceView } from '@vibe/ui/lib/resourceObservation';
import { quotaView } from '@vibe/ui/lib/codexQuota';
import { useResourceObservation } from '@/shared/hooks/useResourceObservation';
import { useCodexQuota } from '@/shared/hooks/useCodexQuota';
import { readUtility } from '@/shared/lib/projectUtilitiesApi';
import { claudeQuotaView } from '@/shared/lib/claudeQuotaView';

export function EnvironmentPanel() {
  const { t, i18n } = useTranslation('common');
  const resources = useResourceObservation();
  const view = resourceView(
    resources.data,
    resources.unavailable,
    resources.now
  );
  const env = useQuery({
    queryKey: ['environment-info'],
    queryFn: () =>
      readUtility<{
        hostname: string | null;
        container: boolean;
        uptime_seconds: number | null;
        running_sessions: number | null;
        observed_at: string;
      }>('/api/environment-info'),
    enabled: resources.visible,
    refetchInterval: resources.visible ? 15000 : false,
    retry: false,
  });
  const envFresh =
    !env.isError &&
    resources.visible &&
    resources.now - env.dataUpdatedAt < 45000;
  const codex = useCodexQuota(resources.visible);
  const claude = useCodexQuota(resources.visible, 'claude_code');
  const cv = quotaView(codex.data, codex.unavailable, resources.now);
  const cl = claudeQuotaView(claude.data, claude.unavailable, resources.now);
  const unknown = t('environment.unknown');
  const periods: Record<string, string> = {
    five_hour: t('environment.five_hour'),
    seven_day: t('environment.seven_day'),
    seven_day_oauth_apps: t('environment.seven_day_oauth_apps'),
    seven_day_opus: t('environment.seven_day_opus'),
    seven_day_sonnet: t('environment.seven_day_sonnet'),
  };

  const pct = (n: number | null) =>
    n === null ? unknown : `${Math.floor(n)}%`;
  const bytes = (n: number | null) =>
    n === null
      ? unknown
      : n === 0
        ? '0 B'
        : `${new Intl.NumberFormat(i18n.language, { maximumFractionDigits: 2 }).format(n / (n >= 2 ** 30 ? 2 ** 30 : 2 ** 20))} ${n >= 2 ** 30 ? 'GiB' : 'MiB'}`;
  const time = (n: number | null) =>
    n === null ? unknown : new Date(n).toLocaleString(i18n.language);
  const age = env.data
    ? Math.max(
        0,
        Math.floor((resources.now - Date.parse(env.data.observed_at)) / 1000)
      )
    : null;
  return (
    <section
      aria-label={t('environment.title')}
      className="relative border-b p-base space-y-base text-sm text-normal break-words"
      data-testid="environment-panel"
    >
      <div className="flex items-start gap-base">
        <SidebarPet />
        <div className="min-w-0">
          <h2 className="font-medium text-high">{t('environment.title')}</h2>
          <p className="break-all">{env.data?.hostname ?? unknown}</p>
          <p>
            {t(
              envFresh && env.data
                ? 'environment.online'
                : 'environment.unavailable'
            )}
          </p>
        </div>
      </div>
      <p>
        {t(
          env.data?.container
            ? 'environment.container'
            : 'environment.scopeUnknown'
        )}
      </p>
      <dl className="space-y-half [&>div]:grid [&>div]:grid-cols-[minmax(0,1fr)_minmax(0,1.4fr)] [&>div]:gap-half">
        <div>
          <dt>{t('environment.lastCheck')}</dt>
          <dd>
            {age !== null && Number.isFinite(age)
              ? t('environment.secondsAgo', { count: age })
              : unknown}
          </dd>
        </div>
        <div>
          <dt>{t('environment.uptime')}</dt>
          <dd>{unknown}</dd>
        </div>
        <div>
          <dt>{t('environment.cpu')}</dt>
          <dd>{pct(view.cpu)}</dd>
        </div>
        <div>
          <dt>{t('environment.ram')}</dt>
          <dd>
            {bytes(view.memory.used)} / {bytes(view.memory.total)} ·{' '}
            {pct(view.ram)}
          </dd>
        </div>
        <div>
          <dt>{t('environment.disk')}</dt>
          <dd>
            {bytes(view.disk.used)} / {bytes(view.disk.total)} ·{' '}
            {pct(view.disk.percent)}
          </dd>
        </div>
        <div>
          <dt>{t('environment.swap')}</dt>
          <dd>
            {bytes(view.swap.used)} / {bytes(view.swap.total)} ·{' '}
            {pct(view.swap.percent)}
          </dd>
        </div>
        <div>
          <dt>{t('environment.sessions')}</dt>
          <dd>
            {envFresh ? (env.data?.running_sessions ?? unknown) : unknown}
          </dd>
        </div>
      </dl>
      <p className="text-xs text-low">{t('environment.metricsScope')}</p>
      <div className="border-t pt-base space-y-half">
        <p>{t('environment.accountScope')}</p>
        <p>
          {t('environment.codex')} · {t(`codexQuota.status.${cv.status}`)}
        </p>
        {cv.rows.map((row, index) => (
          <div key={index}>
            <p>{row.name || row.id || t('codexQuota.defaultBucket')}</p>
            {row.windows.map((w) => (
              <p key={w.key}>
                {t('codexQuota.window', {
                  kind: w.key,
                  period: w.period
                    ? t(`codexQuota.period.${w.period.unit}`, {
                        count: w.period.count,
                      })
                    : t('codexQuota.unknownPeriod'),
                  value: pct(w.remaining),
                  reset: time(w.reset),
                })}
              </p>
            ))}
          </div>
        ))}
        <p>
          {t('environment.claude')} ·{' '}
          {cl.fresh
            ? t('codexQuota.status.fresh')
            : t('environment.unavailable')}
        </p>
        {cl.windows.map((w) => (
          <p key={w.key}>
            {t('environment.quotaWindow', {
              period: periods[w.key] ?? (w.label || unknown),
              value: pct(w.remaining),
              reset: time(w.reset),
            })}
          </p>
        ))}
      </div>
    </section>
  );
}
