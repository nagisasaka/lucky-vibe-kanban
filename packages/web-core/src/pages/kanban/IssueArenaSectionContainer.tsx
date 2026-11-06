import { useProjectContext } from '@/shared/hooks/useProjectContext';
import { useNavigate, useParams } from '@tanstack/react-router';
import { useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { IssueAttemptOption } from '@vibe/ui/components/IssueTaskAttemptsSection';
import {
  arenaQueryKeys,
  useActiveArenaForIssue,
} from '@/shared/hooks/useArenaGroup';
import { CreateArenaDialog } from '@/features/arena';
import { arenaApi, isActiveArenaAgentRunStatus } from '@/shared/lib/arenaApi';
import { useCurrentKanbanRouteState } from '@/shared/hooks/useCurrentKanbanRouteState';

interface IssueArenaSectionContainerProps {
  issueId: string;
  /** Suggested initial prompt for the new Arena. */
  initialPrompt?: string;
}

/** Compact Arena entry point for the Kanban issue panel. */
export function IssueArenaSectionContainer({
  issueId,
  initialPrompt,
}: IssueArenaSectionContainerProps) {
  const { projectId } = useParams({ strict: false });
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { t } = useTranslation('common');
  const routeState = useCurrentKanbanRouteState();
  const { getIssue } = useProjectContext();
  const issueArchived = getIssue(issueId)?.archived ?? false;

  const {
    data: activeArena,
    isLoading,
    refetch,
  } = useActiveArenaForIssue(issueId);

  if (!projectId) return null;

  const goToArena = (groupId: string) => {
    const target =
      `/projects/${projectId}/issues/${issueId}/arena/${groupId}` as '/';
    void navigate({ to: target });
  };

  const handleStart = async () => {
    const latestArena = await refetch();
    if (latestArena.data) {
      if (latestArena.data.workspaces.length > 0) {
        goToArena(latestArena.data.id);
        return;
      }

      try {
        await arenaApi.dissolve(latestArena.data.id);
        await queryClient.invalidateQueries({
          queryKey: arenaQueryKeys.activeForIssue(issueId),
        });
      } catch (error) {
        console.warn('[IssueArenaSection] Failed to clear empty Arena:', error);
        goToArena(latestArena.data.id);
        return;
      }
    }

    const result = await CreateArenaDialog.show({
      projectId,
      issueId,
      hostId: routeState.hostId,
      initialPrompt,
    });
    if (result.kind === 'created') {
      await queryClient.invalidateQueries({
        queryKey: arenaQueryKeys.activeForIssue(issueId),
      });
      goToArena(result.groupId);
    }
  };

  if (activeArena && activeArena.workspaces.length > 0) {
    const total = activeArena.workspaces.length;
    const running = activeArena.workspaces.filter((ws) =>
      isActiveArenaAgentRunStatus(ws.latest_agent_run_status)
    ).length;
    const modeLabel =
      activeArena.mode === 'design'
        ? t('arena.modes.design')
        : t('arena.modes.implementation');
    return (
      <IssueAttemptOption
        title={`${t('arena.title')} — ${t('arena.actions.open')}`}
        description={t('arena.issueSection.activeSummary', {
          mode: modeLabel,
          total,
          running,
        })}
        onClick={() => goToArena(activeArena.id)}
      />
    );
  }

  return (
    <IssueAttemptOption
      title={t('arena.actions.startArena')}
      description={t('arena.issueSection.startSubtitle')}
      disabledReason={
        isLoading
          ? t('states.loading')
          : issueArchived
            ? t('kanban.restoreToUse')
            : undefined
      }
      onClick={() => void handleStart()}
    />
  );
}
