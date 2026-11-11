import { useQueryClient } from '@tanstack/react-query';
import { useHostId } from '@/shared/providers/HostIdProvider';
import { workspaceSummaryKeys } from '@/shared/hooks/workspaceSummaryKeys';
import { errorDiagnostic } from '@/shared/lib/errorDiagnostic';
import i18n from '@/i18n/config';
import { useCallback, useEffect, useMemo, useState, useRef } from 'react';
import { useParams } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { BaseCodingAgent, type StartPresetRequest } from 'shared/types';
import { useUserSystem } from '@/shared/hooks/useUserSystem';
import { useProjectWorkspaceCreateDraft } from '@/shared/hooks/useProjectWorkspaceCreateDraft';
import { WorkspaceTargetDialog } from '@/shared/dialogs/shared/WorkspaceTargetDialog';
import { workflowPresetsApi } from '@/shared/lib/workflowApi';
import { LinkIcon } from '@phosphor-icons/react';
import { useProjectContext } from '@/shared/hooks/useProjectContext';
import { useAuth } from '@/shared/hooks/auth/useAuth';
import { useOrgContext } from '@/shared/hooks/useOrgContext';
import { useUserContext } from '@/shared/hooks/useUserContext';
import { useWorkspaceContext } from '@/shared/hooks/useWorkspaceContext';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { useCurrentKanbanRouteState } from '@/shared/hooks/useCurrentKanbanRouteState';
import { workspacesApi } from '@/shared/lib/api';
import { getWorkspaceDefaults } from '@/shared/lib/workspaceDefaults';
import {
  buildLocalWorkspaceIdSet,
  buildWorkspaceCreateInitialState,
  buildLinkedIssueCreateState,
  buildWorkspaceCreatePrompt,
} from '@/shared/lib/workspaceCreateState';
import {
  useWorkflowAttemptMutations,
  useWorkflowAttempts,
} from '@/shared/hooks/useWorkflowAttempts';
import { buildTaskAttempts, type TaskAttemptView } from '@/features/workflow';
import { ConfirmDialog } from '@vibe/ui/components/ConfirmDialog';
import { DeleteWorkspaceDialog } from '@vibe/ui/components/DeleteWorkspaceDialog';
import type { IssueTaskAttemptCardData } from '@vibe/ui/components/IssueTaskAttemptCard';
import { IssueTaskAttemptsSection } from '@vibe/ui/components/IssueTaskAttemptsSection';
import type { SectionAction } from '@vibe/ui/components/CollapsibleSectionHeader';
import type { WorkspaceWithStats } from '@vibe/ui/components/IssueWorkspaceCard';

interface IssueTaskAttemptsSectionContainerProps {
  issueId: string;
  issueTitle: string;
  issueDescription?: string | null;
}

export function IssueTaskAttemptsSectionContainer({
  issueId,
  issueTitle,
  issueDescription,
}: IssueTaskAttemptsSectionContainerProps) {
  const { t } = useTranslation('common');
  const queryClient = useQueryClient();
  const hostId = useHostId();
  useEffect(() => {
    void queryClient.invalidateQueries({
      queryKey: workspaceSummaryKeys.byArchived(true, hostId),
    });
  }, [issueId, hostId, queryClient]);
  const { projectId } = useParams({ strict: false });
  const appNavigation = useAppNavigation();
  const routeState = useCurrentKanbanRouteState();
  const { config } = useUserSystem();
  const { openWorkspaceCreateFromState } = useProjectWorkspaceCreateDraft();
  const { userId } = useAuth();
  const { workspaces } = useUserContext();
  const [error, setError] = useState<string | null>(null);
  const pendingPreset = useRef<StartPresetRequest | null>(null);
  const starting = useRef(false);

  const {
    pullRequests,
    getIssue,
    getWorkspacesForIssue,
    isLoading: projectLoading,
  } = useProjectContext();
  const { activeWorkspaces, archivedWorkspaces } = useWorkspaceContext();
  const { membersWithProfilesById, isLoading: orgLoading } = useOrgContext();
  const { data: workflowAttemptData, isLoading: workflowAttemptsLoading } =
    useWorkflowAttempts(projectId, issueId, { enabled: !!projectId });
  const { deleteAttempt: deleteWorkflowAttempt } =
    useWorkflowAttemptMutations();

  const localWorkspacesById = useMemo(() => {
    const map = new Map<string, (typeof activeWorkspaces)[number]>();

    for (const workspace of activeWorkspaces) {
      map.set(workspace.id, workspace);
    }

    for (const workspace of archivedWorkspaces) {
      map.set(workspace.id, workspace);
    }

    return map;
  }, [activeWorkspaces, archivedWorkspaces]);

  const workspacesWithStats: WorkspaceWithStats[] = useMemo(() => {
    const rawWorkspaces = getWorkspacesForIssue(issueId);

    return rawWorkspaces.map((workspace) => {
      const localWorkspace = workspace.local_workspace_id
        ? localWorkspacesById.get(workspace.local_workspace_id)
        : undefined;
      const linkedPrs = pullRequests
        .filter((pr) => pr.workspace_id === workspace.id)
        .map((pr) => ({
          number: pr.number,
          url: pr.url,
          status: pr.status as 'open' | 'merged' | 'closed',
        }));
      const owner =
        membersWithProfilesById.get(workspace.owner_user_id) ?? null;

      return {
        id: workspace.id,
        localWorkspaceId: workspace.local_workspace_id,
        name: workspace.name,
        archived: workspace.archived,
        filesChanged: workspace.files_changed ?? 0,
        linesAdded: workspace.lines_added ?? 0,
        linesRemoved: workspace.lines_removed ?? 0,
        prs: linkedPrs,
        owner,
        updatedAt: workspace.updated_at,
        isOwnedByCurrentUser: workspace.owner_user_id === userId,
        isRunning: localWorkspace?.isRunning,
        hasPendingApproval: localWorkspace?.hasPendingApproval,
        hasRunningDevServer: localWorkspace?.hasRunningDevServer,
        hasUnseenActivity: localWorkspace?.hasUnseenActivity,
        latestProcessStartedAt: localWorkspace?.latestProcessStartedAt,
        latestProcessCompletedAt: localWorkspace?.latestProcessCompletedAt,
        latestProcessStatus: localWorkspace?.latestProcessStatus,
      };
    });
  }, [
    issueId,
    getWorkspacesForIssue,
    pullRequests,
    membersWithProfilesById,
    userId,
    localWorkspacesById,
  ]);

  const attempts = useMemo(
    () =>
      buildTaskAttempts(
        {
          workspaceAttempts: workspacesWithStats,
          workflowAttempts: workflowAttemptData?.attempts ?? [],
        },
        t
      ),
    [workspacesWithStats, workflowAttemptData, t]
  );
  const handleAddWorkspace = useCallback(async () => {
    if (!projectId || starting.current) return;
    starting.current = true;
    setError(null);
    try {
      if (
        pendingPreset.current?.issue_id === issueId &&
        pendingPreset.current.project_id === projectId
      ) {
        const run = await workflowPresetsApi.start(pendingPreset.current);
        pendingPreset.current = null;
        await queryClient.invalidateQueries({
          queryKey: ['workflow-attempts'],
        });
        appNavigation.goToProjectWorkflowRun(projectId, run.id);
        return;
      }
      const defaults = await getWorkspaceDefaults(
        workspaces,
        buildLocalWorkspaceIdSet(activeWorkspaces, archivedWorkspaces),
        projectId,
        routeState.hostId
      );
      const result = await WorkspaceTargetDialog.show({
        projectId,
        hostId: routeState.hostId,
        initialRepos: defaults?.preferredRepos,
        workflowProjectId: projectId,
        initialStage: {
          engine: config?.executor_profile.executor ?? 'CODEX',
          model: null,
          variant: config?.executor_profile.variant ?? null,
        },
      });
      if (result.kind !== 'confirmed') return;
      const selection = result.selection;
      const repos =
        selection.mode === 'worktree'
          ? selection.repos.map((v) => ({
              repo_id: v.repo.id,
              target_branch: v.targetBranch,
            }))
          : [];
      if (result.workflow) {
        pendingPreset.current = {
          request_id: crypto.randomUUID(),
          project_id: projectId,
          issue_id: issueId,
          workspace_id: null,
          repos,
          parameters: result.workflow,
        };
        const run = await workflowPresetsApi.start(pendingPreset.current);
        pendingPreset.current = null;
        await queryClient.invalidateQueries({
          queryKey: ['workflow-attempts'],
        });
        appNavigation.goToProjectWorkflowRun(projectId, run.id);
      } else {
        const stage = result.singleStage;
        const state = buildWorkspaceCreateInitialState({
          prompt: buildWorkspaceCreatePrompt(issueTitle, issueDescription),
          defaults: {
            preferredRepos: repos,
            preferredDirectoryPath:
              selection.mode === 'direct_folder' ? selection.path : null,
          },
          linkedIssue: buildLinkedIssueCreateState(
            getIssue(issueId),
            projectId
          ),
          executorConfig: {
            executor:
              stage?.engine === 'CLAUDE_CODE'
                ? BaseCodingAgent.CLAUDE_CODE
                : BaseCodingAgent.CODEX,
            variant: stage?.variant,
            model_id: stage?.model,
          },
        });
        if (!(await openWorkspaceCreateFromState(state, { issueId }))) {
          throw new Error('Failed to prepare single-agent input');
        }
      }
    } catch (e) {
      if (
        e &&
        typeof e === 'object' &&
        'status' in e &&
        typeof e.status === 'number' &&
        e.status >= 400 &&
        e.status < 500
      ) {
        pendingPreset.current = null;
      }
      setError(errorDiagnostic(e));
    } finally {
      starting.current = false;
    }
  }, [
    projectId,
    issueId,
    issueTitle,
    issueDescription,
    config,
    workspaces,
    activeWorkspaces,
    archivedWorkspaces,
    routeState.hostId,
    queryClient,
    appNavigation,
    getIssue,
    openWorkspaceCreateFromState,
  ]);

  const handleLinkWorkspace = useCallback(async () => {
    if (!projectId) {
      return;
    }

    const { WorkspaceSelectionDialog } = await import(
      '@/shared/dialogs/command-bar/WorkspaceSelectionDialog'
    );
    await WorkspaceSelectionDialog.show({
      projectId,
      issueId,
      hostId: routeState.hostId,
    });
  }, [projectId, issueId, routeState.hostId]);

  const handleOpenAttempt = useCallback(
    (attemptData: IssueTaskAttemptCardData) => {
      if (!projectId) return;
      const attempt = attemptData as TaskAttemptView;

      if (attempt.kind === 'workflow') {
        if (attempt.latestRunId)
          appNavigation.goToProjectWorkflowRun(projectId, attempt.latestRunId);
        else if (attempt.workflowId)
          appNavigation.goToProjectWorkflowEdit(projectId, attempt.workflowId);
        return;
      }

      if (attempt.localWorkspaceId) {
        appNavigation.goToProjectIssueWorkspace(
          projectId,
          issueId,
          attempt.localWorkspaceId
        );
      }
    },
    [projectId, issueId, appNavigation]
  );

  const handleUnlinkAttempt = useCallback(
    async (attemptData: IssueTaskAttemptCardData) => {
      const attempt = attemptData as TaskAttemptView;
      if (attempt.kind !== 'single_agent' || !attempt.localWorkspaceId) return;

      const result = await ConfirmDialog.show({
        title: t('workspaces.unlinkFromIssue'),
        message: t('workspaces.unlinkConfirmMessage'),
        confirmText: t('workspaces.unlink'),
        variant: 'destructive',
      });

      if (result === 'confirmed') {
        try {
          await workspacesApi.unlinkFromIssue(attempt.localWorkspaceId);
        } catch (error) {
          ConfirmDialog.show({
            title: t('common:error'),
            message: () =>
              `${i18n.t('workspaces.unlinkError', { ns: 'common' })} ${errorDiagnostic(error)}`,
            confirmText: t('common:ok'),
            showCancelButton: false,
          });
        }
      }
    },
    [t]
  );

  const handleDeleteAttempt = useCallback(
    async (attemptData: IssueTaskAttemptCardData) => {
      const attempt = attemptData as TaskAttemptView;
      if (attempt.kind === 'workflow') {
        if (!attempt.workflowAttemptId) return;

        const result = await ConfirmDialog.show({
          title: t('attempts.deleteWorkflowTitle'),
          message: t('attempts.deleteWorkflowMessage'),
          confirmText: t('attempts.deleteAttempt'),
          variant: 'destructive',
        });

        if (result !== 'confirmed') {
          return;
        }

        try {
          await deleteWorkflowAttempt(attempt.workflowAttemptId);
        } catch (error) {
          ConfirmDialog.show({
            title: t('common:error'),
            message: () =>
              `${i18n.t('attempts.deleteWorkflowError', { ns: 'common' })} ${errorDiagnostic(error)}`,
            confirmText: t('common:ok'),
            showCancelButton: false,
          });
        }
        return;
      }

      if (!attempt.localWorkspaceId) return;

      const localWorkspace = localWorkspacesById.get(attempt.localWorkspaceId);
      if (!localWorkspace) {
        ConfirmDialog.show({
          title: t('common:error'),
          message: t('workspaces.deleteError'),
          confirmText: t('common:ok'),
          showCancelButton: false,
        });
        return;
      }

      const result = await DeleteWorkspaceDialog.show({
        branchName: localWorkspace.branch,
        hasOpenPR:
          workspacesWithStats
            .find(
              (workspace) =>
                workspace.localWorkspaceId === attempt.localWorkspaceId
            )
            ?.prs.some((pr) => pr.status === 'open') ?? false,
        isLinkedToIssue: true,
        linkedIssueSimpleId: getIssue(issueId)?.simple_id,
      });

      if (result.action !== 'confirmed') {
        return;
      }

      try {
        await workspacesApi.delete(
          attempt.localWorkspaceId,
          result.deleteBranches
        );
        if (result.unlinkFromIssue) {
          await workspacesApi.unlinkFromIssue(attempt.localWorkspaceId);
        }
      } catch (error) {
        ConfirmDialog.show({
          title: t('common:error'),
          message: () =>
            `${i18n.t('workspaces.deleteError', { ns: 'common' })} ${errorDiagnostic(error)}`,
          confirmText: t('common:ok'),
          showCancelButton: false,
        });
      }
    },
    [
      deleteWorkflowAttempt,
      localWorkspacesById,
      workspacesWithStats,
      getIssue,
      issueId,
      t,
    ]
  );

  const issueArchived = getIssue(issueId)?.archived ?? false;
  const actions: SectionAction[] = useMemo(
    () => [
      {
        disabled: issueArchived,
        title: issueArchived ? t('kanban.restoreToUse') : undefined,
        icon: LinkIcon,
        onClick: handleLinkWorkspace,
        label: t('kanban.linkWorkspace', 'Link workspace'),
      },
    ],
    [handleLinkWorkspace, t, issueArchived]
  );

  return (
    <div>
      <IssueTaskAttemptsSection
        attempts={attempts}
        isLoading={projectLoading || orgLoading || workflowAttemptsLoading}
        actions={actions}
        onOpenAttempt={handleOpenAttempt}
        onUnlinkAttempt={handleUnlinkAttempt}
        onDeleteAttempt={handleDeleteAttempt}
        createDisabledReason={
          issueArchived ? t('kanban.restoreToUse') : undefined
        }
        onStartAttempt={handleAddWorkspace}
      />
      {error ? (
        <p className="px-base pb-base text-xs text-error" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}
