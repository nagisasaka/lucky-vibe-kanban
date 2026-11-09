import { useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useUserContext } from '@/shared/hooks/useUserContext';
import { useWorkspaceContext } from '@/shared/hooks/useWorkspaceContext';
import { useCurrentKanbanRouteState } from '@/shared/hooks/useCurrentKanbanRouteState';
import { getWorkspaceDefaults } from '@/shared/lib/workspaceDefaults';
import { buildLocalWorkspaceIdSet } from '@/shared/lib/workspaceCreateState';
import { repoApi } from '@/shared/lib/api';
import type { DraftWorkspaceRepo } from 'shared/types';
import { WorkspaceTargetDialog } from '@/shared/dialogs/shared/WorkspaceTargetDialog';

interface UseWorkflowRepositorySelectionOptions {
  projectId?: string | null;
  issueId: string;
  issueTitle: string;
}

export function useWorkflowRepositorySelection({
  projectId,
}: UseWorkflowRepositorySelectionOptions) {
  const { t } = useTranslation('common');
  const { workspaces } = useUserContext();
  const { activeWorkspaces, archivedWorkspaces } = useWorkspaceContext();
  const routeState = useCurrentKanbanRouteState();

  const localWorkspaceIds = useMemo(
    () => buildLocalWorkspaceIdSet(activeWorkspaces, archivedWorkspaces),
    [activeWorkspaces, archivedWorkspaces]
  );

  const selectWorkflowRepositories = useCallback(async (): Promise<
    DraftWorkspaceRepo[] | null
  > => {
    if (!projectId) return null;

    const defaults = await getWorkspaceDefaults(
      workspaces,
      localWorkspaceIds,
      projectId,
      routeState.hostId
    );
    const preferredRepo = defaults?.preferredRepos[0];
    const preferredRepoDetails = preferredRepo
      ? await repoApi
          .getById(preferredRepo.repo_id, routeState.hostId)
          .catch(() => null)
      : null;

    const result = await WorkspaceTargetDialog.show({
      initialPath: preferredRepoDetails?.path,
      initialMode: 'worktree',
      initialBranch: preferredRepo?.target_branch,
      initialRepos: preferredRepo ? [preferredRepo] : undefined,
      hostId: routeState.hostId,
      allowedModes: ['worktree'],
      singleRepository: true,
      projectId,
      title: t('workflow.workspaceDialog.title'),
      description: t('workflow.workspaceDialog.description'),
    });

    if (result.kind === 'canceled' || result.selection.mode !== 'worktree') {
      return null;
    }

    const repos: DraftWorkspaceRepo[] = [
      {
        repo_id: result.selection.repo.id,
        target_branch: result.selection.targetBranch,
      },
    ];

    return repos;
  }, [projectId, routeState.hostId, t, workspaces, localWorkspaceIds]);

  return { selectWorkflowRepositories };
}
