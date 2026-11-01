import { describe, expect, it } from 'vitest';
import { getTodoCategorySuggestions } from './todoCategories';

const todo = (category: string, completed = false) => ({ category, completed });

describe('getTodoCategorySuggestions', () => {
  it('uses only nonempty incomplete categories, deduplicated in existing sort order', () => {
    expect(
      getTodoCategorySuggestions([
        todo('z'),
        todo('a'),
        todo('z'),
        todo(''),
        todo('completed only', true),
        todo('a', true),
      ])
    ).toEqual(['a', 'z']);
    expect(getTodoCategorySuggestions([])).toEqual([]);
  });

  it('keeps a shared category until its last incomplete todo is completed', () => {
    expect(
      getTodoCategorySuggestions([todo('shared'), todo('shared', true)])
    ).toEqual(['shared']);
    expect(
      getTodoCategorySuggestions([todo('shared', true), todo('shared', true)])
    ).toEqual([]);
  });

  it('recomputes after deletion, category change and reopening without changing saved values', () => {
    const completed = Object.freeze(todo('old', true));
    expect(getTodoCategorySuggestions([completed, todo('old')])).toEqual([
      'old',
    ]);
    // Refetched snapshot after deleting the last incomplete todo.
    expect(getTodoCategorySuggestions([completed])).toEqual([]);
    // Refetched snapshot after changing its category through free input.
    expect(
      getTodoCategorySuggestions([completed, todo('new free text')])
    ).toEqual(['new free text']);
    expect(
      getTodoCategorySuggestions([{ ...completed, completed: false }])
    ).toEqual(['old']);
    expect(completed).toEqual(todo('old', true));
  });
});
