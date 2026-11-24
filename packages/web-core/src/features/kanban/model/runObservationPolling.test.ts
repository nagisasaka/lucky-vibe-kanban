import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  createIssueRefreshListener,
  RunObservationHub,
  startRunObservationPolling,
  type RunObservationPage,
} from './runObservationPolling';

const page = (cursor: number): RunObservationPage => ({
  cursor,
  events: [],
  pending: [],
  active: [],
});

afterEach(() => vi.useRealTimers());

describe('run observation polling', () => {
  it('subscribes at the baseline, rebaselines after an error and stops without replaying', async () => {
    vi.useFakeTimers();
    const load = vi
      .fn()
      .mockResolvedValueOnce(page(11))
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValueOnce(page(20))
      .mockResolvedValueOnce(page(21));
    const accept = vi.fn();
    const stop = startRunObservationPolling(page(10), load, accept);
    await vi.waitFor(() => expect(accept).toHaveBeenCalledTimes(2));
    expect(load.mock.calls[0][0]).toBe(10);
    await vi.advanceTimersByTimeAsync(1000);
    expect(load.mock.calls[1][0]).toBe(11);
    await vi.advanceTimersByTimeAsync(1000);
    expect(load.mock.calls[2][0]).toBeUndefined();
    expect(accept).toHaveBeenNthCalledWith(3, page(20), true);
    await vi.advanceTimersByTimeAsync(1000);
    expect(load.mock.calls[3][0]).toBe(20);
    expect(accept).toHaveBeenNthCalledWith(4, page(21), false);
    stop();
    await vi.advanceTimersByTimeAsync(5000);
    expect(load).toHaveBeenCalledTimes(4);
  });
  it('ignores a response that arrives after stopping', async () => {
    let resolve!: (value: RunObservationPage) => void;
    const accept = vi.fn();
    const load = vi.fn(
      (_cursor: number | undefined, _signal: AbortSignal) =>
        new Promise<RunObservationPage>((done) => {
          resolve = done;
        })
    );
    const stop = startRunObservationPolling(page(10), load, accept);
    const signal = load.mock.calls[0][1] as AbortSignal;
    stop();
    expect(signal.aborted).toBe(true);
    resolve(page(11));
    await Promise.resolve();
    expect(accept).toHaveBeenCalledTimes(1);
  });
});

const lifecycle = (
  run: string,
  status: string,
  attempt = 1
): RunObservationPage['events'][number] =>
  ({
    run_id: run,
    attempt,
    issue: 'LOCAL-1',
    payload: { type: 'lifecycle_changed', data: { status } },
  }) as RunObservationPage['events'][number];

const withEvents = (
  cursor: number,
  events: RunObservationPage['events']
): RunObservationPage => ({ ...page(cursor), events });

describe('shared run observation hub', () => {
  it('polls once for speech and board, joins late listeners at the latest cursor and stops when unobserved', async () => {
    vi.useFakeTimers();
    const pages = [
      page(10),
      withEvents(11, [lifecycle('a', 'running')]),
      page(11),
      page(11),
    ];
    const load = vi.fn(async () => pages.shift() ?? page(11));
    const hub = new RunObservationHub('project', load);
    expect(load).not.toHaveBeenCalled();
    const board = vi.fn();
    const stopBoard = hub.subscribe({ page: board });
    await vi.waitFor(() => expect(board).toHaveBeenCalledWith(page(10), true));
    expect(load.mock.calls[0][0]).toBeUndefined();
    await vi.advanceTimersByTimeAsync(1000);
    expect(board).toHaveBeenLastCalledWith(
      withEvents(11, [lifecycle('a', 'running')]),
      false
    );
    // Speech joining does not start a second subscription or replay events.
    const speech = vi.fn();
    const stopSpeech = hub.subscribe({ page: speech });
    expect(speech).toHaveBeenCalledWith(page(11), true);
    await vi.advanceTimersByTimeAsync(1000);
    expect(load).toHaveBeenCalledTimes(3);
    expect(load.mock.calls[2][0]).toBe(11);
    expect(speech).toHaveBeenLastCalledWith(page(11), false);
    stopBoard();
    await vi.advanceTimersByTimeAsync(1000);
    expect(load).toHaveBeenCalledTimes(4);
    stopSpeech();
    await vi.advanceTimersByTimeAsync(5000);
    expect(load).toHaveBeenCalledTimes(4);
    // A new consumer starts from a fresh baseline instead of the old cursor.
    const again = vi.fn();
    const stopAgain = hub.subscribe({ page: again });
    await vi.waitFor(() => expect(load).toHaveBeenCalledTimes(5));
    expect(load.mock.calls[4][0]).toBeUndefined();
    stopAgain();
  });

  it('reports a failed connection only to listeners still waiting for a baseline', async () => {
    vi.useFakeTimers();
    const load = vi
      .fn()
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValueOnce(page(20))
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValue(page(30));
    const hub = new RunObservationHub('project', load);
    const speechError = vi.fn();
    const stopSpeech = hub.subscribe({ page: vi.fn(), error: speechError });
    await vi.waitFor(() => expect(speechError).toHaveBeenCalledTimes(1));
    stopSpeech();
    const board = { page: vi.fn(), error: vi.fn() };
    // Polling is idle once unobserved, so the board restarts it.
    const stopBoard = hub.subscribe(board);
    await vi.waitFor(() =>
      expect(board.page).toHaveBeenCalledWith(page(20), true)
    );
    await vi.advanceTimersByTimeAsync(1000);
    expect(board.error).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1000);
    expect(board.page).toHaveBeenLastCalledWith(page(30), true);
    stopBoard();
  });
});

describe('issue refresh from run observations', () => {
  it('refreshes once per page after Issue-changing transitions only', () => {
    const refresh = vi.fn();
    const listener = createIssueRefreshListener(refresh);
    listener.page(page(10), true);
    expect(refresh).not.toHaveBeenCalled();
    listener.page(withEvents(11, [lifecycle('a', 'starting')]), false);
    listener.page(
      withEvents(12, [
        {
          ...lifecycle('a', 'running'),
          payload: { type: 'message', data: {} },
        },
      ]),
      false
    );
    expect(refresh).not.toHaveBeenCalled();
    listener.page(
      withEvents(13, [
        lifecycle('a', 'running'),
        lifecycle('b', 'running'),
        lifecycle('c', 'succeeded'),
      ]),
      false
    );
    expect(refresh).toHaveBeenCalledTimes(1);
    // Resuming after a question is not a new start of the same attempt.
    listener.page(withEvents(14, [lifecycle('a', 'awaiting_input')]), false);
    listener.page(withEvents(15, [lifecycle('a', 'running')]), false);
    expect(refresh).toHaveBeenCalledTimes(1);
    listener.page(withEvents(16, [lifecycle('a', 'running', 2)]), false);
    expect(refresh).toHaveBeenCalledTimes(2);
    listener.page(withEvents(17, [lifecycle('a', 'failed', 2)]), false);
    expect(refresh).toHaveBeenCalledTimes(3);
  });

  it('refreshes after a later baseline that may have skipped transitions', () => {
    const refresh = vi.fn();
    const listener = createIssueRefreshListener(refresh);
    listener.page(page(10), true);
    listener.page(page(20), true);
    expect(refresh).toHaveBeenCalledTimes(1);
    const returning = vi.fn();
    createIssueRefreshListener(returning, {
      refreshOnFirstBaseline: true,
    }).page(page(30), true);
    expect(returning).toHaveBeenCalledTimes(1);
  });
});
