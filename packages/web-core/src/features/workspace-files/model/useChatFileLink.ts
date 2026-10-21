import { useCallback } from 'react';
import { useWorkspaceRecord } from '@/shared/hooks/useWorkspaceRecord';
import { useWorkspaceRepo } from '@/shared/hooks/useWorkspaceRepo';
import { useWorkspaceFilePreviewActions } from './WorkspaceFilePreviewActionsProvider';
import { resolveChatFileLink } from './chatFileLink';

export function useChatFileLink(workspaceId?: string, sessionId?: string) {
  const { data: workspace } = useWorkspaceRecord(workspaceId);
  const { repos } = useWorkspaceRepo(workspaceId);
  const { canOpenWorkspaceFilePreview, openWorkspaceFilePreview } =
    useWorkspaceFilePreviewActions();
  return useCallback(
    (href: string) => {
      if (!canOpenWorkspaceFilePreview) return;
      const target = resolveChatFileLink({
        href,
        workspaceId,
        sessionId,
        repos,
        containerRef: workspace?.container_ref,
      });
      return target ? () => openWorkspaceFilePreview(target) : undefined;
    },
    [
      workspaceId,
      sessionId,
      repos,
      workspace?.container_ref,
      canOpenWorkspaceFilePreview,
      openWorkspaceFilePreview,
    ]
  );
}
