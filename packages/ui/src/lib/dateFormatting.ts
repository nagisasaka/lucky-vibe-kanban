/** Localized relative age, retaining the existing minute/hour/day boundaries. */
export function formatRelativeTime(
  dateString: string,
  locale: string,
  now = Date.now()
): string {
  const timestamp = Date.parse(dateString);
  if (!Number.isFinite(timestamp)) return '—';
  const minutes = Math.floor((now - timestamp) / 60_000);
  const formatter = new Intl.RelativeTimeFormat(locale, {
    numeric: 'always',
    style: 'narrow',
  });
  if (minutes < 1)
    return new Intl.RelativeTimeFormat(locale, { numeric: 'auto' }).format(
      0,
      'second'
    );
  if (minutes < 60) return formatter.format(-minutes, 'minute');
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return formatter.format(-hours, 'hour');
  return formatter.format(-Math.floor(hours / 24), 'day');
}
