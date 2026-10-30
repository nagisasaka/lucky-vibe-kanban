import type { Todo } from 'shared/types';

// Use the full Project snapshot, independently of list search/display filters.
export function getTodoCategorySuggestions(
  todos: readonly Pick<Todo, 'category' | 'completed'>[]
): string[] {
  return [
    ...new Set(
      todos
        .filter((todo) => !todo.completed)
        .map((todo) => todo.category)
        .filter(Boolean)
    ),
  ].sort();
}
