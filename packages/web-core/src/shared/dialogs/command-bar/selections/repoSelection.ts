import type { RepoItem } from '@/shared/types/selectionItems';
import type { SelectionPage } from '../SelectionDialog';

export interface RepoSelectionResult {
  repoId: string;
}

export function buildRepoSelectionPages(
  repos: RepoItem[]
): Record<string, SelectionPage<RepoSelectionResult>> {
  return {
    selectRepo: {
      id: 'selectRepo',
      title: (t) => t('tasks:selections.selectRepo'),
      buildGroups: (t) => [
        {
          label: t('tasks:selections.repos'),
          items: repos.map((r) => ({ type: 'repo' as const, repo: r })),
        },
      ],
      onSelect: (item) => {
        if (item.type === 'repo') {
          return { type: 'complete', data: { repoId: item.repo.id } };
        }
        return { type: 'complete', data: undefined as never };
      },
    },
  };
}
