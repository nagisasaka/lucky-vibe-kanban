import { CODEX_STALE_MS } from '@vibe/ui/lib/codexQuota';
const record = (value: unknown): Record<string, unknown> =>
  value !== null && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
export function claudeQuotaView(
  data: unknown,
  unavailable: boolean,
  now: number
) {
  const quota = record(record(record(data).result).quota);
  const observed =
    typeof quota.observedAt === 'string' ? Date.parse(quota.observedAt) : NaN;
  const fresh =
    !unavailable &&
    quota.status === 'fresh' &&
    Number.isFinite(observed) &&
    observed <= now &&
    now - observed < CODEX_STALE_MS;
  const payload = record(quota.data);
  const entries = [
    'five_hour',
    'seven_day',
    'seven_day_oauth_apps',
    'seven_day_opus',
    'seven_day_sonnet',
  ]
    .filter((key) => payload[key] != null)
    .map((key) => ({ key, label: '', window: record(payload[key]) }));
  if (Array.isArray(payload.model_scoped)) {
    payload.model_scoped.forEach((value, index) => {
      const window = record(value);
      const label =
        typeof window.display_name === 'string'
          ? window.display_name.replace(/[\p{Cc}\p{Cf}]/gu, '').slice(0, 120)
          : '';
      entries.push({ key: `model-${index}`, label, window });
    });
  }
  const windows = entries.map(({ key, label, window }) => {
    const reset =
      typeof window.resets_at === 'string' ? Date.parse(window.resets_at) : NaN;
    const used = window.utilization;
    return {
      key,
      label,
      reset: Number.isFinite(reset) ? reset : null,
      remaining:
        fresh &&
        typeof used === 'number' &&
        Number.isFinite(used) &&
        used >= 0 &&
        used <= 100 &&
        (window.resets_at == null || (Number.isFinite(reset) && reset > now))
          ? 100 - used
          : null,
    };
  });
  return { fresh, windows };
}
