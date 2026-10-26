import { describe, expect, it } from 'vitest';
import { matchesListSearch } from './listSearch';

describe('browser list search', () => {
  it.each([
    ['  TODO  ', ['todo body', 'other'], true],
    ['   ', ['anything'], true],
    ['カテゴリ', ['body', 'カテゴリ'], true],
    ['Ä', ['äpfel'], true],
    ['日本', ['日本語'], true],
    ['ab', ['a', 'b'], false],
    ['%_', ['anything'], false],
    ['%_', ['x%_y'], true],
    ['a', ['Ａ'], false],
  ])(
    'matches literal query %j against separate fields',
    (query, fields, expected) => {
      expect(matchesListSearch(query, ...fields)).toBe(expected);
    }
  );
});
