import { afterEach, describe, expect, it, vi } from 'vitest';
import {
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
