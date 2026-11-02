import type { Issue } from 'shared/remote-types';
import type { SelectionPage } from '../SelectionDialog';

export interface RelationshipSelectionResult {
  issueId: string;
}

export function buildRelationshipSelectionPages(
  issues: Issue[]
): Record<string, SelectionPage<RelationshipSelectionResult>> {
  return {
    selectRelationshipIssue: {
      id: 'selectRelationshipIssue',
      title: (t) => t('tasks:selections.selectIssue'),
      buildGroups: (t) => [
        {
          label: t('tasks:selections.issues'),
          items: issues.map((issue) => ({ type: 'issue' as const, issue })),
        },
      ],
      onSelect: (item) => {
        if (item.type === 'issue') {
          return {
            type: 'complete',
            data: { issueId: item.issue.id },
          };
        }
        return { type: 'complete', data: undefined as never };
      },
    },
  };
}
