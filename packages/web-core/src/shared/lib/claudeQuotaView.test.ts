import { describe, expect, it } from 'vitest';
import { claudeQuotaView } from './claudeQuotaView';
const now = Date.parse('2026-11-02T12:00:00Z');
const fixture = (used: number | null = 100) => ({
  result: {
    quota: {
      status: 'fresh',
      observedAt: new Date(now).toISOString(),
      data: {
        five_hour: {
          utilization: used,
          resets_at: new Date(now + 3600000).toISOString(),
        },
        seven_day: { utilization: 20, resets_at: null },
      },
    },
  },
});
describe('Claude account windows', () => {
  it('reports observation and distinguishes unsupported, failed, stale and expired snapshots', () => {
    expect(claudeQuotaView(fixture(25), false, now)).toMatchObject({
      status: 'fresh',
      observed: now,
    });
    expect(
      claudeQuotaView({ result: { status: 'unsupported' } }, false, now).status
    ).toBe('unsupported');
    expect(claudeQuotaView(fixture(), true, now).status).toBe('error');
    expect(claudeQuotaView(fixture(), false, now + 600000).status).toBe(
      'stale'
    );
    const expired = fixture();
    expired.result.quota.data.five_hour.resets_at = new Date(
      now - 1
    ).toISOString();
    expired.result.quota.data.seven_day.utilization = NaN;
    expect(claudeQuotaView(expired, false, now)).toMatchObject({
      status: 'unknown',
      observed: now,
    });
    expect(
      claudeQuotaView(expired, false, now).windows.every(
        (window) => window.remaining === null
      )
    ).toBe(true);
  });
  it('keeps exhausted zero distinct from an absent utilization and preserves reset time', () => {
    expect(claudeQuotaView(fixture(), false, now).windows[0]).toMatchObject({
      remaining: 0,
      reset: now + 3600000,
    });
    expect(
      claudeQuotaView(fixture(null), false, now).windows[0].remaining
    ).toBeNull();
    expect(claudeQuotaView(fixture(), false, now).windows[1].remaining).toBe(
      80
    );
  });
  it('suppresses stale, failed, expired and malformed usage without treating them as unlimited', () => {
    expect(
      claudeQuotaView(fixture(), true, now).windows[0].remaining
    ).toBeNull();
    expect(
      claudeQuotaView(fixture(), false, now + 600000).windows[0].remaining
    ).toBeNull();
    expect(
      claudeQuotaView(fixture(), false, now + 3600000).windows[0].remaining
    ).toBeNull();
    expect(
      claudeQuotaView(fixture(101), false, now).windows[0].remaining
    ).toBeNull();
    expect(claudeQuotaView(undefined, false, now)).toEqual({
      fresh: false,
      windows: [],
      status: 'pending',
      observed: null,
    });
  });
  it('includes provider model windows without exposing control characters', () => {
    const data = {
      result: {
        quota: {
          status: 'fresh',
          observedAt: new Date(now).toISOString(),
          data: {
            model_scoped: [
              {
                display_name: 'Model\u202eX',
                utilization: 25,
                resets_at: null,
              },
            ],
          },
        },
      },
    };
    expect(claudeQuotaView(data, false, now).windows[0]).toMatchObject({
      label: 'ModelX',
      remaining: 75,
      reset: null,
    });
  });
});
