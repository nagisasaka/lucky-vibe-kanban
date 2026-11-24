import type { PendingQuestion, RunObservation } from './runSpeechQueue';

export type RunObservationPage = {
  cursor: number;
  events: RunObservation[];
  pending: PendingQuestion[];
  active: string[];
};

/** A failed connection starts a fresh subscription instead of replaying its gap. */
export function startRunObservationPolling(
  initial: RunObservationPage | undefined,
  load: (
    cursor: number | undefined,
    signal: AbortSignal
  ) => Promise<RunObservationPage>,
  accept: (page: RunObservationPage, baseline: boolean) => void,
  fail?: () => void
) {
  let stopped = false;
  let cursor: number | undefined = initial?.cursor;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const abort = new AbortController();
  if (initial) accept(initial, true);
  async function poll() {
    try {
      const page = await load(cursor, abort.signal);
      if (stopped) return;
      const baseline = cursor === undefined;
      cursor = page.cursor;
      accept(page, baseline);
    } catch {
      cursor = undefined;
      if (!stopped) fail?.();
    }
    if (!stopped) timer = setTimeout(poll, 1000);
  }
  void poll();
  return () => {
    stopped = true;
    abort.abort();
    if (timer) clearTimeout(timer);
  };
}

export type RunObservationListener = {
  page: (page: RunObservationPage, baseline: boolean) => void;
  /** Called when polling fails before this listener received any page. */
  error?: () => void;
};

/**
 * One cursor subscription per board Project, shared by speech and card refresh.
 * Polling runs only while at least one consumer listens. A late consumer joins
 * at the latest delivered cursor and never receives earlier events.
 */
export class RunObservationHub {
  private listeners = new Set<RunObservationListener>();
  private waiting = new Set<RunObservationListener>();
  private last: RunObservationPage | null = null;
  private stop: (() => void) | null = null;

  constructor(
    readonly projectId: string,
    private readonly load: (
      cursor: number | undefined,
      signal: AbortSignal
    ) => Promise<RunObservationPage>
  ) {}

  subscribe(listener: RunObservationListener) {
    this.listeners.add(listener);
    if (this.last) listener.page({ ...this.last, events: [] }, true);
    else this.waiting.add(listener);
    if (!this.stop) {
      this.stop = startRunObservationPolling(
        undefined,
        this.load,
        (page, baseline) => this.publish(page, baseline),
        () => this.fail()
      );
    }
    return () => {
      this.listeners.delete(listener);
      this.waiting.delete(listener);
      if (this.listeners.size || !this.stop) return;
      this.stop();
      this.stop = null;
      this.last = null;
    };
  }

  private publish(page: RunObservationPage, baseline: boolean) {
    this.last = page;
    this.waiting.clear();
    for (const listener of [...this.listeners]) {
      if (this.listeners.has(listener)) listener.page(page, baseline);
    }
  }

  private fail() {
    // The next successful page re-establishes a baseline for every listener.
    this.last = null;
    const waiting = [...this.waiting];
    this.waiting.clear();
    for (const listener of waiting) listener.error?.();
  }
}

const ISSUE_STATUS_TERMINALS = [
  'succeeded',
  'failed',
  'cancelled',
  'crashed',
  'audit_failed',
];

/**
 * The server moves a card in the same transaction that persists the Run's
 * `running` or terminal lifecycle event, so either event proves the status is
 * readable. Refresh at most once per page; an initial baseline needs no
 * refresh, while a later baseline may have skipped transitions.
 */
export function createIssueRefreshListener(
  refresh: () => void,
  { refreshOnFirstBaseline = false } = {}
): RunObservationListener {
  let connected = false;
  const running = new Set<string>();
  return {
    page(page, baseline) {
      if (baseline) {
        const recovered = connected || refreshOnFirstBaseline;
        connected = true;
        running.clear();
        if (recovered) refresh();
        return;
      }
      let changed = false;
      for (const { run_id, attempt, payload } of page.events) {
        if (payload.type !== 'lifecycle_changed') continue;
        const status = payload.data.status;
        const key = `${run_id}:${attempt}`;
        if (status === 'running' && !running.has(key)) {
          running.add(key);
          changed = true;
        } else if (
          typeof status === 'string' &&
          ISSUE_STATUS_TERMINALS.includes(status)
        ) {
          running.delete(key);
          changed = true;
        }
      }
      if (changed) refresh();
    },
  };
}
