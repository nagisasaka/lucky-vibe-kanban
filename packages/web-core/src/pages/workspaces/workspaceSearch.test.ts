import { describe, expect, it } from 'vitest';
import { matchesWorkspaceSearch } from './workspaceSearch';

const workspace = { name: 'Alpha task', branch: 'feature/branch' };

describe('workspace sidebar search', () => {
  it.each([
    ['106', 'LOCAL-106', true],
    ['106', 'LOCAL-1060', true],
    ['local-106', 'LOCAL-106', true],
    ['local-106', 'LOCAL-1060', true],
    ['107', 'LOCAL-106', false],
    ['1060', 'LOCAL-106', false],
    ['1060', 'LOCAL-1060', true],
    ['106', null, false],
    ['106', undefined, false],
    ['alpha', undefined, true],
    ['branch', null, true],
    ['', undefined, true],
  ])('query %s and issue %s => %s', (query, issue, expected) => {
    expect(matchesWorkspaceSearch(workspace, query, issue)).toBe(expected);
  });
});
