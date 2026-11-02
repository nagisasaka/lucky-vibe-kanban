import { afterEach, describe, expect, it, vi } from 'vitest';
import { formatRelativeTime as formatAge } from '@vibe/ui/lib/dateFormatting';
import i18n from '@/i18n/config';
import {
  formatDateShortWithTime,
  formatRelativeTime,
  formatUtcDateTime,
} from './date';

const now = Date.parse('2026-01-10T15:04:05Z');
const ago = (minutes: number) => new Date(now - minutes * 60_000).toISOString();

afterEach(async () => {
  vi.useRealTimers();
  vi.unstubAllEnvs();
  await i18n.changeLanguage('en');
});

describe('localized time formatting', () => {
  it.each([
    ['en', 0, 'now'],
    ['en', 1, '1m ago'],
    ['en', 59, '59m ago'],
    ['en', 60, '1h ago'],
    ['en', 1439, '23h ago'],
    ['en', 2880, '2d ago'],
    ['ja', 0, '今'],
    ['ja', 1, '1分前'],
    ['ja', 59, '59分前'],
    ['ja', 60, '1時間前'],
    ['ja', 1439, '23時間前'],
    ['ja', 2880, '2日前'],
  ])('%s renders an age of %s minutes as %s', (locale, minutes, expected) => {
    expect(formatAge(ago(Number(minutes)), String(locale), now)).toBe(expected);
  });

  it('keeps the existing near-now treatment for future timestamps and handles invalid dates', () => {
    expect(formatAge(ago(-10), 'ja', now)).toBe('今');
    expect(formatAge('invalid', 'ja', now)).toBe('—');
  });

  it.each(['en', 'ja'])(
    'uses selected %s locale without changing the timezone or timestamp',
    async (locale) => {
      vi.useFakeTimers();
      vi.setSystemTime(now);
      await i18n.changeLanguage(locale);
      const value = ago(60);
      expect(formatRelativeTime(value)).toBe(
        locale === 'ja' ? '1時間前' : '1h ago'
      );
      expect(formatDateShortWithTime(value)).toBe(
        new Date(value).toLocaleDateString(locale, {
          month: 'short',
          day: 'numeric',
          hour: '2-digit',
          minute: '2-digit',
        })
      );
    }
  );
});

it.each(['en', 'ja'])(
  'formats SQLite integration timestamps as UTC in %s, retaining milliseconds',
  (locale) => {
    vi.stubEnv('TZ', 'America/Los_Angeles');
    const sqlite = '2026-01-02 15:04:05.123';
    const formatted = formatUtcDateTime(sqlite, locale);
    expect(formatted).toBe(
      formatUtcDateTime('2026-01-02T15:04:05.123Z', locale)
    );
    expect(formatted).toBe(
      formatUtcDateTime('2026-01-02T16:04:05.123+01:00', locale)
    );
    expect(formatted).toContain('123');
    expect(formatted).toContain('UTC');
    expect(formatUtcDateTime('invalid', locale)).toBe('—');
  }
);
