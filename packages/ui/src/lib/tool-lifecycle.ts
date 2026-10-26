export interface ToolLifecycle {
  status?: { status: string };
  active?: boolean;
}

export function isToolActive(entry: ToolLifecycle) {
  return (
    entry.active ??
    (entry.status?.status === 'created' ||
      entry.status?.status === 'waiting_approval' ||
      entry.status?.status === 'pending_approval')
  );
}

export function toolDisplayStatus(entry: ToolLifecycle) {
  const status = entry.status?.status;
  if (status === 'pending_approval' || status === 'waiting_approval')
    return entry.active === false ? 'unknown' : 'pending_approval';
  if (status === 'created') return isToolActive(entry) ? 'running' : 'unknown';
  if (
    status &&
    [
      'success',
      'failed',
      'denied',
      'timed_out',
      'cancelled',
      'approved',
    ].includes(status)
  )
    return status;
  return isToolActive(entry) ? 'running' : 'unknown';
}

/** An aggregate must never turn a non-successful observation into success. */
export function aggregateToolStatus(entries: ToolLifecycle[]) {
  for (const status of ['failed', 'denied', 'timed_out', 'cancelled']) {
    const exceptional = entries.find(
      (entry) => entry.status?.status === status
    );
    if (exceptional) return exceptional.status;
  }
  const active = entries.find(isToolActive);
  if (active) return active.status;
  if (
    entries.length > 0 &&
    entries.every((entry) => entry.status?.status === 'success')
  ) {
    return { status: 'success' };
  }
  return { status: 'unknown' };
}
