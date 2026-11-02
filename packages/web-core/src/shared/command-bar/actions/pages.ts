import { Actions } from '@/shared/actions';
import type { ActionDefinition } from '@/shared/types/actions';
import { RIGHT_MAIN_PANEL_MODES } from '@/shared/stores/useUiPreferencesStore';
import type { StaticPageId, CommandBarPage } from '@/shared/types/commandBar';

export const Pages: Record<StaticPageId, CommandBarPage> = {
  // Root page - shown when opening via CMD+K
  root: {
    id: 'root',
    items: [
      {
        type: 'group',
        label: (t) => t('tasks:commandPages.actions'),
        items: [
          { type: 'action', action: Actions.NewWorkspace },
          { type: 'action', action: Actions.CreateWorkspaceFromPR },
          { type: 'action', action: Actions.OpenInIDE },
          { type: 'action', action: Actions.CopyWorkspacePath },
          { type: 'action', action: Actions.CopyRawLogs },
          { type: 'action', action: Actions.ToggleDevServer },

          { type: 'childPages', id: 'workspaceActions' },
          { type: 'childPages', id: 'repoActions' },
          { type: 'childPages', id: 'issueActions' },
        ],
      },
      {
        type: 'group',
        label: (t) => t('tasks:commandPages.view'),
        items: [
          { type: 'childPages', id: 'viewOptions' },
          { type: 'childPages', id: 'diffOptions' },
        ],
      },
      {
        type: 'group',
        label: (t) => t('tasks:commandPages.general'),
        items: [
          { type: 'action', action: Actions.WorkspacesGuide },
          { type: 'action', action: Actions.ProjectsGuide },
          { type: 'action', action: Actions.ProjectSettings },
          { type: 'action', action: Actions.Settings },
        ],
      },
    ],
  },

  // Workspace actions page - shown when clicking three-dots on a workspace
  workspaceActions: {
    id: 'workspace-actions',
    title: (t) => t('tasks:commandPages.workspaceActions'),
    parent: 'root',
    isVisible: (ctx) => ctx.hasWorkspace,
    items: [
      {
        type: 'group',
        label: (t) => t('tasks:commandPages.workspace'),
        items: [
          { type: 'action', action: Actions.StartReview },
          { type: 'action', action: Actions.RenameWorkspace },
          { type: 'action', action: Actions.DuplicateWorkspace },
          { type: 'action', action: Actions.SpinOffWorkspace },
          { type: 'action', action: Actions.PinWorkspace },
          { type: 'action', action: Actions.ArchiveWorkspace },
          { type: 'action', action: Actions.DeleteWorkspace },
        ],
      },
      {
        type: 'group',
        label: (t) => t('tasks:commandPages.scripts'),
        items: [
          { type: 'action', action: Actions.RunSetupScript },
          { type: 'action', action: Actions.RunCleanupScript },
          { type: 'action', action: Actions.RunArchiveScript },
        ],
      },
    ],
  },

  // Diff options page - shown when changes panel is visible
  diffOptions: {
    id: 'diff-options',
    title: (t) => t('tasks:commandPages.diffOptions'),
    parent: 'root',
    isVisible: (ctx) =>
      ctx.rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.CHANGES,
    items: [
      {
        type: 'group',
        label: (t) => t('tasks:commandPages.display'),
        items: [
          { type: 'action', action: Actions.ToggleDiffViewMode },
          { type: 'action', action: Actions.ToggleWrapLines },
          { type: 'action', action: Actions.ToggleIgnoreWhitespace },
          { type: 'action', action: Actions.ToggleAllDiffs },
        ],
      },
    ],
  },

  // View options page - layout panel controls
  viewOptions: {
    id: 'view-options',
    title: (t) => t('tasks:commandPages.viewOptions'),
    parent: 'root',
    isVisible: (ctx) => ctx.layoutMode === 'workspaces',
    items: [
      {
        type: 'group',
        label: (t) => t('tasks:commandPages.panels'),
        items: [
          { type: 'action', action: Actions.ToggleLeftSidebar },
          { type: 'action', action: Actions.ToggleLeftMainPanel },
          { type: 'action', action: Actions.ToggleRightSidebar },
          { type: 'action', action: Actions.ToggleChangesMode },
          { type: 'action', action: Actions.ToggleFilesMode },
          { type: 'action', action: Actions.ToggleLogsMode },
        ],
      },
    ],
  },

  // Repository actions page - shown when clicking "..." on a repo card or via CMD+K
  repoActions: {
    id: 'repo-actions',
    title: (t) => t('tasks:commandPages.repoActions'),
    parent: 'root',
    isVisible: (ctx) => ctx.hasWorkspace && ctx.hasGitRepos,
    items: [
      {
        type: 'group',
        label: (t) => t('tasks:commandPages.actions'),
        items: [
          { type: 'action', action: Actions.RepoCopyPath },
          { type: 'action', action: Actions.RepoOpenInIDE },
          { type: 'action', action: Actions.RepoSettings },
          { type: 'action', action: Actions.GitCreatePR },
          { type: 'action', action: Actions.GitLinkPR },
          { type: 'action', action: Actions.GitMerge },
          { type: 'action', action: Actions.GitPush },
          { type: 'action', action: Actions.GitRebase },
          { type: 'action', action: Actions.GitChangeTarget },
        ],
      },
    ],
  },

  // Issue actions page - shown in kanban mode
  issueActions: {
    id: 'issue-actions',
    title: (t) => t('tasks:commandPages.issueActions'),
    parent: 'root',
    isVisible: (ctx) => ctx.layoutMode === 'kanban',
    items: [
      {
        type: 'group',
        label: (t) => t('tasks:commandPages.actions'),
        items: [
          { type: 'action', action: Actions.CreateIssue },
          { type: 'action', action: Actions.ChangeIssueStatus },
          { type: 'action', action: Actions.ChangeNewIssueStatus },
          { type: 'action', action: Actions.ChangePriority },
          { type: 'action', action: Actions.ChangeNewIssuePriority },
          { type: 'action', action: Actions.ChangeAssignees },
          { type: 'action', action: Actions.ChangeNewIssueAssignees },
          { type: 'action', action: Actions.MakeSubIssueOf },
          { type: 'action', action: Actions.AddSubIssue },
          { type: 'action', action: Actions.RemoveParentIssue },
          { type: 'action', action: Actions.LinkWorkspace },
          { type: 'action', action: Actions.MarkBlocking },
          { type: 'action', action: Actions.MarkBlockedBy },
          { type: 'action', action: Actions.MarkRelated },
          { type: 'action', action: Actions.MarkDuplicateOf },
          { type: 'action', action: Actions.DuplicateIssue },
          { type: 'action', action: Actions.ArchiveIssue },
          { type: 'action', action: Actions.DeleteIssue },
        ],
      },
    ],
  },
};

// Get all actions from a specific page
export function getPageActions(pageId: StaticPageId): ActionDefinition[] {
  const page = Pages[pageId];
  const actions: ActionDefinition[] = [];

  for (const group of page.items) {
    for (const item of group.items) {
      if (item.type === 'action') {
        actions.push(item.action);
      }
    }
  }

  return actions;
}
