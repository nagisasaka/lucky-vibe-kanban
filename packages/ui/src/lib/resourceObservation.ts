// The shared observer returns extensible JSON. Validate only the displayed
// fields; absent, unlimited and malformed values must never become zero.
export const RESOURCE_POLL_MS = 15_000;
export const RESOURCE_STALE_MS = 45_000;

type RecordValue = Record<string, unknown>;
const record = (v: unknown): RecordValue =>
  v !== null && typeof v === 'object' ? (v as RecordValue) : {};
const number = (v: unknown): number | null =>
  typeof v === 'number' && Number.isFinite(v) && v >= 0 ? v : null;
const known = (v: unknown) => {
  const f = record(v);
  return f.status === 'known' ? number(f.value) : null;
};
const percent = (v: unknown) => {
  const n = number(v);
  return n !== null && n <= 100 ? n : null;
};
export function resourceTone(value: number | null) {
  return value === null
    ? 'text-low'
    : value >= 80
      ? 'text-error'
      : value >= 70
        ? 'text-orange-500'
        : 'text-success';
}
export function resourceView(data: unknown, unavailable: boolean, now: number) {
  const root = record(data);
  const timestamp =
    typeof root.timestamp === 'string' ? Date.parse(root.timestamp) : NaN;
  const stale =
    unavailable ||
    !Number.isFinite(timestamp) ||
    now - timestamp >= RESOURCE_STALE_MS ||
    timestamp > now + 5000;
  const system = record(root.system_visible);
  const total = known(system.memory_total_bytes);
  const available = known(system.memory_available_bytes);
  const ram =
    total !== null && total > 0 && available !== null && available <= total
      ? ((total - available) / total) * 100
      : null;
  const disks = Array.isArray(root.disks) ? root.disks : [];
  const matches = disks.filter((d) => record(d).target === 'worktree_base');
  const observation = record(
    record(matches.length === 1 ? matches[0] : null).observation
  );
  const disk = record(
    observation.status === 'known' ? observation.value : null
  );
  const diskTotal = number(disk.total_bytes);
  const diskUsed = number(disk.used_bytes);
  const diskPercent = percent(disk.used_percent);
  const validDisk =
    !stale &&
    diskTotal !== null &&
    diskTotal > 0 &&
    diskUsed !== null &&
    diskUsed <= diskTotal &&
    diskPercent !== null &&
    Math.abs(diskPercent - (diskUsed / diskTotal) * 100) < 0.000001;
  return {
    stale,
    timestamp: Number.isFinite(timestamp)
      ? new Date(timestamp).toISOString()
      : 'unknown',
    cpu: stale ? null : percent(known(record(system.cpu_usage).percent)),
    duration: number(record(system.cpu_usage).sampling_duration_ms),
    ram: stale ? null : ram,
    disk: {
      percent: validDisk ? diskPercent : null,
      used: validDisk ? diskUsed : null,
      total: validDisk ? diskTotal : null,
    },
  };
}
