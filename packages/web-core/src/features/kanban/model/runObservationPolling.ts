import type { PendingQuestion, RunObservation } from './runSpeechQueue';

export type RunObservationPage = {
  cursor: number;
  events: RunObservation[];
  pending: PendingQuestion[];
  active: string[];
};

/** A failed connection starts a fresh subscription instead of replaying its gap. */
export function startRunObservationPolling(
  initial: RunObservationPage,
  load: (
    cursor: number | undefined,
    signal: AbortSignal
  ) => Promise<RunObservationPage>,
  accept: (page: RunObservationPage, baseline: boolean) => void
) {
  let stopped = false;
  let cursor: number | undefined = initial.cursor;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const abort = new AbortController();
  accept(initial, true);
  async function poll() {
    try {
      const page = await load(cursor, abort.signal);
      if (stopped) return;
      const baseline = cursor === undefined;
      cursor = page.cursor;
      accept(page, baseline);
    } catch {
      cursor = undefined;
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
