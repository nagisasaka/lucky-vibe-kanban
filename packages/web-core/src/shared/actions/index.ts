import { useTodoViewStore } from '@/features/todos/model/todoViewStore';
import { isProjectDestination } from '@/shared/lib/routes/appNavigation';
import { errorDiagnostic } from '@/shared/lib/errorDiagnostic';
import {
  IssueArchiveActionError,
  receiptMessage,
} from '@/shared/lib/issueArchiveNotice';
import { ActionError } from '@/shared/lib/actionMessages';
import i18n from '@/i18n/config';
import { IssueArchiveDialog } from '@/shared/dialogs/kanban/IssueArchiveDialog';
import {
  issueArchiveApi,
  refreshIssueArchive,
} from '@/shared/lib/issueArchiveApi';
import { forwardRef, createElement } from 'react';
import type { Icon, IconProps } from '@phosphor-icons/react';
import type { ExecutorConfig, Merge, Workspace } from 'shared/types';
import type { QueryClient } from '@tanstack/react-query';
import {
  CopyIcon,
  XIcon,
  PushPinIcon,
  ArchiveIcon,
  TrashIcon,
  PlusIcon,
  GearIcon,
  ColumnsIcon,
  RowsIcon,
  TextAlignLeftIcon,
  FilesIcon,
  ListChecksIcon,
  BookOpenIcon,
  EyeSlashIcon,
  SidebarSimpleIcon,
  ChatsTeardropIcon,
  GitDiffIcon,
  TerminalIcon,
  CaretDoubleUpIcon,
  CaretDoubleDownIcon,
  PlayIcon,
  PauseIcon,
  SpinnerIcon,
  GitPullRequestIcon,
  GitMergeIcon,
  GitForkIcon,
  ArrowsClockwiseIcon,
  CrosshairIcon,
  DesktopIcon,
  PencilSimpleIcon,
  ArrowUpIcon,
  HighlighterIcon,
  ListIcon,
  QuestionIcon,
  ArrowsLeftRightIcon,
  ArrowFatLineUpIcon,
  UsersIcon,
  TreeStructureIcon,
  LinkIcon,
  ArrowBendUpRightIcon,
  ProhibitIcon,
} from '@phosphor-icons/react';
import { useDiffViewStore } from '@/shared/stores/useDiffViewStore';
import { useWorkspaceDiffStore } from '@/shared/stores/useWorkspaceDiffStore';
import {
  useUiPreferencesStore,
  RIGHT_MAIN_PANEL_MODES,
} from '@/shared/stores/useUiPreferencesStore';

import { workspacesApi, repoApi } from '@/shared/lib/api';
import { bulkUpdateIssues } from '@/shared/lib/remoteApi';
import { workspaceRecordKeys } from '@/shared/hooks/useWorkspaceRecord';
import { workspaceRepoKeys } from '@/shared/hooks/useWorkspaceRepo';
import { repoBranchKeys } from '@/shared/hooks/useRepoBranches';
import { workspaceSummaryKeys } from '@/shared/hooks/workspaceSummaryKeys';
import { ConfirmDialog } from '@vibe/ui/components/ConfirmDialog';
import { ChangeTargetDialog } from '@vibe/ui/components/ChangeTargetDialog';
import { DeleteWorkspaceDialog } from '@vibe/ui/components/DeleteWorkspaceDialog';
import { RebaseDialog } from '@/shared/dialogs/command-bar/RebaseDialog';
import { RenameWorkspaceDialog } from '@vibe/ui/components/RenameWorkspaceDialog';
import { InteractiveHelp } from '@/shared/components/InteractiveHelp';
import { ProjectsGuideDialog } from '@vibe/ui/components/ProjectsGuideDialog';
import { CreatePRDialog } from '@/shared/dialogs/command-bar/CreatePRDialog';
import { EditorSelectionDialog } from '@/shared/dialogs/command-bar/EditorSelectionDialog';
import { StartReviewDialog } from '@/shared/dialogs/command-bar/StartReviewDialog';
import { WorkspacesGuideDialog } from '@/shared/dialogs/shared/WorkspacesGuideDialog';
import { SettingsDialog } from '@/shared/dialogs/settings/SettingsDialog';
import { CreateWorkspaceFromPrDialog } from '@/shared/dialogs/command-bar/CreateWorkspaceFromPrDialog';
import { buildWorkspaceCreateInitialState } from '@/shared/lib/workspaceCreateState';
import { setCreateModeSeedState } from '@/features/create-mode/model/createModeSeedStore';

// Mirrored sidebar icon for right sidebar toggle
const RightSidebarIcon: Icon = forwardRef<SVGSVGElement, IconProps>(
  (props, ref) =>
    createElement(SidebarSimpleIcon, {
      ref,
      ...props,
      style: { transform: 'scaleX(-1)', ...props.style },
    })
);
RightSidebarIcon.displayName = 'RightSidebarIcon';

import type {
  ActionExecutorContext,
  ActionDefinition,
  GlobalActionDefinition,
  WorkspaceActionDefinition,
  IssueActionDefinition,
  NavbarItem,
} from '@/shared/types/actions';
import { ActionTargetType, NavbarDivider } from '@/shared/types/actions';

async function resolveLinkedIssue(
  workspaceId: string,
  remoteWorkspaces: {
    local_workspace_id: string | null;
    issue_id: string | null;
    project_id: string;
  }[]
): Promise<{ issueId: string; remoteProjectId: string } | undefined> {
  const remoteWs = remoteWorkspaces.find(
    (w) => w.local_workspace_id === workspaceId
  );
  if (remoteWs?.issue_id) {
    return { issueId: remoteWs.issue_id, remoteProjectId: remoteWs.project_id };
  }
  return undefined;
}

async function getWorkspace(
  queryClient: QueryClient,
  workspaceId: string
): Promise<Workspace> {
  const cached = queryClient.getQueryData<Workspace>(
    workspaceRecordKeys.byId(workspaceId)
  );
  if (cached) {
    return cached;
  }
  // Fetch from API if not in cache
  return workspacesApi.get(workspaceId);
}

// Helper to invalidate workspace-related queries
function invalidateWorkspaceQueries(
  queryClient: QueryClient,
  workspaceId: string
) {
  queryClient.invalidateQueries({
    queryKey: workspaceRecordKeys.byId(workspaceId),
  });
  queryClient.invalidateQueries({ queryKey: workspaceSummaryKeys.all });
}

// Helper to find the next workspace to navigate to when removing current workspace
function getNextWorkspaceId(
  activeWorkspaces: { id: string; isRunning?: boolean }[],
  removingWorkspaceId: string
): string | null {
  const currentIndex = activeWorkspaces.findIndex(
    (ws) => ws.id === removingWorkspaceId
  );
  if (currentIndex >= 0 && activeWorkspaces.length > 1) {
    const nextWorkspace =
      activeWorkspaces[currentIndex + 1] || activeWorkspaces[currentIndex - 1];
    return nextWorkspace?.id ?? null;
  }
  return null;
}

// Helper to navigate to create-issue form for a sub-issue, carrying over parent assignees
function navigateToCreateSubIssue(
  ctx: ActionExecutorContext,
  parentIssueId: string
) {
  const assigneeIds = ctx.projectMutations
    ?.getAssigneesForIssue(parentIssueId)
    .map((a) => a.user_id);
  ctx.navigateToCreateIssue({
    statusId: ctx.defaultCreateStatusId,
    parentIssueId,
    assigneeIds: assigneeIds?.length ? assigneeIds : undefined,
  });
}

// All application actions
export const Actions = {
  // === Workspace Actions ===
  DuplicateWorkspace: {
    id: 'duplicate-workspace',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.duplicate'),
    icon: CopyIcon,
    shortcut: 'W D',
    requiresTarget: ActionTargetType.WORKSPACE,
    execute: async (ctx, workspaceId) => {
      try {
        const [firstMessage, repos, workspaceWithSession] = await Promise.all([
          workspacesApi.getFirstUserMessage(workspaceId),
          workspacesApi.getRepos(workspaceId),
          workspacesApi.getWithSession(workspaceId),
        ]);

        const linkedIssue = await resolveLinkedIssue(
          workspaceId,
          ctx.remoteWorkspaces
        );

        const executorConfig = workspaceWithSession.session?.executor
          ? {
              executor: workspaceWithSession.session
                .executor as ExecutorConfig['executor'],
            }
          : null;

        const createState = buildWorkspaceCreateInitialState({
          prompt: firstMessage,
          defaults: {
            preferredRepos: repos.map((r) => ({
              repo_id: r.id,
              target_branch: r.target_branch,
            })),
          },
          linkedIssue,
          executorConfig,
        });
        setCreateModeSeedState(createState);
        ctx.appNavigation.goToWorkspacesCreate();
      } catch {
        ctx.appNavigation.goToWorkspacesCreate();
      }
    },
  },

  RenameWorkspace: {
    id: 'rename-workspace',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.rename'),
    icon: PencilSimpleIcon,
    shortcut: 'W R',
    requiresTarget: ActionTargetType.WORKSPACE,
    execute: async (ctx, workspaceId) => {
      const workspace = await getWorkspace(ctx.queryClient, workspaceId);
      await RenameWorkspaceDialog.show({
        currentName: workspace.name || workspace.branch,
        onRename: async (newName) => {
          await workspacesApi.update(workspaceId, { name: newName });
          invalidateWorkspaceQueries(ctx.queryClient, workspaceId);
        },
      });
    },
  },

  PinWorkspace: {
    id: 'pin-workspace',
    label: (workspace?: Workspace, t = i18n.getFixedT(null, 'tasks')) =>
      workspace?.pinned
        ? t('tasks:actionLabels.unpin')
        : t('tasks:actionLabels.pin'),
    icon: PushPinIcon,
    shortcut: 'W P',
    requiresTarget: ActionTargetType.WORKSPACE,
    execute: async (ctx, workspaceId) => {
      const workspace = await getWorkspace(ctx.queryClient, workspaceId);
      await workspacesApi.update(workspaceId, {
        pinned: !workspace.pinned,
      });
      invalidateWorkspaceQueries(ctx.queryClient, workspaceId);
    },
  },

  ArchiveWorkspace: {
    id: 'archive-workspace',
    label: (workspace?: Workspace, t = i18n.getFixedT(null, 'tasks')) =>
      workspace?.archived
        ? t('tasks:actionLabels.unarchive')
        : t('tasks:actionLabels.archive'),
    icon: ArchiveIcon,
    shortcut: 'W A',
    requiresTarget: ActionTargetType.WORKSPACE,
    isVisible: (ctx) => ctx.hasWorkspace && ctx.layoutMode === 'workspaces',
    isActive: (ctx) => ctx.workspaceArchived,
    execute: async (ctx, workspaceId) => {
      const workspace = await getWorkspace(ctx.queryClient, workspaceId);
      const wasArchived = workspace.archived;

      // Calculate next workspace before archiving
      const nextWorkspaceId = !wasArchived
        ? getNextWorkspaceId(ctx.activeWorkspaces, workspaceId)
        : null;

      if (wasArchived) {
        await workspacesApi.update(workspaceId, { archived: false });
      } else {
        // Reject busy workspaces before showing the single confirmation.
        const prepared = await workspacesApi.prepareArchive(workspaceId);
        const result = await ConfirmDialog.show({
          title: () => i18n.t('tasks:actionDialogs.archiveWorktree'),
          message: '',
          confirmText: () => i18n.t('tasks:actionLabels.archive'),
          variant: 'destructive',
        });
        if (result !== 'confirmed') return;
        const archived = await workspacesApi.confirmArchive(
          workspaceId,
          prepared.confirmation_token
        );
        invalidateWorkspaceQueries(ctx.queryClient, workspaceId);
        if (archived.deletion_error) throw new Error(archived.deletion_error);
      }
      invalidateWorkspaceQueries(ctx.queryClient, workspaceId);

      // Select next workspace after successful archive
      if (!wasArchived && nextWorkspaceId) {
        ctx.selectWorkspace(nextWorkspaceId);
      }
    },
  },

  DeleteWorkspace: {
    id: 'delete-workspace',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.delete'),
    icon: TrashIcon,
    shortcut: 'W X',
    variant: 'destructive',
    requiresTarget: ActionTargetType.WORKSPACE,
    execute: async (ctx, workspaceId) => {
      const workspace = await getWorkspace(ctx.queryClient, workspaceId);

      // Check if workspace is linked to a remote issue
      const remoteWs = ctx.remoteWorkspaces.find(
        (w) => w.local_workspace_id === workspaceId
      );
      const linkedIssueSimpleId = remoteWs?.issue_id
        ? ctx.projectMutations?.getIssue(remoteWs.issue_id)?.simple_id
        : undefined;
      const branchStatus = await workspacesApi.getBranchStatus(workspaceId);
      const hasOpenPR = branchStatus.some((repoStatus) =>
        repoStatus.merges?.some(
          (m: Merge) => m.type === 'pr' && m.pr_info.status === 'open'
        )
      );

      const result = await DeleteWorkspaceDialog.show({
        branchName: workspace.branch,
        hasOpenPR,
        isLinkedToIssue: Boolean(remoteWs?.issue_id),
        linkedIssueSimpleId,
      });
      if (result.action === 'confirmed') {
        // Calculate next workspace before deleting (only if deleting current)
        const isCurrentWorkspace = ctx.currentWorkspaceId === workspaceId;
        const nextWorkspaceId = isCurrentWorkspace
          ? getNextWorkspaceId(ctx.activeWorkspaces, workspaceId)
          : null;

        await workspacesApi.delete(workspaceId, result.deleteBranches);

        // Unlink from remote issue after successful deletion
        if (result.unlinkFromIssue) {
          await workspacesApi.unlinkFromIssue(workspaceId);
        }
        ctx.queryClient.invalidateQueries({
          queryKey: workspaceSummaryKeys.all,
        });

        // Navigate away if we deleted the current workspace
        if (isCurrentWorkspace) {
          if (nextWorkspaceId) {
            ctx.selectWorkspace(nextWorkspaceId);
          } else {
            ctx.appNavigation.goToWorkspacesCreate();
          }
        }
      }
    },
  },

  StartReview: {
    id: 'start-review',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.start_review'),
    icon: HighlighterIcon,
    requiresTarget: ActionTargetType.WORKSPACE,
    isVisible: (ctx) => ctx.hasWorkspace,
    getTooltip: (_ctx, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.review_changes_with_agent'),
    execute: async (_ctx, workspaceId) => {
      await StartReviewDialog.show({
        workspaceId,
      });
    },
  },

  SpinOffWorkspace: {
    id: 'spin-off-workspace',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.spin_off_workspace'),
    icon: GitForkIcon,
    requiresTarget: ActionTargetType.WORKSPACE,
    isVisible: (ctx) => ctx.hasWorkspace,
    execute: async (ctx, workspaceId) => {
      try {
        const [workspace, repos] = await Promise.all([
          getWorkspace(ctx.queryClient, workspaceId),
          workspacesApi.getRepos(workspaceId),
        ]);
        const linkedIssue = await resolveLinkedIssue(
          workspaceId,
          ctx.remoteWorkspaces
        );

        const createState = buildWorkspaceCreateInitialState({
          prompt: null,
          defaults: {
            preferredRepos: repos.map((r) => ({
              repo_id: r.id,
              target_branch: workspace.branch,
            })),
          },
          linkedIssue,
        });
        setCreateModeSeedState(createState);
        ctx.appNavigation.goToWorkspacesCreate();
      } catch {
        ctx.appNavigation.goToWorkspacesCreate();
      }
    },
  },

  // === Global/Navigation Actions ===
  NewWorkspace: {
    id: 'new-workspace',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.new_workspace'),
    icon: PlusIcon,
    shortcut: 'G N',
    requiresTarget: ActionTargetType.NONE,
    execute: (ctx) => {
      ctx.appNavigation.goToWorkspacesCreate();
    },
  },

  CreateWorkspaceFromPR: {
    id: 'create-workspace-from-pr',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.create_workspace_from_pr'),
    icon: GitPullRequestIcon,
    keywords: ['pull request'],
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) => ctx.layoutMode === 'workspaces',
    execute: async () => {
      await CreateWorkspaceFromPrDialog.show({});
    },
  } satisfies GlobalActionDefinition,

  Settings: {
    id: 'settings',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.settings'),
    icon: GearIcon,
    shortcut: 'G S',
    requiresTarget: ActionTargetType.NONE,
    execute: async () => {
      await SettingsDialog.show();
    },
  },

  ProjectSettings: {
    id: 'project-settings',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.project_settings'),
    icon: GearIcon,
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) => ctx.layoutMode === 'kanban',
    execute: async (ctx) => {
      await SettingsDialog.show({
        initialSection: 'remote-projects',
        initialState: {
          organizationId: ctx.kanbanOrgId,
          projectId: ctx.kanbanProjectId,
        },
      });
    },
  } satisfies GlobalActionDefinition,

  WorkspacesGuide: {
    id: 'workspaces-guide',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.workspaces_guide'),
    icon: QuestionIcon,
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) => ctx.layoutMode === 'workspaces',
    execute: async () => {
      await WorkspacesGuideDialog.show();
    },
  },

  ProjectsGuide: {
    id: 'projects-guide',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.projects_guide'),
    icon: QuestionIcon,
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) => ctx.layoutMode === 'kanban',
    execute: async () => {
      await ProjectsGuideDialog.show({
        interactiveHelp: createElement(InteractiveHelp),
        interactiveHelpTitle: i18n.t('common:interactiveHelp.title'),
      });
    },
  } satisfies GlobalActionDefinition,

  OpenCommandBar: {
    id: 'open-command-bar',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.open_command_bar'),
    icon: ListIcon,
    shortcut: '{mod} K',
    requiresTarget: ActionTargetType.NONE,
    execute: async () => {
      // Dynamic import to avoid circular dependency (pages.ts imports Actions)
      const { CommandBarDialog } = await import(
        '@/shared/dialogs/command-bar/CommandBarDialog'
      );
      CommandBarDialog.show();
    },
  },

  // === Diff View Actions ===
  ToggleDiffViewMode: {
    id: 'toggle-diff-view-mode',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      useDiffViewStore.getState().mode === 'unified'
        ? t('tasks:actionLabels.switch_to_side_by_side_view')
        : t('tasks:actionLabels.switch_to_inline_view'),
    icon: ColumnsIcon,
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) =>
      ctx.rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.CHANGES &&
      ctx.layoutMode === 'workspaces',
    isActive: (ctx) => ctx.diffViewMode === 'split',
    getIcon: (ctx) => (ctx.diffViewMode === 'split' ? ColumnsIcon : RowsIcon),
    getTooltip: (ctx, t = i18n.getFixedT(null, 'tasks')) =>
      ctx.diffViewMode === 'split'
        ? t('tasks:actionLabels.inline_view')
        : t('tasks:actionLabels.side_by_side_view'),
    execute: () => {
      useDiffViewStore.getState().toggle();
    },
  },

  ToggleIgnoreWhitespace: {
    id: 'toggle-ignore-whitespace',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      useDiffViewStore.getState().ignoreWhitespace
        ? t('tasks:actionLabels.show_whitespace_changes')
        : t('tasks:actionLabels.ignore_whitespace_changes'),
    icon: EyeSlashIcon,
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) =>
      ctx.rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.CHANGES &&
      ctx.layoutMode === 'workspaces',
    execute: () => {
      const store = useDiffViewStore.getState();
      store.setIgnoreWhitespace(!store.ignoreWhitespace);
    },
  },

  ToggleWrapLines: {
    id: 'toggle-wrap-lines',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      useDiffViewStore.getState().wrapText
        ? t('tasks:actionLabels.disable_line_wrapping')
        : t('tasks:actionLabels.enable_line_wrapping'),
    icon: TextAlignLeftIcon,
    shortcut: 'T W',
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) =>
      ctx.rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.CHANGES &&
      ctx.layoutMode === 'workspaces',
    execute: () => {
      const store = useDiffViewStore.getState();
      store.setWrapText(!store.wrapText);
    },
  },

  // === Layout Panel Actions ===
  ToggleLeftSidebar: {
    id: 'toggle-left-sidebar',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      useUiPreferencesStore.getState().isLeftSidebarVisible
        ? t('tasks:actionLabels.hide_left_sidebar')
        : t('tasks:actionLabels.show_left_sidebar'),
    icon: SidebarSimpleIcon,
    shortcut: 'V S',
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) => ctx.layoutMode === 'workspaces',
    isActive: (ctx) => ctx.isLeftSidebarVisible,
    execute: () => {
      useUiPreferencesStore.getState().toggleLeftSidebar();
    },
  },

  ToggleLeftMainPanel: {
    id: 'toggle-left-main-panel',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.toggle_chat_panel'),
    icon: ChatsTeardropIcon,
    shortcut: 'V H',
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) => ctx.layoutMode === 'workspaces',
    isActive: (ctx) => ctx.isLeftMainPanelVisible,
    isEnabled: (ctx) =>
      !(ctx.isLeftMainPanelVisible && ctx.rightMainPanelMode === null),
    getLabel: (ctx, t = i18n.getFixedT(null, 'tasks')) =>
      ctx.isLeftMainPanelVisible
        ? t('tasks:actionLabels.hide_chat_panel')
        : t('tasks:actionLabels.show_chat_panel'),
    execute: (ctx) => {
      useUiPreferencesStore
        .getState()
        .toggleLeftMainPanel(ctx.currentWorkspaceId ?? undefined);
    },
  },

  ToggleRightSidebar: {
    id: 'toggle-right-sidebar',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      useUiPreferencesStore.getState().isRightSidebarVisible
        ? t('tasks:actionLabels.hide_right_sidebar')
        : t('tasks:actionLabels.show_right_sidebar'),
    icon: RightSidebarIcon,
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) =>
      ctx.layoutMode === 'workspaces' || ctx.layoutMode === 'kanban',
    isActive: (ctx) =>
      window.matchMedia('(max-width: 767px)').matches &&
      ctx.layoutMode === 'workspaces'
        ? useUiPreferencesStore.getState().mobileActiveTab === 'git'
        : ctx.isRightSidebarVisible,
    getLabel: (ctx, t = i18n.getFixedT(null, 'tasks')) => {
      const visible =
        window.matchMedia('(max-width: 767px)').matches &&
        ctx.layoutMode === 'workspaces'
          ? useUiPreferencesStore.getState().mobileActiveTab === 'git'
          : ctx.isRightSidebarVisible;
      return t(
        visible
          ? 'tasks:actionLabels.hide_right_sidebar'
          : 'tasks:actionLabels.show_right_sidebar'
      );
    },
    getTooltip: (ctx, t = i18n.getFixedT(null, 'tasks')) => {
      const visible =
        window.matchMedia('(max-width: 767px)').matches &&
        ctx.layoutMode === 'workspaces'
          ? useUiPreferencesStore.getState().mobileActiveTab === 'git'
          : ctx.isRightSidebarVisible;
      return t(
        visible
          ? 'tasks:actionLabels.hide_right_sidebar'
          : 'tasks:actionLabels.show_right_sidebar'
      );
    },
    execute: (ctx) => {
      const state = useUiPreferencesStore.getState();
      if (
        window.matchMedia('(max-width: 767px)').matches &&
        !isProjectDestination(
          ctx.appNavigation.resolveFromPath(window.location.pathname)
        )
      ) {
        const opening = state.mobileActiveTab !== 'git';
        useUiPreferencesStore.setState({
          isRightSidebarVisible: opening,
          mobileActiveTab: opening ? 'git' : 'chat',
        });
      } else state.toggleRightSidebar();
    },
  },

  ToggleChangesMode: {
    id: 'toggle-changes-mode',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.toggle_changes_panel'),
    icon: GitDiffIcon,
    shortcut: 'V C',
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) => !ctx.isCreateMode && ctx.layoutMode === 'workspaces',
    isActive: (ctx) =>
      ctx.rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.CHANGES,
    isEnabled: (ctx) => !ctx.isCreateMode,
    getLabel: (ctx, t = i18n.getFixedT(null, 'tasks')) =>
      ctx.rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.CHANGES
        ? t('tasks:actionLabels.hide_changes_panel')
        : t('tasks:actionLabels.show_changes_panel'),
    execute: (ctx) => {
      useUiPreferencesStore
        .getState()
        .toggleRightMainPanelMode(
          RIGHT_MAIN_PANEL_MODES.CHANGES,
          ctx.currentWorkspaceId ?? undefined
        );
    },
  },

  ToggleFilesMode: {
    id: 'toggle-files-mode',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.toggle_files_panel'),
    icon: FilesIcon,
    shortcut: 'V F',
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) => !ctx.isCreateMode && ctx.layoutMode === 'workspaces',
    isActive: (ctx) => ctx.rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.FILES,
    isEnabled: (ctx) => !ctx.isCreateMode,
    getLabel: (ctx, t = i18n.getFixedT(null, 'tasks')) =>
      ctx.rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.FILES
        ? t('tasks:actionLabels.hide_files_panel')
        : t('tasks:actionLabels.show_files_panel'),
    execute: (ctx) => {
      useUiPreferencesStore
        .getState()
        .toggleRightMainPanelMode(
          RIGHT_MAIN_PANEL_MODES.FILES,
          ctx.currentWorkspaceId ?? undefined
        );
    },
  },

  ToggleTodosMode: {
    id: 'toggle-todos-mode',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.toggle_todo_list'),
    icon: ListChecksIcon,
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) => !ctx.isCreateMode && ctx.layoutMode === 'workspaces',
    isActive: (ctx) => ctx.rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.TODOS,
    isEnabled: (ctx) => !ctx.isCreateMode,
    getLabel: (ctx, t = i18n.getFixedT(null, 'tasks')) =>
      ctx.rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.TODOS
        ? t('tasks:actionLabels.hide_todo_list')
        : t('tasks:actionLabels.show_todo_list'),
    execute: (ctx) => {
      useUiPreferencesStore
        .getState()
        .toggleRightMainPanelMode(
          RIGHT_MAIN_PANEL_MODES.TODOS,
          ctx.currentWorkspaceId ?? undefined
        );
    },
  },

  ToggleLogsMode: {
    id: 'toggle-logs-mode',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.toggle_logs_panel'),
    icon: TerminalIcon,
    shortcut: 'V L',
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) => !ctx.isCreateMode && ctx.layoutMode === 'workspaces',
    isActive: (ctx) => ctx.rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.LOGS,
    isEnabled: (ctx) => !ctx.isCreateMode,
    getLabel: (ctx, t = i18n.getFixedT(null, 'tasks')) =>
      ctx.rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.LOGS
        ? t('tasks:actionLabels.hide_logs_panel')
        : t('tasks:actionLabels.show_logs_panel'),
    execute: (ctx) => {
      useUiPreferencesStore
        .getState()
        .toggleRightMainPanelMode(
          RIGHT_MAIN_PANEL_MODES.LOGS,
          ctx.currentWorkspaceId ?? undefined
        );
    },
  },

  ToggleWikiMode: {
    id: 'toggle-wiki-mode',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.toggle_wiki_reader'),
    icon: BookOpenIcon,
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) => !ctx.isCreateMode && ctx.layoutMode === 'workspaces',
    isActive: (ctx) => ctx.rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.WIKI,
    isEnabled: (ctx) => !ctx.isCreateMode && ctx.hasWorkspace,
    getLabel: (ctx, t = i18n.getFixedT(null, 'tasks')) =>
      ctx.rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.WIKI
        ? t('tasks:actionLabels.close_wiki_reader')
        : t('tasks:actionLabels.open_wiki_reader'),
    execute: (ctx) => {
      useUiPreferencesStore.setState({ isRightSidebarVisible: true });
      useUiPreferencesStore
        .getState()
        .toggleRightMainPanelMode(
          RIGHT_MAIN_PANEL_MODES.WIKI,
          ctx.currentWorkspaceId ?? undefined
        );
    },
  },

  // === Diff Actions for Navbar ===
  ToggleAllDiffs: {
    id: 'toggle-all-diffs',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) => {
      const diffPaths = Array.from(useWorkspaceDiffStore.getState().diffPaths);
      const { expanded } = useUiPreferencesStore.getState();
      const keys = diffPaths.map((p) => `diff:${p}`);
      const isAllExpanded =
        keys.length > 0 && keys.every((k) => expanded[k] !== false);
      return isAllExpanded
        ? t('tasks:actionLabels.collapse_all_diffs')
        : t('tasks:actionLabels.expand_all_diffs');
    },
    icon: CaretDoubleUpIcon,
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) =>
      ctx.rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.CHANGES &&
      ctx.layoutMode === 'workspaces',
    getIcon: (ctx) =>
      ctx.isAllDiffsExpanded ? CaretDoubleUpIcon : CaretDoubleDownIcon,
    getTooltip: (ctx, t = i18n.getFixedT(null, 'tasks')) =>
      ctx.isAllDiffsExpanded
        ? t('tasks:actionLabels.collapse_all_diffs')
        : t('tasks:actionLabels.expand_all_diffs'),
    execute: () => {
      const diffPaths = Array.from(useWorkspaceDiffStore.getState().diffPaths);
      const { expanded, setExpandedAll } = useUiPreferencesStore.getState();
      const keys = diffPaths.map((p) => `diff:${p}`);
      const isAllExpanded =
        keys.length > 0 && keys.every((k) => expanded[k] !== false);
      setExpandedAll(keys, !isAllExpanded);
    },
  },

  // === ContextBar Actions ===
  OpenInIDE: {
    id: 'open-in-ide',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.open_in_external_editor'),
    icon: 'ide-icon' as const,
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) => ctx.hasWorkspace,
    getTooltip: (_ctx, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.open_in_external_editor'),
    execute: async (ctx) => {
      if (ctx.currentWorkspaceId)
        await EditorSelectionDialog.show({
          selectedAttemptId: ctx.currentWorkspaceId,
        });
    },
  },

  CopyWorkspacePath: {
    id: 'copy-workspace-path',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.copy_workspace_path'),
    icon: 'copy-icon' as const,
    shortcut: 'Y P',
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) => ctx.hasWorkspace,
    execute: async (ctx) => {
      if (!ctx.containerRef) return;
      await navigator.clipboard.writeText(ctx.containerRef);
    },
  },

  CopyRawLogs: {
    id: 'copy-raw-logs',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.copy_raw_logs'),
    icon: CopyIcon,
    shortcut: 'Y L',
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) =>
      ctx.rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.LOGS &&
      ctx.logsPanelContent?.type !== 'terminal',
    execute: async (ctx) => {
      if (!ctx.currentLogs || ctx.currentLogs.length === 0) return;
      const rawText = ctx.currentLogs.map((log) => log.content).join('\n');
      await navigator.clipboard.writeText(rawText);
    },
  },

  ToggleDevServer: {
    id: 'toggle-dev-server',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.dev_server'),
    icon: PlayIcon,
    shortcut: 'T D',
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) => ctx.hasWorkspace,
    isEnabled: (ctx) =>
      ctx.devServerState !== 'starting' && ctx.devServerState !== 'stopping',
    getIcon: (ctx) => {
      if (
        ctx.devServerState === 'starting' ||
        ctx.devServerState === 'stopping'
      ) {
        return SpinnerIcon;
      }
      if (ctx.devServerState === 'running') {
        return PauseIcon;
      }
      return PlayIcon;
    },
    getTooltip: (ctx, t = i18n.getFixedT(null, 'tasks')) => {
      switch (ctx.devServerState) {
        case 'starting':
          return t('tasks:actionLabels.starting_dev_server');
        case 'stopping':
          return t('tasks:actionLabels.stopping_dev_server');
        case 'running':
          return t('tasks:actionLabels.stop_dev_server');
        default:
          return t('tasks:actionLabels.start_dev_server');
      }
    },
    getLabel: (ctx, t = i18n.getFixedT(null, 'tasks')) =>
      ctx.devServerState === 'running'
        ? t('tasks:actionLabels.stop_dev_server')
        : t('tasks:actionLabels.start_dev_server'),
    execute: (ctx) => {
      if (ctx.runningDevServers.length > 0) {
        ctx.stopDevServer();
      } else {
        ctx.startDevServer();
        // Managed development servers open their logs, never an embedded browser.
        useUiPreferencesStore
          .getState()
          .setRightMainPanelMode(
            RIGHT_MAIN_PANEL_MODES.LOGS,
            ctx.currentWorkspaceId ?? undefined
          );
      }
    },
  },

  // === Git Actions ===
  GitCreatePR: {
    id: 'git-create-pr',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.create_pull_request'),
    icon: GitPullRequestIcon,
    shortcut: 'X P',
    requiresTarget: ActionTargetType.GIT,
    isVisible: (ctx) => ctx.hasWorkspace && ctx.hasGitRepos,
    execute: async (ctx, workspaceId, repoId) => {
      const workspace = await getWorkspace(ctx.queryClient, workspaceId);

      const repos = await workspacesApi.getRepos(workspaceId);
      const repo = repos.find((r) => r.id === repoId);

      // Resolve vibe-kanban identifier from remote workspace + issue
      let issueIdentifier: string | undefined;
      const remoteWs = ctx.remoteWorkspaces.find(
        (w) => w.local_workspace_id === workspaceId
      );
      if (remoteWs?.issue_id && ctx.projectMutations?.getIssue) {
        const issue = ctx.projectMutations.getIssue(remoteWs.issue_id);
        issueIdentifier = issue?.simple_id || remoteWs.issue_id;
      }

      const result = await CreatePRDialog.show({
        attempt: workspace,
        repoId,
        targetBranch: repo?.target_branch,
        issueIdentifier,
      });

      if (!result.success && result.error) {
        throw new Error(result.error);
      }
    },
  },

  GitLinkPR: {
    id: 'git-link-pr',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.link_pull_request'),
    icon: LinkIcon,
    requiresTarget: ActionTargetType.GIT,
    isVisible: (ctx) => ctx.hasWorkspace && ctx.hasGitRepos && !ctx.hasOpenPR,
    execute: async (ctx, workspaceId, repoId) => {
      const result = await workspacesApi.attachPr(workspaceId, {
        repo_id: repoId,
      });

      if (result.success && result.data.pr_attached && result.data.pr_number) {
        invalidateWorkspaceQueries(ctx.queryClient, workspaceId);
        ctx.queryClient.invalidateQueries({
          queryKey: ['branch-status'],
        });

        await ConfirmDialog.show({
          title: () => i18n.t('tasks:actionDialogs.prLinked'),
          message: () =>
            i18n.t('tasks:actionDialogs.linkedPr', {
              number: result.data.pr_number,
            }) + (result.data.pr_url ? ` — ${result.data.pr_url}` : ''),
          confirmText: () => i18n.t('common:ok'),
          showCancelButton: false,
          variant: 'success',
        });
      } else if (result.success && !result.data.pr_attached) {
        await ConfirmDialog.show({
          title: () => i18n.t('tasks:actionDialogs.noPr'),
          message: () => i18n.t('tasks:actionDialogs.noPrBody'),
          confirmText: () => i18n.t('common:ok'),
          showCancelButton: false,
          variant: 'info',
        });
      } else if (!result.success) {
        throw new Error(result.message || 'Failed to attach PR');
      }
    },
  },

  GitAutoMerge: {
    id: 'git-auto-merge',
    label: () => i18n.t('common:integrationPanel.title'),
    icon: GitMergeIcon,
    shortcut: 'X M',
    requiresTarget: ActionTargetType.GIT,
    isVisible: (ctx) => ctx.hasWorkspace && ctx.hasGitRepos,
    execute: async (ctx, workspaceId, repoId) => {
      const linked = await resolveLinkedIssue(
        workspaceId,
        ctx.remoteWorkspaces
      );
      if (!linked)
        throw new Error(i18n.t('common:integrationPanel.linkRequired'));
      useTodoViewStore.getState().openWorkspaceIntegration({
        projectId: linked.remoteProjectId,
        workspaceId,
        repositoryId: repoId,
      });
      ctx.appNavigation.goToProject(linked.remoteProjectId);
    },
  },

  GitRebase: {
    id: 'git-rebase',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.rebase'),
    icon: ArrowsClockwiseIcon,
    shortcut: 'X R',
    requiresTarget: ActionTargetType.GIT,
    isVisible: (ctx) => ctx.hasWorkspace && ctx.hasGitRepos,
    execute: async (_ctx, workspaceId, repoId) => {
      // Open rebase dialog - it loads branches/status internally and handles conflicts
      await RebaseDialog.show({
        workspaceId: workspaceId,
        repoId,
      });
    },
  },

  GitChangeTarget: {
    id: 'git-change-target',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.change_target_branch'),
    icon: CrosshairIcon,
    requiresTarget: ActionTargetType.GIT,
    isVisible: (ctx) => ctx.hasWorkspace && ctx.hasGitRepos,
    execute: async (ctx, workspaceId, repoId) => {
      const branches = await repoApi.getBranches(repoId);
      await ChangeTargetDialog.show({
        branches: branches.map((branch) => ({
          name: branch.name,
          isCurrent: branch.is_current,
        })),
        onChangeTargetBranch: async (newTargetBranch) => {
          await workspacesApi.change_target_branch(workspaceId, {
            new_target_branch: newTargetBranch,
            repo_id: repoId,
          });

          ctx.queryClient.invalidateQueries({
            queryKey: ['branchStatus', workspaceId],
          });
          ctx.queryClient.invalidateQueries({
            queryKey: workspaceRecordKeys.byId(workspaceId),
          });
          ctx.queryClient.invalidateQueries({
            queryKey: workspaceRepoKeys.byWorkspace(workspaceId),
          });
          ctx.queryClient.invalidateQueries({
            queryKey: repoBranchKeys.byRepo(repoId),
          });
        },
      });
    },
  },

  GitPush: {
    id: 'git-push',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.push'),
    icon: ArrowUpIcon,
    shortcut: 'X U',
    requiresTarget: ActionTargetType.GIT,
    isVisible: (ctx) =>
      ctx.hasWorkspace &&
      ctx.hasGitRepos &&
      ctx.hasOpenPR &&
      ctx.hasUnpushedCommits,
    execute: async (ctx, workspaceId, repoId) => {
      const result = await workspacesApi.push(workspaceId, { repo_id: repoId });
      if (!result.success) {
        if (result.error?.type === 'force_push_required') {
          throw new ActionError('forcePushRequired');
        }
        throw new Error('Failed to push changes');
      }
      invalidateWorkspaceQueries(ctx.queryClient, workspaceId);
    },
  },

  // === Repo-specific Actions (for command bar when selecting a repo) ===
  RepoCopyPath: {
    id: 'repo-copy-path',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.copy_repo_path'),
    icon: CopyIcon,
    requiresTarget: ActionTargetType.GIT,
    isVisible: (ctx) => ctx.hasWorkspace && ctx.hasGitRepos,
    execute: async (_ctx, _workspaceId, repoId) => {
      try {
        const repo = await repoApi.getById(repoId);
        if (repo?.path) {
          await navigator.clipboard.writeText(repo.path);
        }
      } catch (err) {
        console.error('Failed to copy repo path:', err);
        throw new ActionError('copyPath');
      }
    },
  },

  RepoOpenInIDE: {
    id: 'repo-open-in-ide',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.open_repo_in_external_editor'),
    icon: DesktopIcon,
    requiresTarget: ActionTargetType.GIT,
    isVisible: (ctx) => ctx.hasWorkspace && ctx.hasGitRepos,
    execute: async (_ctx, workspaceId, repoId) => {
      await EditorSelectionDialog.show({
        selectedAttemptId: workspaceId,
        repoId,
      });
    },
  },

  RepoSettings: {
    id: 'repo-settings',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.repository_settings'),
    icon: GearIcon,
    requiresTarget: ActionTargetType.GIT,
    isVisible: (ctx) => ctx.hasWorkspace && ctx.hasGitRepos,
    execute: async (_ctx, _workspaceId, repoId) => {
      await SettingsDialog.show({
        initialSection: 'repos',
        initialState: {
          repoId,
        },
      });
    },
  },

  // === Script Actions ===
  RunSetupScript: {
    id: 'run-setup-script',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.run_setup_script'),
    icon: TerminalIcon,
    shortcut: 'R S',
    requiresTarget: ActionTargetType.WORKSPACE,
    isVisible: (ctx) => ctx.hasWorkspace,
    isEnabled: (ctx) => !ctx.isAttemptRunning,
    execute: async (_ctx, workspaceId) => {
      const result = await workspacesApi.runSetupScript(workspaceId);
      if (!result.success) {
        if (result.error?.type === 'no_script_configured') {
          throw new ActionError('noSetupScript');
        }
        if (result.error?.type === 'process_already_running') {
          throw new ActionError('processRunning');
        }
        throw new Error('Failed to run setup script');
      }
    },
  },

  RunCleanupScript: {
    id: 'run-cleanup-script',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.run_cleanup_script'),
    icon: TerminalIcon,
    shortcut: 'R C',
    requiresTarget: ActionTargetType.WORKSPACE,
    isVisible: (ctx) => ctx.hasWorkspace,
    isEnabled: (ctx) => !ctx.isAttemptRunning,
    execute: async (_ctx, workspaceId) => {
      const result = await workspacesApi.runCleanupScript(workspaceId);
      if (!result.success) {
        if (result.error?.type === 'no_script_configured') {
          throw new ActionError('noCleanupScript');
        }
        if (result.error?.type === 'process_already_running') {
          throw new ActionError('processRunning');
        }
        throw new Error('Failed to run cleanup script');
      }
    },
  },

  RunArchiveScript: {
    id: 'run-archive-script',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.run_archive_script'),
    icon: TerminalIcon,
    shortcut: 'R A',
    requiresTarget: ActionTargetType.WORKSPACE,
    isVisible: (ctx) => ctx.hasWorkspace,
    isEnabled: (ctx) => !ctx.isAttemptRunning,
    execute: async (_ctx, workspaceId) => {
      const result = await workspacesApi.runArchiveScript(workspaceId);
      if (!result.success) {
        if (result.error?.type === 'no_script_configured') {
          throw new ActionError('noArchiveScript');
        }
        if (result.error?.type === 'process_already_running') {
          throw new ActionError('processRunning');
        }
        throw new Error('Failed to run archive script');
      }
    },
  } satisfies WorkspaceActionDefinition,

  // === Issue Actions ===
  CreateIssue: {
    id: 'create-issue',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.create_issue'),
    icon: PlusIcon,
    shortcut: 'I C',
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) => ctx.layoutMode === 'kanban' && !ctx.isCreatingIssue,
    execute: (ctx) => {
      ctx.navigateToCreateIssue({ statusId: ctx.defaultCreateStatusId });
    },
  } satisfies GlobalActionDefinition,

  ChangeIssueStatus: {
    id: 'change-issue-status',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.change_status'),
    icon: ArrowsLeftRightIcon,
    shortcut: 'I S',
    requiresTarget: ActionTargetType.ISSUE,
    isVisible: (ctx) =>
      ctx.layoutMode === 'kanban' && ctx.hasSelectedKanbanIssue,
    execute: async (ctx, projectId, issueIds) => {
      await ctx.openStatusSelection(projectId, issueIds);
    },
  } satisfies IssueActionDefinition,

  ChangeNewIssueStatus: {
    id: 'change-new-issue-status',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.change_status'),
    icon: ArrowsLeftRightIcon,
    shortcut: 'I S',
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) => ctx.layoutMode === 'kanban' && ctx.isCreatingIssue,
    execute: async (ctx) => {
      if (!ctx.kanbanProjectId) return;
      const { ProjectSelectionDialog } = await import(
        '@/shared/dialogs/command-bar/selections/ProjectSelectionDialog'
      );
      await ProjectSelectionDialog.show({
        projectId: ctx.kanbanProjectId,
        selection: { type: 'status', issueIds: [], isCreateMode: true },
      });
    },
  } satisfies GlobalActionDefinition,

  ChangePriority: {
    id: 'change-issue-priority',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.change_priority'),
    icon: ArrowFatLineUpIcon,
    shortcut: 'I P',
    requiresTarget: ActionTargetType.ISSUE,
    isVisible: (ctx) =>
      ctx.layoutMode === 'kanban' && ctx.hasSelectedKanbanIssue,
    execute: async (ctx, projectId, issueIds) => {
      await ctx.openPrioritySelection(projectId, issueIds);
    },
  } satisfies IssueActionDefinition,

  ChangeNewIssuePriority: {
    id: 'change-new-issue-priority',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.change_priority'),
    icon: ArrowFatLineUpIcon,
    shortcut: 'I P',
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) => ctx.layoutMode === 'kanban' && ctx.isCreatingIssue,
    execute: async (ctx) => {
      if (!ctx.kanbanProjectId) return;
      const { ProjectSelectionDialog } = await import(
        '@/shared/dialogs/command-bar/selections/ProjectSelectionDialog'
      );
      await ProjectSelectionDialog.show({
        projectId: ctx.kanbanProjectId,
        selection: { type: 'priority', issueIds: [], isCreateMode: true },
      });
    },
  } satisfies GlobalActionDefinition,

  ChangeAssignees: {
    id: 'change-assignees',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.change_assignees'),
    icon: UsersIcon,
    shortcut: 'I A',
    requiresTarget: ActionTargetType.ISSUE,
    isVisible: (ctx) =>
      ctx.layoutMode === 'kanban' && ctx.hasSelectedKanbanIssue,
    execute: async (ctx, projectId, issueIds) => {
      await ctx.openAssigneeSelection(projectId, issueIds, false);
    },
  } satisfies IssueActionDefinition,

  ChangeNewIssueAssignees: {
    id: 'change-new-issue-assignees',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.change_assignees'),
    icon: UsersIcon,
    shortcut: 'I A',
    requiresTarget: ActionTargetType.NONE,
    isVisible: (ctx) => ctx.layoutMode === 'kanban' && ctx.isCreatingIssue,
    execute: async (ctx) => {
      // Opens assignee selection for the issue being created
      // ProjectId will be resolved from route params inside the dialog
      await ctx.openAssigneeSelection('', [], true);
    },
  } satisfies GlobalActionDefinition,

  MakeSubIssueOf: {
    id: 'make-sub-issue-of',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.make_sub_issue_of'),
    icon: TreeStructureIcon,
    shortcut: 'I M',
    requiresTarget: ActionTargetType.ISSUE,
    isVisible: (ctx) =>
      ctx.layoutMode === 'kanban' && ctx.hasSelectedKanbanIssue,
    execute: async (ctx, projectId, issueIds) => {
      if (issueIds.length === 1) {
        await ctx.openSubIssueSelection(projectId, issueIds[0], 'setParent');
      }
    },
  } satisfies IssueActionDefinition,

  AddSubIssue: {
    id: 'add-sub-issue',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.add_sub_issue'),
    icon: PlusIcon,
    shortcut: 'I B',
    requiresTarget: ActionTargetType.ISSUE,
    isVisible: (ctx) =>
      ctx.layoutMode === 'kanban' && ctx.hasSelectedKanbanIssue,
    execute: async (ctx, projectId, issueIds) => {
      if (issueIds.length !== 1) return;
      const parentIssueId = issueIds[0];
      const result = await ctx.openSubIssueSelection(
        projectId,
        parentIssueId,
        'addChild'
      );
      if (result?.type === 'createNew') {
        navigateToCreateSubIssue(ctx, parentIssueId);
      }
    },
  } satisfies IssueActionDefinition,

  CreateSubIssue: {
    id: 'create-sub-issue',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.create_sub_issue'),
    icon: PlusIcon,
    requiresTarget: ActionTargetType.ISSUE,
    isVisible: (ctx) =>
      ctx.layoutMode === 'kanban' && ctx.hasSelectedKanbanIssue,
    execute: async (ctx, _projectId, issueIds) => {
      if (issueIds.length !== 1) return;
      navigateToCreateSubIssue(ctx, issueIds[0]);
    },
  } satisfies IssueActionDefinition,

  RemoveParentIssue: {
    id: 'remove-parent-issue',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.remove_parent'),
    icon: XIcon,
    shortcut: 'I U',
    requiresTarget: ActionTargetType.ISSUE,
    isVisible: (ctx) =>
      ctx.layoutMode === 'kanban' &&
      ctx.hasSelectedKanbanIssue &&
      ctx.hasSelectedKanbanIssueParent,
    execute: async (_ctx, _projectId, issueIds) => {
      await bulkUpdateIssues(
        issueIds.map((issueId) => ({
          id: issueId,
          changes: {
            parent_issue_id: null,
            parent_issue_sort_order: null,
          },
        }))
      );
    },
  } satisfies IssueActionDefinition,

  LinkWorkspace: {
    id: 'link-workspace',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.link_workspace'),
    icon: LinkIcon,
    shortcut: 'I W',
    requiresTarget: ActionTargetType.ISSUE,
    isEnabled: (ctx) => !ctx.selectedIssueArchived,
    getLabel: (ctx, t = i18n.getFixedT(null, 'tasks')) =>
      ctx.selectedIssueArchived
        ? t('tasks:actionLabels.link_workspace_archived_hint')
        : t('tasks:actionLabels.link_workspace'),
    isVisible: (ctx) =>
      ctx.layoutMode === 'kanban' && ctx.hasSelectedKanbanIssue,
    execute: async (ctx, projectId, issueIds) => {
      if (issueIds.length === 1) {
        await ctx.openWorkspaceSelection(projectId, issueIds[0]);
      }
    },
  } satisfies IssueActionDefinition,

  ArchiveIssue: {
    id: 'archive-issue',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.archive_issue'),
    getLabel: (ctx, t = i18n.getFixedT(null, 'tasks')) =>
      ctx.selectedIssueArchived
        ? t('tasks:actionLabels.unarchive_issue')
        : t('tasks:actionLabels.archive_issue'),
    icon: ArchiveIcon,
    requiresTarget: ActionTargetType.ISSUE,
    isVisible: (ctx) =>
      ctx.layoutMode === 'kanban' &&
      ctx.hasSelectedKanbanIssue &&
      (ctx.selectedIssueCount ?? 1) === 1,
    execute: async (ctx, projectId, issueIds) => {
      if (issueIds.length !== 1) return;
      const issue = ctx.projectMutations?.getIssue(issueIds[0]);
      if (issue?.archived) {
        const requestId = crypto.randomUUID();
        let receipt;
        try {
          receipt = await issueArchiveApi.restore(
            projectId,
            issueIds[0],
            requestId
          );
        } catch (error) {
          throw new IssueArchiveActionError(
            { kind: 'request', requestId },
            errorDiagnostic(error)
          );
        }
        refreshIssueArchive(projectId);
        const message = receiptMessage(receipt, i18n.getFixedT(null, 'tasks'));
        if (message)
          throw new IssueArchiveActionError({ kind: 'receipt', receipt });
      } else {
        await IssueArchiveDialog.show({ projectId, issueId: issueIds[0] });
      }
    },
  } satisfies IssueActionDefinition,

  DeleteIssue: {
    id: 'delete-issue',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.delete_issue'),
    icon: TrashIcon,
    shortcut: 'I X',
    variant: 'destructive',
    requiresTarget: ActionTargetType.ISSUE,
    isVisible: (ctx) =>
      ctx.layoutMode === 'kanban' && ctx.hasSelectedKanbanIssue,
    execute: async (ctx, _projectId, issueIds) => {
      const count = issueIds.length;
      const result = await ConfirmDialog.show({
        title: () =>
          i18n.t('tasks:actionDialogs.deleteIssues', {
            count,
            formattedCount: new Intl.NumberFormat(
              i18n.resolvedLanguage || 'en'
            ).format(count),
          }),
        message: () =>
          i18n.t('tasks:actionDialogs.deleteIssuesBody', {
            count,
            formattedCount: new Intl.NumberFormat(
              i18n.resolvedLanguage || 'en'
            ).format(count),
          }),
        confirmText: () => i18n.t('common:buttons.delete'),
        cancelText: () => i18n.t('common:buttons.cancel'),
        variant: 'destructive',
      });
      if (result === 'confirmed' && ctx.projectMutations?.removeIssue) {
        for (const issueId of issueIds) {
          ctx.projectMutations.removeIssue(issueId);
        }
      }
    },
  } satisfies IssueActionDefinition,

  DuplicateIssue: {
    id: 'duplicate-issue',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.duplicate_issue'),
    icon: CopyIcon,
    shortcut: 'I D',
    requiresTarget: ActionTargetType.ISSUE,
    isVisible: (ctx) =>
      ctx.layoutMode === 'kanban' && ctx.hasSelectedKanbanIssue,
    execute: async (ctx, _projectId, issueIds) => {
      if (issueIds.length !== 1) {
        throw new ActionError('singleDuplicate');
      }
      ctx.projectMutations?.duplicateIssue(issueIds[0]);
    },
  } satisfies IssueActionDefinition,

  MarkBlocking: {
    id: 'mark-blocking',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.mark_blocking'),
    icon: ArrowBendUpRightIcon,
    requiresTarget: ActionTargetType.ISSUE,
    isVisible: (ctx) =>
      ctx.layoutMode === 'kanban' && ctx.hasSelectedKanbanIssue,
    execute: async (ctx, projectId, issueIds) => {
      if (issueIds.length === 1) {
        await ctx.openRelationshipSelection(
          projectId,
          issueIds[0],
          'blocking',
          'forward'
        );
      }
    },
  } satisfies IssueActionDefinition,

  MarkBlockedBy: {
    id: 'mark-blocked-by',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.mark_blocked_by'),
    icon: ProhibitIcon,
    requiresTarget: ActionTargetType.ISSUE,
    isVisible: (ctx) =>
      ctx.layoutMode === 'kanban' && ctx.hasSelectedKanbanIssue,
    execute: async (ctx, projectId, issueIds) => {
      if (issueIds.length === 1) {
        await ctx.openRelationshipSelection(
          projectId,
          issueIds[0],
          'blocking',
          'reverse'
        );
      }
    },
  } satisfies IssueActionDefinition,

  MarkRelated: {
    id: 'mark-related',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.mark_related'),
    icon: ArrowsLeftRightIcon,
    requiresTarget: ActionTargetType.ISSUE,
    isVisible: (ctx) =>
      ctx.layoutMode === 'kanban' && ctx.hasSelectedKanbanIssue,
    execute: async (ctx, projectId, issueIds) => {
      if (issueIds.length === 1) {
        await ctx.openRelationshipSelection(
          projectId,
          issueIds[0],
          'related',
          'forward'
        );
      }
    },
  } satisfies IssueActionDefinition,

  MarkDuplicateOf: {
    id: 'mark-duplicate-of',
    label: (_workspace, t = i18n.getFixedT(null, 'tasks')) =>
      t('tasks:actionLabels.mark_duplicate_of'),
    icon: CopyIcon,
    requiresTarget: ActionTargetType.ISSUE,
    isVisible: (ctx) =>
      ctx.layoutMode === 'kanban' && ctx.hasSelectedKanbanIssue,
    execute: async (ctx, projectId, issueIds) => {
      if (issueIds.length === 1) {
        await ctx.openRelationshipSelection(
          projectId,
          issueIds[0],
          'has_duplicate',
          'forward'
        );
      }
    },
  } satisfies IssueActionDefinition,
} as const satisfies Record<string, ActionDefinition>;

// Navbar action groups define which actions appear in each section
export const NavbarActionGroups = {
  left: [Actions.ArchiveWorkspace] as NavbarItem[],
  right: [
    Actions.ToggleDiffViewMode,
    Actions.ToggleAllDiffs,
    NavbarDivider,
    Actions.ToggleLeftSidebar,
    Actions.ToggleLeftMainPanel,
    Actions.ToggleChangesMode,
    Actions.ToggleFilesMode,
    Actions.ToggleTodosMode,
    Actions.ToggleWikiMode,
    Actions.ToggleLogsMode,
    Actions.ToggleRightSidebar,
    NavbarDivider,
    Actions.OpenCommandBar,
    Actions.WorkspacesGuide,
    Actions.ProjectsGuide,
    Actions.Settings,
  ] as NavbarItem[],
};

// ContextBar action groups define which actions appear in each section
export const ContextBarActionGroups = {
  primary: [Actions.OpenInIDE, Actions.CopyWorkspacePath] as ActionDefinition[],
  secondary: [
    Actions.ToggleDevServer,
    Actions.ToggleFilesMode,
    Actions.ToggleChangesMode,
  ] as ActionDefinition[],
};
