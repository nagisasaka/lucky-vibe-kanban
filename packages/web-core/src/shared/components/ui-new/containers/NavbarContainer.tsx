import { useWikiAvailability } from '@/shared/hooks/useWikiAvailability';
import { useQuery } from '@tanstack/react-query';
import { readUtility, type MainRepo } from '@/shared/lib/projectUtilitiesApi';
import { BookOpenIcon, GitMergeIcon } from '@phosphor-icons/react';
import { useTranslation } from 'react-i18next';
import { useTodoViewStore } from '@/features/todos/model/todoViewStore';
import { useMemo, useCallback } from 'react';

import { useWorkspaceContext } from '@/shared/hooks/useWorkspaceContext';
import { useActions } from '@/shared/hooks/useActions';
import { useSyncErrorContext } from '@/shared/hooks/useSyncErrorContext';
import {
  Navbar,
  MOBILE_TABS,
  type NavbarSectionItem,
  type NavbarBreadcrumbItem,
  type MobileTabId,
} from '@vibe/ui/components/Navbar';
import { AppBarUserPopoverContainer } from './AppBarUserPopoverContainer';

import { NavbarActionGroups } from '@/shared/actions';
import {
  NavbarDivider,
  type ActionDefinition,
  type ActionTranslator,
  type NavbarItem as ActionNavbarItem,
  type ActionVisibilityContext,
  isSpecialIcon,
  getActionIcon,
  getActionTooltip,
  isActionActive,
  isActionEnabled,
  isActionVisible,
} from '@/shared/types/actions';
import { useActionVisibilityContext } from '@/shared/hooks/useActionVisibilityContext';
import {
  useMobileActiveTab,
  useUiPreferencesStore,
} from '@/shared/stores/useUiPreferencesStore';
import { CommandBarDialog } from '@/shared/dialogs/command-bar/CommandBarDialog';
import { SettingsDialog } from '@/shared/dialogs/settings/SettingsDialog';
import { getProjectDestination } from '@/shared/lib/routes/appNavigation';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { useCurrentAppDestination } from '@/shared/hooks/useCurrentAppDestination';
import { buildWorkspaceContext } from '@/shared/lib/workspaceContext';

/**
 * Check if a NavbarItem is a divider
 */
function isDivider(item: ActionNavbarItem): item is typeof NavbarDivider {
  return 'type' in item && item.type === 'divider';
}

/**
 * Filter navbar items by visibility, keeping dividers but removing them
 * if they would appear at the start, end, or consecutively.
 */
function filterNavbarItems(
  items: readonly ActionNavbarItem[],
  ctx: ActionVisibilityContext
): ActionNavbarItem[] {
  // Filter actions by visibility, keep dividers
  const filtered = items.filter((item) => {
    if (isDivider(item)) return true;
    if (!isActionVisible(item, ctx)) return false;
    return !isSpecialIcon(getActionIcon(item, ctx));
  });

  // Remove leading/trailing dividers and consecutive dividers
  const result: ActionNavbarItem[] = [];
  for (const item of filtered) {
    if (isDivider(item)) {
      // Only add divider if we have items before it and last item wasn't a divider
      if (result.length > 0 && !isDivider(result[result.length - 1])) {
        result.push(item);
      }
    } else {
      result.push(item);
    }
  }

  // Remove trailing divider
  if (result.length > 0 && isDivider(result[result.length - 1])) {
    result.pop();
  }

  return result;
}

function toNavbarSectionItems(
  items: readonly ActionNavbarItem[],
  ctx: ActionVisibilityContext,
  onExecuteAction: (action: ActionDefinition) => void,
  t: ActionTranslator
): NavbarSectionItem[] {
  return items.reduce<NavbarSectionItem[]>((result, item) => {
    if (isDivider(item)) {
      result.push({ type: 'divider' });
      return result;
    }

    const icon = getActionIcon(item, ctx);
    if (isSpecialIcon(icon)) {
      return result;
    }

    result.push({
      type: 'action',
      id: item.id,
      icon,
      isActive: isActionActive(item, ctx),
      tooltip: getActionTooltip(item, ctx, t),
      shortcut: item.shortcut,
      disabled: !isActionEnabled(item, ctx),
      onClick: () => onExecuteAction(item),
    });
    return result;
  }, []);
}

export function NavbarContainer({
  mobileMode = false,
  onOpenDrawer,
}: {
  mobileMode?: boolean;
  onOpenDrawer?: () => void;
}) {
  const { t } = useTranslation('common');
  const openIntegration = useTodoViewStore((s) => s.setIntegrationProject);
  const { executeAction } = useActions();
  const {
    workspace: selectedWorkspace,
    selectedSession,
    repos,
    isCreateMode,
  } = useWorkspaceContext();
  const syncErrorContext = useSyncErrorContext();
  const appNavigation = useAppNavigation();
  const destination = useCurrentAppDestination();
  const projectDestination = useMemo(
    () => getProjectDestination(destination),
    [destination]
  );
  const isOnProjectPage = projectDestination !== null;
  const projectId = projectDestination?.projectId ?? null;
  const mainInfo = useQuery({
    queryKey: ['project-main-utilities', projectId],
    queryFn: () =>
      readUtility<{ repos: MainRepo[] }>(
        `/api/projects/${projectId}/utilities/info`
      ),
    enabled: Boolean(projectId),
    staleTime: 30000,
  });
  const mainWikiItem =
    projectId &&
    !mainInfo.isError &&
    mainInfo.data?.repos.some((repo) => repo.wiki_enabled === true)
      ? [
          {
            id: 'project-wiki',
            icon: BookOpenIcon,
            tooltip: t('workspaceWiki.open'),
            onClick: () => {
              useUiPreferencesStore
                .getState()
                .setExpanded(`kanban-wiki:${projectId}`, true);
              useUiPreferencesStore.setState({ isRightSidebarVisible: true });
            },
          },
        ]
      : [];
  const isOnProjectSubRoute =
    projectDestination !== null && projectDestination.kind !== 'project';
  const [mobileActiveTab, setMobileActiveTab] = useMobileActiveTab();

  const enabledWikiRepos = useWikiAvailability(repos.map((repo) => repo.id));
  // Only data availability restricts tabs; mobile has the same workspace tools.
  const mobileWorkspaceTabs = useMemo(
    () =>
      MOBILE_TABS.filter(
        (tab) =>
          (tab.id !== 'wiki' || enabledWikiRepos.length > 0) &&
          (tab.id === 'workspaces' ||
            tab.id === 'chat' ||
            (!isCreateMode && Boolean(selectedWorkspace)))
      ),
    [isCreateMode, selectedWorkspace, enabledWikiRepos.length]
  );

  const handleMobileTabChange = useCallback(
    (tab: MobileTabId) => {
      if (tab === 'todos' || tab === 'wiki')
        useUiPreferencesStore
          .getState()
          .setRightMainPanelMode(tab, selectedWorkspace?.id);
      setMobileActiveTab(tab);
    },
    [selectedWorkspace?.id, setMobileActiveTab]
  );

  // Get action visibility context (includes all state for visibility/active/enabled)
  const actionCtx = useActionVisibilityContext();

  // Action handler - all actions go through the standard executeAction
  const handleExecuteAction = useCallback(
    (action: ActionDefinition) => {
      if (action.requiresTarget && selectedWorkspace?.id) {
        executeAction(action, selectedWorkspace.id);
      } else {
        executeAction(action);
      }
    },
    [executeAction, selectedWorkspace?.id]
  );

  const leftItems = useMemo(
    () =>
      toNavbarSectionItems(
        filterNavbarItems(NavbarActionGroups.left, actionCtx),
        actionCtx,
        handleExecuteAction,
        t
      ),
    [actionCtx, handleExecuteAction, t]
  );

  const rightItems = toNavbarSectionItems(
    filterNavbarItems(NavbarActionGroups.right, actionCtx).filter(
      (item) =>
        !('id' in item) ||
        item.id !== 'toggle-wiki-mode' ||
        enabledWikiRepos.length > 0
    ),
    actionCtx,
    handleExecuteAction,
    t
  );

  const workspaceParts = useMemo(() => {
    if (isOnProjectPage || isCreateMode || !selectedWorkspace) return undefined;

    return buildWorkspaceContext({
      containerRef: selectedWorkspace.container_ref,
      workingDir: selectedSession?.agent_working_dir,
      fallbackRepoName: repos.length === 1 ? repos[0].name : undefined,
      branch: selectedWorkspace.branch,
      workspaceKind: selectedWorkspace.workspace_kind,
    });
  }, [
    isCreateMode,
    isOnProjectPage,
    repos,
    selectedSession?.agent_working_dir,
    selectedWorkspace,
  ]);

  const workspaceContext = useMemo((): NavbarBreadcrumbItem[] | undefined => {
    return workspaceParts?.length
      ? workspaceParts.map((part) => ({ label: part.label }))
      : undefined;
  }, [workspaceParts]);

  const workspacePath = workspaceParts?.find(
    (part) => part.kind === 'path'
  )?.label;

  const navbarTitle = isCreateMode
    ? 'Create Workspace'
    : isOnProjectPage
      ? undefined
      : workspacePath ||
        (selectedWorkspace?.branch === 'direct-folder'
          ? ''
          : selectedWorkspace?.branch);

  // Mobile-specific callbacks
  const handleOpenCommandBar = useCallback(() => {
    CommandBarDialog.show();
  }, []);

  const handleOpenSettings = useCallback(() => {
    SettingsDialog.show();
  }, []);

  const handleNavigateBack = useCallback(() => {
    if (isOnProjectPage && projectId) {
      // On project sub-route: go back to project root (kanban board)
      appNavigation.goToProject(projectId);
    } else {
      // Non-project page: go to workspaces
      appNavigation.goToWorkspaces();
    }
  }, [isOnProjectPage, projectId, appNavigation]);

  const handleNavigateToBoard = useMemo(() => {
    if (!isOnProjectPage || !projectId) return null;
    return () => {
      appNavigation.goToProject(projectId);
    };
  }, [isOnProjectPage, projectId, appNavigation]);

  // Build user popover slot for mobile mode
  const userPopoverSlot = useMemo(() => {
    if (!mobileMode) return undefined;
    return <AppBarUserPopoverContainer />;
  }, [mobileMode]);

  const syncErrors = useMemo(() => {
    const errors = syncErrorContext?.errors ? [...syncErrorContext.errors] : [];

    return errors;
  }, [syncErrorContext?.errors]);

  return (
    <Navbar
      workspaceTitle={navbarTitle}
      breadcrumbs={workspaceContext}
      leftItems={leftItems}
      rightItems={
        projectDestination?.kind === 'project' && projectId
          ? [
              {
                id: 'project-integration',
                icon: GitMergeIcon,
                tooltip: t('todo.integration'),
                onClick: () => openIntegration(projectId),
              },
              ...mainWikiItem,
              ...rightItems,
            ]
          : [...mainWikiItem, ...rightItems]
      }
      syncErrors={syncErrors}
      mobileMode={mobileMode}
      mobileUserSlot={userPopoverSlot}
      isOnProjectPage={isOnProjectPage}
      isOnProjectSubRoute={isOnProjectSubRoute}
      onOpenCommandBar={handleOpenCommandBar}
      onOpenSettings={handleOpenSettings}
      onNavigateBack={handleNavigateBack}
      onNavigateToBoard={handleNavigateToBoard}
      onOpenDrawer={onOpenDrawer}
      mobileActiveTab={mobileActiveTab as MobileTabId}
      onMobileTabChange={handleMobileTabChange}
      mobileTabs={mobileWorkspaceTabs}
    />
  );
}
