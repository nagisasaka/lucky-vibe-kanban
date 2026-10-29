export const CODEX_POLL_MS = 5 * 60_000;
export const CODEX_STALE_MS = 10 * 60_000;
type RecordValue = Record<string, unknown>;
const record = (v: unknown): RecordValue =>
  v !== null && typeof v === 'object' && !Array.isArray(v)
    ? (v as RecordValue)
    : {};
export function quotaView(data: unknown, unavailable: boolean, now: number) {
  const result = record(record(data).result);
  const quota = record(result.quota);
  const observed =
    typeof quota.observedAt === 'string' ? Date.parse(quota.observedAt) : NaN;
  const fresh =
    !unavailable &&
    quota.status === 'fresh' &&
    Number.isFinite(observed) &&
    observed <= now &&
    now - observed < CODEX_STALE_MS;
  const payload = record(quota.data);
  const defaultBucket = record(payload.rateLimits);
  const hasDefault = Object.keys(defaultBucket).length > 0;
  const seen = new Set<string>();
  if (typeof defaultBucket.limitId === 'string')
    seen.add(defaultBucket.limitId);
  const others = Object.entries(record(payload.rateLimitsByLimitId))
    .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
    .flatMap(([id, value]) => {
      const bucket = record(value);
      const limitId = typeof bucket.limitId === 'string' ? bucket.limitId : id;
      if (seen.has(limitId)) return [];
      seen.add(limitId);
      return [{ ...bucket, limitId }];
    });
  // Only the API-designated default bucket is representative.
  const buckets: RecordValue[] = hasDefault
    ? [defaultBucket, ...others]
    : others;
  const rows = buckets.map((bucket, index) => {
    const windows = ['primary', 'secondary'].map((key) => {
      const w = record(bucket[key]);
      const reset =
        typeof w.resetsAt === 'number' &&
        Number.isFinite(w.resetsAt) &&
        Math.abs(w.resetsAt) <= 8.64e12
          ? w.resetsAt * 1000
          : null;
      const used = w.usedPercent;
      const remaining =
        fresh &&
        payload.ordinaryUsageAllowed !== false &&
        typeof used === 'number' &&
        Number.isFinite(used) &&
        used >= 0 &&
        used <= 100 &&
        (w.resetsAt == null || reset !== null) &&
        (reset === null || reset > now)
          ? 100 - used
          : null;
      const mins = w.windowDurationMins;
      const period =
        typeof mins === 'number' && Number.isFinite(mins) && mins > 0
          ? mins % 1440 === 0
            ? { unit: 'days', count: mins / 1440 }
            : mins % 60 === 0
              ? { unit: 'hours', count: mins / 60 }
              : { unit: 'minutes', count: mins }
          : null;
      return { key, period, remaining, reset };
    });
    // React escapes text; bound length and remove control/bidi characters. Never expose account fields.
    const publicText = (value: unknown) =>
      typeof value === 'string'
        ? value
            .replace(
              // eslint-disable-next-line no-control-regex -- Strip provider label control characters.
              /[\u0000-\u001f\u007f-\u009f\u202a-\u202e\u2066-\u2069]/g,
              ''
            )
            .slice(0, 120)
        : '';
    return {
      representative: hasDefault && index === 0,
      name: publicText(bucket.limitName),
      id: publicText(bucket.limitId),
      index: index + 1,
      windows,
      unlimitedCredits: record(bucket.credits).unlimited === true,
    };
  });
  const values =
    rows[0]?.windows
      .filter((w) => w.remaining !== null)
      .map((w) => w.remaining as number) ?? [];
  // Unknown windows cannot be treated as unlimited or fully available.
  const present =
    rows[0] &&
    ['primary', 'secondary'].filter((k) => buckets[0][k] != null).length;
  const remaining =
    hasDefault && values.length && values.length === present
      ? Math.min(...values)
      : null;
  const status = unavailable
    ? 'error'
    : !data
      ? 'pending'
      : result.status === 'unsupported' || quota.status === 'unsupported'
        ? 'unsupported'
        : !fresh
          ? 'stale'
          : remaining === null
            ? 'unknown'
            : 'fresh';
  return {
    remaining,
    rows,
    status,
    observed: Number.isFinite(observed) ? observed : null,
  };
}
