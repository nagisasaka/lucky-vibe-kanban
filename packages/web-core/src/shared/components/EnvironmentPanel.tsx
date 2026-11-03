import { useTranslation } from 'react-i18next';
import { resourceTone, resourceView } from '@vibe/ui/lib/resourceObservation';
import { quotaView } from '@vibe/ui/lib/codexQuota';
import { useResourceObservation } from '@/shared/hooks/useResourceObservation';
import { useCodexQuota } from '@/shared/hooks/useCodexQuota';
import { claudeQuotaView } from '@/shared/lib/claudeQuotaView';

export function EnvironmentPanel() {
  const { t } = useTranslation('common');
  const resources = useResourceObservation();
  const view = resourceView(
    resources.data,
    resources.unavailable,
    resources.now
  );
  const codex = useCodexQuota(resources.visible);
  const claude = useCodexQuota(resources.visible, 'claude_code');
  const cv = quotaView(codex.data, codex.unavailable, resources.now);
  const cl = claudeQuotaView(claude.data, claude.unavailable, resources.now);
  const pct = (value: number | null) =>
    value === null ? '—' : `${Math.floor(value)}%`;
  const periods: Record<string, string> = {
    five_hour: t('environment.five_hour'),
    seven_day: t('environment.seven_day'),
    seven_day_oauth_apps: t('environment.seven_day_oauth_apps'),
    seven_day_opus: t('environment.seven_day_opus'),
    seven_day_sonnet: t('environment.seven_day_sonnet'),
  };
  const metrics = [
    { label: 'CPU', value: view.cpu },
    { label: 'RAM', value: view.ram },
    { label: 'SWAP', value: view.swap.percent },
    { label: 'DISK', value: view.disk.percent },
  ];

  return (
    <section
      aria-label={t('environment.title')}
      className="border-b text-sm text-normal"
      data-testid="environment-panel"
    >
      <dl className="grid grid-cols-4 gap-half p-base text-xs">
        {metrics.map(({ label, value }) => (
          <div key={label} className="flex min-w-0 items-baseline gap-half">
            <dt className="text-xs text-low">{label}</dt>
            <dd className={`font-medium tabular-nums ${resourceTone(value)}`}>
              {pct(value)}
            </dd>
          </div>
        ))}
      </dl>
      <div className="border-t p-base space-y-base">
        <div className="space-y-half">
          <h3 className="text-xs font-medium text-high">
            {t('environment.codex')}
          </h3>
          {cv.rows.length === 0 && <p className="text-low">—</p>}
          {cv.rows.map((row, index) => (
            <div key={index} className="space-y-half">
              {cv.rows.length > 1 && (
                <p className="text-xs text-low break-words">
                  {row.name || row.id || t('codexQuota.defaultBucket')}
                </p>
              )}
              <dl className="space-y-half">
                {row.windows.map((window) => (
                  <div
                    key={window.key}
                    className="flex items-center justify-between gap-base"
                  >
                    <dt className="text-low">
                      {window.period
                        ? t(`codexQuota.period.${window.period.unit}`, {
                            count: window.period.count,
                          })
                        : t('codexQuota.unknownPeriod')}
                    </dt>
                    <dd className="font-medium tabular-nums">
                      {pct(window.remaining)}
                    </dd>
                  </div>
                ))}
              </dl>
            </div>
          ))}
        </div>
        <div className="space-y-half">
          <h3 className="text-xs font-medium text-high">
            {t('environment.claude')}
          </h3>
          {cl.windows.length === 0 && <p className="text-low">—</p>}
          <dl className="space-y-half">
            {cl.windows.map((window) => (
              <div
                key={window.key}
                className="flex items-center justify-between gap-base"
              >
                <dt className="text-low">
                  {periods[window.key] ?? (window.label || '—')}
                </dt>
                <dd className="font-medium tabular-nums">
                  {pct(window.remaining)}
                </dd>
              </div>
            ))}
          </dl>
        </div>
      </div>
    </section>
  );
}
