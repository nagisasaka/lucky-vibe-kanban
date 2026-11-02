import { describe, expect, it } from 'vitest';
import { resolveUiLocale } from './languages';

describe('browser UI locale resolution', () => {
  it.each([
    ['en-US', 'en'],
    ['ja-JP', 'ja'],
    ['fr-CA', 'fr'],
    ['es-MX', 'es'],
    ['ko-KR', 'ko'],
    ['zh', 'zh-Hans'],
    ['zh-CN', 'zh-Hans'],
    ['zh-TW', 'zh-Hant'],
    ['zh-HK', 'zh-Hant'],
    ['zh-MO', 'zh-Hant'],
    ['zh-Hant-TW', 'zh-Hant'],
    ['zh-Hans-CN', 'zh-Hans'],
    ['ja_JP', 'ja'],
    ['de-DE', 'en'],
  ])('resolves %s to %s', (input, expected) => {
    expect(resolveUiLocale(input)).toBe(expected);
  });
});
