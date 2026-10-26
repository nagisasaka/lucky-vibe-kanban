import { describe, expect, it } from 'vitest';
import {
  isValidProjectKey,
  normalizeProjectKey,
  suggestProjectKey,
} from './projectKey';

describe('Project keys', () => {
  it.each([
    ['Lucky Vibe Kanban', 'LVK'],
    ['Image-Studio_App', 'ISA'],
    ['Nectaris', 'NEC'],
    ['ab', 'AB'],
    ['App2', 'APP'],
    ['Image Studio 2', 'IS2'],
    ['A1', ''],
    ['12App', ''],
    ['N', ''],
    ['日本語', ''],
    ['日本語 App', ''],
    ['', ''],
  ])('suggests from %s', (name, expected) => {
    expect(suggestProjectKey(name, [])).toBe(expected);
  });
  it('avoids collisions within the ten character limit', () => {
    expect(suggestProjectKey('Lucky Vibe Kanban', ['LVK', 'LVK2'])).toBe(
      'LVK3'
    );
    expect(suggestProjectKey('A B C D E F G H I J', ['ABCDEFGHIJ'])).toBe(
      'ABCDEFGHI2'
    );
  });
  it.each([
    [' lvk ', 'LVK'],
    ['ab2', 'AB2'],
    ['ABCDEFGHIJ', 'ABCDEFGHIJ'],
    ['A1', 'A1'],
    ['\u0085ab\u0085', 'AB'],
  ])('normalizes valid input %s', (raw, expected) => {
    expect(isValidProjectKey(raw)).toBe(true);
    expect(normalizeProjectKey(raw)).toBe(expected);
  });
  it.each([
    '',
    'A',
    '1A',
    'AB-C',
    'AB_C',
    'ＡＢ',
    'éa',
    'あい',
    'ABCDEFGHIJK',
    '\ufeffAB',
  ])('rejects %s', (raw) => {
    expect(isValidProjectKey(raw)).toBe(false);
  });
});
