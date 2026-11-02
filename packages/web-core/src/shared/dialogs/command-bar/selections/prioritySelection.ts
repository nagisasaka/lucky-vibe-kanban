import type { IssuePriority } from 'shared/remote-types';
import type { SelectionPage } from '../SelectionDialog';

export interface PrioritySelectionResult {
  priority: IssuePriority | null;
}

export function buildPrioritySelectionPages(): Record<
  string,
  SelectionPage<PrioritySelectionResult>
> {
  return {
    selectPriority: {
      id: 'selectPriority',
      title: (t) => t('tasks:selections.selectPriority'),
      buildGroups: (t) => [
        {
          label: t('tasks:selections.priority'),
          items: [
            { id: 'urgent' as const, name: t('tasks:selections.urgent') },
            { id: 'high' as const, name: t('tasks:selections.high') },
            { id: 'medium' as const, name: t('tasks:selections.medium') },
            { id: 'low' as const, name: t('tasks:selections.low') },
          ].map((p) => ({
            type: 'priority' as const,
            priority: p,
          })),
        },
      ],
      onSelect: (item) => {
        if (item.type === 'priority') {
          return {
            type: 'complete',
            data: { priority: item.priority.id },
          };
        }
        return { type: 'complete', data: undefined as never };
      },
    },
  };
}
