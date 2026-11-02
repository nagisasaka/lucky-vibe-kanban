import i18n from '@/i18n/config';
import { formatRelativeTime as formatAge } from '@vibe/ui/lib/dateFormatting';

/** Short calendar date/time in the selected UI locale; keep the local timezone. */
export function formatDateShortWithTime(dateString: string): string {
  const date = new Date(dateString);
  if (!Number.isFinite(date.getTime())) return '—';
  return date.toLocaleDateString(i18n.resolvedLanguage || i18n.language, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

export function formatRelativeTime(dateString: string): string {
  return formatAge(dateString, i18n.resolvedLanguage || i18n.language);
}

/** SQLite datetime('now','subsec') values are UTC even without a zone suffix. */
export function formatUtcDateTime(value: string, locale: string): string {
  const normalized = /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}(?:\.\d+)?$/.test(
    value
  )
    ? `${value.replace(' ', 'T')}Z`
    : value;
  const date = new Date(normalized);
  if (!Number.isFinite(date.getTime())) return '—';
  return date.toLocaleString(locale, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    fractionalSecondDigits: 3,
    timeZone: 'UTC',
    timeZoneName: 'short',
  });
}
