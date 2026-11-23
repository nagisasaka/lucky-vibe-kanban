import { useContext } from 'react';
import { ProjectContext } from '@/shared/hooks/useProjectContext';
import { WorkspaceContext } from '@/shared/hooks/useWorkspaceContext';
import type { SidebarWorkspace } from '@/shared/hooks/useWorkspaces';
import type { Workspace } from 'shared/remote-types';

export type PetScope =
  | { projectId: string }
  | { workspaceId: string | undefined };

export function hasScopedPetApproval(
  scope: PetScope,
  workspaces: Pick<SidebarWorkspace, 'id' | 'hasPendingRunApproval'>[],
  projectWorkspaces: Pick<Workspace, 'project_id' | 'local_workspace_id'>[]
): boolean {
  const ids = new Set(
    'workspaceId' in scope
      ? [scope.workspaceId]
      : projectWorkspaces
          .filter((workspace) => workspace.project_id === scope.projectId)
          .map((workspace) => workspace.local_workspace_id)
  );
  return workspaces.some(
    (workspace) =>
      ids.has(workspace.id) && workspace.hasPendingRunApproval === true
  );
}

// Reuse the existing workspace summaries and their refresh path. Do not start
// a Pet-specific subscription, and do not include Integration recovery waits.
export function usePetApproval(scope: PetScope): boolean {
  const project = useContext(ProjectContext);
  const workspace = useContext(WorkspaceContext);
  return hasScopedPetApproval(
    scope,
    [
      ...(workspace?.activeWorkspaces ?? []),
      ...(workspace?.archivedWorkspaces ?? []),
    ],
    project?.workspaces ?? []
  );
}
