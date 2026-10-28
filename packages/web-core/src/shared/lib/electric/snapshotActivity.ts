/** Frontend-only freshness: cached rows never count as a successful request. */
export class SnapshotActivity {
  state = { sequence: 0, startedAt: 0, healthy: false };
  pending = false;
  begin() {
    this.pending = true;
  }
  private listeners = new Set<() => void>();

  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  report(startedAt: number, healthy: boolean) {
    this.pending = false;
    this.state = { sequence: this.state.sequence + 1, startedAt, healthy };
    this.listeners.forEach((listener) => listener());
  }
}
const activities = new Map<string, SnapshotActivity>();
export function snapshotActivity(key: string) {
  let activity = activities.get(key);
  if (!activity) {
    activity = new SnapshotActivity();
    activities.set(key, activity);
  }
  return activity;
}

/** A recovery snapshot establishes a baseline; only later snapshots may move it. */
export class SnapshotMotionGate {
  private after = -Infinity;
  private waiting = true;
  private silentSequence = 0;

  suspend(now: number) {
    this.after = now;
    this.waiting = true;
  }
  observe(state: SnapshotActivity['state']) {
    if (!state.healthy) {
      this.waiting = true;
      this.after = Math.max(this.after, state.startedAt);
    } else if (this.waiting && state.startedAt > this.after) {
      this.waiting = false;
      this.silentSequence = state.sequence;
    }
  }
  allows(state: SnapshotActivity['state']) {
    return (
      state.healthy && !this.waiting && state.sequence > this.silentSequence
    );
  }
}
