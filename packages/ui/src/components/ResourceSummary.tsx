import { useTranslation } from 'react-i18next';
import {
  resourceTone,
  resourceView,
  formatDiskCapacity as space,
} from '../lib/resourceObservation';
import { Tooltip } from './Tooltip';

function percentage(value: number | null, locale: string) {
  // Floor avoids displaying 70/80 in the preceding colour band.
  return value === null
    ? '—'
    : new Intl.NumberFormat(locale, {
        style: 'percent',
        maximumFractionDigits: 0,
      }).format(Math.floor(value) / 100);
}

export function ResourceSummary({
  data,
  unavailable,
  now,
}: {
  data: unknown;
  unavailable: boolean;
  now: number;
}) {
  const { t, i18n } = useTranslation('common');
  const locale = i18n.resolvedLanguage ?? 'en';
  const view = resourceView(data, unavailable, now);
  const unknown = t('resourceSummary.unknown');
  // The observer previously displayed an ISO timestamp (UTC). Retain that zone.
  const timestamp =
    view.timestamp === 'unknown'
      ? unknown
      : new Intl.DateTimeFormat(locale, {
          year: 'numeric',
          month: '2-digit',
          day: '2-digit',
          hour: '2-digit',
          minute: '2-digit',
          second: '2-digit',
          fractionalSecondDigits: 3,
          hour12: false,
          timeZone: 'UTC',
          timeZoneName: 'short',
        }).format(new Date(view.timestamp));
  const stale = view.stale ? t('resourceSummary.stale') : '';
  const cpu = percentage(view.cpu, locale);
  const ram = percentage(view.ram, locale);
  const disk = percentage(view.disk.percent, locale);
  const sample =
    view.duration === null
      ? unknown
      : t('resourceSummary.milliseconds', {
          value: new Intl.NumberFormat(locale).format(
            Math.round(view.duration)
          ),
        });
  const scope = t('resourceSummary.scope', { stale, timestamp });
  return (
    <div
      className="flex max-w-full shrink-0 flex-wrap items-center gap-base text-xs"
      aria-label={t('resourceSummary.label')}
      data-testid="resource-summary"
    >
      <Tooltip
        className="max-w-[min(24rem,calc(100vw-1rem))] whitespace-normal"
        content={t('resourceSummary.cpu', { value: cpu, scope, sample })}
      >
        <span
          tabIndex={0}
          className={resourceTone(view.cpu)}
          data-testid="resource-cpu"
        >
          CPU {cpu}
        </span>
      </Tooltip>
      <Tooltip
        className="max-w-[min(24rem,calc(100vw-1rem))] whitespace-normal"
        content={t('resourceSummary.ram', { value: ram, scope })}
      >
        <span
          tabIndex={0}
          className={resourceTone(view.ram)}
          data-testid="resource-ram"
        >
          RAM {ram}
        </span>
      </Tooltip>
      <Tooltip
        className="max-w-[min(24rem,calc(100vw-1rem))] whitespace-normal"
        content={t('resourceSummary.disk', { stale, timestamp })}
      >
        <span
          tabIndex={0}
          className={resourceTone(view.disk.percent)}
          data-testid="resource-disk"
        >
          {t('resourceSummary.diskLabel')} {disk} (
          {space(view.disk.used, locale)} / {space(view.disk.total, locale)})
        </span>
      </Tooltip>
    </div>
  );
}
