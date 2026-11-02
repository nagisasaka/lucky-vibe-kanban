import { describe, expect, it } from 'vitest';
import { formatFileSize } from '@vibe/ui/lib/formatFileSize';

describe('explicit-locale file size formatting', () => {
  it('retains existing units, one decimal rounding and empty behavior', () => {
    expect(formatFileSize(null, 'ja')).toBe('');
    expect(formatFileSize(0n, 'ja')).toBe('');
    expect(formatFileSize(1023n, 'ja')).toBe('1,023 B');
    expect(formatFileSize(1536n, 'ja')).toBe('1.5 KB');
    expect(formatFileSize(1536, 'ja')).toBe('1.5 KB');
    expect(formatFileSize(1048576n, 'en')).toBe('1.0 MB');
    expect(formatFileSize(1536n)).toBe('1.5 KB');
  });
  it('preserves large byte counts and uses the requested decimal/grouping', () => {
    const bytes = 9007199254740993n * 1048576n;
    expect(formatFileSize(bytes, 'en')).toBe('9,007,199,254,740,993.0 MB');
    expect(formatFileSize(1536n, 'de')).toBe('1,5 KB');
  });
});
