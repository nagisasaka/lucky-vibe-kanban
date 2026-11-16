import { describe, expect, it } from 'vitest';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import type { Issue, IssueRelationship } from 'shared/remote-types';
import { searchKanbanIssues } from './searchKanbanIssues';
import { useKanbanFilters } from './hooks/useKanbanFilters';
import { DEFAULT_KANBAN_FILTER_STATE } from '@/shared/stores/useUiPreferencesStore';

const issue = (id: string, number: number, extra: Partial<Issue> = {}): Issue =>
  ({
    id,
    project_id: 'project',
    issue_number: number,
    simple_id: `TEST-${number}`,
    title: id,
    status_id: 'todo',
    archived: false,
    parent_issue_id: null,
    ...extra,
  }) as Issue;
const link = (
  a: string,
  b: string,
  type: IssueRelationship['relationship_type'] = 'related'
): IssueRelationship => ({
  id: `${a}-${b}`,
  issue_id: a,
  related_issue_id: b,
  relationship_type: type,
  created_at: '',
});
const issues = [
  issue('theme', 101),
  issue('b', 202, { status_id: 'doing' }),
  issue('c', 303),
  issue('theme two', 404),
  issue('blocked', 505),
  issue('duplicate', 606),
  issue('child', 707, { parent_issue_id: 'theme' }),
  issue('foreign', 808, { project_id: 'other' }),
  issue('isolated', 909),
];
const links = [
  link('b', 'theme'),
  link('b', 'c'),
  link('c', 'theme'),
  link('theme two', 'c'),
  link('theme', 'blocked', 'blocking'),
  link('theme', 'duplicate', 'has_duplicate'),
  link('theme', 'foreign'),
  link('theme', 'missing'),
  link('missing', 'isolated'),
];
const ids = (result: Set<string>) => [...result].sort();

describe('board related issue search', () => {
  it.each(['101', 'TEST-101', 'theme'])(
    'expands %s through all related edges only',
    (query) => {
      expect(ids(searchKanbanIssues(issues, links, query, true))).toEqual([
        'b',
        'c',
        'theme',
        'theme two',
      ]);
    }
  );
  it.each(['101', 'theme'])(
    'keeps direct matches when disabled: %s',
    (query) => {
      expect(ids(searchKanbanIssues(issues, links, query, false))).toEqual(
        query === '101' ? ['theme'] : ['theme', 'theme two']
      );
    }
  );
  it('walks in reverse, terminates cycles and deduplicates multiple seeds', () => {
    expect(ids(searchKanbanIssues(issues, links, '303', true))).toEqual([
      'b',
      'c',
      'theme',
      'theme two',
    ]);
  });
  it('handles no matches and empty search', () => {
    expect(searchKanbanIssues(issues, links, 'absent', true).size).toBe(0);
    for (const enabled of [false, true]) {
      expect(searchKanbanIssues(issues, links, '  ', enabled).size).toBe(
        issues.length
      );
    }
  });
  it('preserves other filters and the original status of expanded issues', () => {
    let result: Issue[] = [];
    function Probe() {
      result = useKanbanFilters({
        issues: issues.map((i) => ({
          ...i,
          priority: i.id === 'b' ? 'high' : 'low',
        })),
        issueRelationships: links,
        issuesById: new Map(issues.map((i) => [i.id, i])),
        issueAssignees: [{ issue_id: 'b', user_id: 'user' } as never],
        issueTags: [{ issue_id: 'b', tag_id: 'tag' } as never],
        doneStatusIds: new Set(),
        currentUserId: 'user',
        showSubIssues: false,
        hideBlocked: true,
        filters: {
          ...DEFAULT_KANBAN_FILTER_STATE,
          searchQuery: '101',
          priorities: ['high'],
          assigneeIds: ['user'],
          tagIds: ['tag'],
        },
      }).filteredIssues;
      return null;
    }
    renderToStaticMarkup(createElement(Probe));
    expect(result.map((i) => [i.id, i.status_id])).toEqual([['b', 'doing']]);
  });
});
