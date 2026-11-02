import { useQuery } from '@tanstack/react-query';
import { FileApiProvider } from '@/features/workspace-files/model/FileApiProvider';
import {
  WorkspaceFilePreviewPanel,
  type WorkspaceFilePreviewTarget,
} from '@/features/workspace-files';
import { WorkspaceWikiProvider } from '@/features/wiki/ui/WorkspaceWikiProvider';
import { WorkspaceWikiArticle } from '@/features/wiki/ui/WorkspaceWikiPanel';
import {
  projectUtilitiesApi,
  readUtility,
  type MainRepo,
} from '@/shared/lib/projectUtilitiesApi';
import { ProjectUtilities } from './ProjectUtilities';
import { useUiPreferencesStore } from '@/shared/stores/useUiPreferencesStore';
import { useTodoViewStore } from '@/features/todos/model/todoViewStore';
import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Group, type Layout, Panel, Separator } from 'react-resizable-panels';
import { KanbanContainer } from '@/features/kanban/ui/KanbanContainer';
import { useActions } from '@/shared/hooks/useActions';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { useAuth } from '@/shared/hooks/auth/useAuth';
import { useCurrentKanbanRouteState } from '@/shared/hooks/useCurrentKanbanRouteState';
import { useIsMobile } from '@/shared/hooks/useIsMobile';
import { useOrgContext } from '@/shared/hooks/useOrgContext';
import { useOrganizationProjects } from '@/shared/hooks/useOrganizationProjects';
import { usePageTitle } from '@/shared/hooks/usePageTitle';
import { useProjectContext } from '@/shared/hooks/useProjectContext';
import { useUserOrganizations } from '@/shared/hooks/useUserOrganizations';
import { ProjectProvider } from '@/shared/providers/remote/ProjectProvider';
import { OrgProvider } from '@/shared/providers/remote/OrgProvider';
import {
  buildKanbanIssueComposerKey,
  closeKanbanIssueComposer,
} from '@/shared/stores/useKanbanIssueComposerStore';
import { useOrganizationStore } from '@/shared/stores/useOrganizationStore';
import {
  PERSIST_KEYS,
  usePaneSize,
} from '@/shared/stores/useUiPreferencesStore';

import { ProjectRightSidebarContainer } from './ProjectRightSidebarContainer';

const KANBAN_DEFAULT_LEFT_PANEL_SIZE = 75;
const KANBAN_BOARD_MIN_PERCENT = 35;
const KANBAN_BOARD_MIN_SIZE = `${KANBAN_BOARD_MIN_PERCENT}%`;
const KANBAN_RIGHT_PANEL_MIN_SIZE = '25%';

function getClampedKanbanLeftPanelSize(size: number | string) {
  if (typeof size !== 'number' || !Number.isFinite(size)) {
    return KANBAN_DEFAULT_LEFT_PANEL_SIZE;
  }

  return Math.max(size, KANBAN_BOARD_MIN_PERCENT);
}

/**
 * Component that registers project mutations with ActionsContext.
 * Must be rendered inside both ActionsProvider and ProjectProvider.
 */
function ProjectMutationsRegistration({ children }: { children: ReactNode }) {
  const { i18n } = useTranslation('common');
  const { registerProjectMutations } = useActions();
  const { removeIssue, insertIssue, getIssue, getAssigneesForIssue, issues } =
    useProjectContext();

  // Use ref to always access latest issues and avoid stale closures.
  const issuesRef = useRef(issues);
  useEffect(() => {
    issuesRef.current = issues;
  }, [issues]);

  useEffect(() => {
    registerProjectMutations({
      removeIssue: (id) => {
        removeIssue(id);
      },
      duplicateIssue: (issueId) => {
        const issue = getIssue(issueId);
        if (!issue) return;

        const currentIssues = issuesRef.current;
        const statusIssues = currentIssues.filter(
          (candidate) => candidate.status_id === issue.status_id
        );
        const minSortOrder =
          statusIssues.length > 0
            ? Math.min(...statusIssues.map((candidate) => candidate.sort_order))
            : 0;

        insertIssue({
          project_id: issue.project_id,
          status_id: issue.status_id,
          title: i18n.t('common:kanban.copyTitle', { title: issue.title }),
          description: issue.description,
          priority: issue.priority,
          sort_order: minSortOrder - 1,
          start_date: issue.start_date,
          target_date: issue.target_date,
          completed_at: null,
          parent_issue_id: issue.parent_issue_id,
          parent_issue_sort_order: issue.parent_issue_sort_order,
          extension_metadata: issue.extension_metadata,
        });
      },
      getIssue,
      getAssigneesForIssue,
    });

    return () => {
      registerProjectMutations(null);
    };
  }, [
    registerProjectMutations,
    removeIssue,
    insertIssue,
    getIssue,
    getAssigneesForIssue,
    i18n,
  ]);

  return <>{children}</>;
}

function ProjectKanbanBoard() {
  return (
    <div className="h-full min-h-0 w-full">
      <KanbanContainer />
    </div>
  );
}

function ProjectKanbanLayout({ projectName }: { projectName: string }) {
  const { projectId } = useProjectContext();
  const { issueId } = useCurrentKanbanRouteState();
  const [detail, setDetail] = useState<'file' | 'wiki' | null>(null);
  useEffect(() => setDetail(null), [issueId]);
  const [file, setFile] = useState<WorkspaceFilePreviewTarget | null>(null);
  const visible = useUiPreferencesStore((s) => s.isRightSidebarVisible);
  const toggle = useUiPreferencesStore((s) => s.toggleRightSidebar);
  const info = useQuery({
    queryKey: ['project-main-utilities', projectId],
    queryFn: () =>
      readUtility<{ repos: MainRepo[] }>(
        `/api/projects/${projectId}/utilities/info`
      ),
    staleTime: 30000,
  });
  const api = useMemo(() => projectUtilitiesApi(projectId), [projectId]);
  const { t } = useTranslation('common');
  const close = () => setDetail(null);
  const body = detail ? (
    <div className="h-full flex flex-col">
      <button
        type="button"
        className="p-half border-b text-normal hover:bg-panel"
        onClick={close}
      >
        {t('buttons.close')}
      </button>
      <div className="flex-1 min-h-0">
        {detail === 'wiki' ? (
          <WorkspaceWikiArticle />
        ) : (
          <WorkspaceFilePreviewPanel target={file} onClearTarget={close} />
        )}
      </div>
    </div>
  ) : null;
  return (
    <FileApiProvider api={api}>
      <WorkspaceWikiProvider
        workspace={undefined}
        repos={[]}
        projectId={projectId}
        mainRepos={info.isError ? [] : info.data?.repos}
      >
        <div className="relative flex h-full min-w-0">
          <div className="flex-1 min-w-0 h-full">
            <ProjectKanbanPanes projectName={projectName} utilityBody={body} />
          </div>
          {visible && (
            <div className="absolute right-0 inset-y-0 z-30 w-[min(320px,100%)] xl:relative xl:w-80 shrink-0 shadow-lg xl:shadow-none">
              <ProjectUtilities
                projectId={projectId}
                repos={info.isError ? [] : (info.data?.repos ?? [])}
                error={info.isError}
                selectedFile={file}
                onClose={toggle}
                onFile={(target) => {
                  setFile(target);
                  setDetail('file');
                  if (window.matchMedia('(max-width: 1279px)').matches)
                    toggle();
                }}
                onWiki={() => {
                  setDetail('wiki');
                  if (window.matchMedia('(max-width: 1279px)').matches)
                    toggle();
                }}
              />
            </div>
          )}
        </div>
      </WorkspaceWikiProvider>
    </FileApiProvider>
  );
}

function ProjectKanbanPanes({
  projectName,
  utilityBody,
}: {
  projectName: string;
  utilityBody: ReactNode;
}) {
  const { issueId, isPanelOpen } = useCurrentKanbanRouteState();
  const isMobile = useIsMobile();
  const { getIssue, projectId } = useProjectContext();
  const todoVisible = useTodoViewStore(
    (s) => s.projectViews[projectId] === 'todos'
  );
  const issue = issueId ? getIssue(issueId) : undefined;
  usePageTitle(issue?.title, projectName);
  const [kanbanLeftPanelSize, setKanbanLeftPanelSize] = usePaneSize(
    PERSIST_KEYS.kanbanLeftPanel,
    KANBAN_DEFAULT_LEFT_PANEL_SIZE
  );

  const isRightPanelOpen =
    Boolean(utilityBody) || (isPanelOpen && !todoVisible);

  if (isMobile && utilityBody) {
    return (
      <div className="h-full flex flex-col">
        <div className="min-h-0 flex-1">
          <ProjectKanbanBoard />
        </div>
        <div className="min-h-0 flex-1 border-t">{utilityBody}</div>
      </div>
    );
  }
  if (isMobile) {
    return isRightPanelOpen ? (
      <div className="h-full w-full overflow-hidden bg-secondary">
        <ProjectRightSidebarContainer />
      </div>
    ) : (
      <div className="h-full w-full overflow-hidden bg-primary">
        <ProjectKanbanBoard />
      </div>
    );
  }

  const clampedKanbanLeftPanelSize =
    getClampedKanbanLeftPanelSize(kanbanLeftPanelSize);

  const kanbanDefaultLayout: Layout = {
    'kanban-left': clampedKanbanLeftPanelSize,
    'kanban-right': 100 - clampedKanbanLeftPanelSize,
  };

  const onKanbanLayoutChange = (layout: Layout) => {
    const nextKanbanLeftPanelSize = layout['kanban-left'];

    if (isRightPanelOpen) {
      setKanbanLeftPanelSize(
        getClampedKanbanLeftPanelSize(nextKanbanLeftPanelSize)
      );
    }
  };

  return (
    <Group
      orientation="horizontal"
      className="flex-1 min-w-0 h-full"
      defaultLayout={kanbanDefaultLayout}
      onLayoutChange={onKanbanLayoutChange}
    >
      <Panel
        id="kanban-left"
        minSize={KANBAN_BOARD_MIN_SIZE}
        className="min-w-0 h-full overflow-hidden bg-primary"
      >
        <ProjectKanbanBoard />
      </Panel>

      {isRightPanelOpen && (
        <Separator
          id="kanban-separator"
          className="w-1 bg-panel outline-none hover:bg-brand/50 transition-colors cursor-col-resize"
        />
      )}

      {isRightPanelOpen && (
        <Panel
          id="kanban-right"
          minSize={KANBAN_RIGHT_PANEL_MIN_SIZE}
          className="min-w-0 h-full overflow-hidden bg-secondary"
        >
          {utilityBody ?? <ProjectRightSidebarContainer />}
        </Panel>
      )}
    </Group>
  );
}

function ProjectKanbanInner({ projectId }: { projectId: string }) {
  const { t } = useTranslation('common');
  const { projects, isLoading } = useOrgContext();

  const project = projects.find((candidate) => candidate.id === projectId);

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full w-full">
        <p className="text-low">{t('states.loading')}</p>
      </div>
    );
  }

  if (!project) {
    return (
      <div className="flex items-center justify-center h-full w-full">
        <p className="text-low">{t('kanban.noProjectFound')}</p>
      </div>
    );
  }

  return (
    <ProjectProvider projectId={projectId}>
      <ProjectMutationsRegistration>
        <ProjectKanbanLayout key={project.id} projectName={project.name} />
      </ProjectMutationsRegistration>
    </ProjectProvider>
  );
}

/**
 * Hook to find a project by ID, using orgId from Zustand store
 */
function useFindProjectById(projectId: string | undefined) {
  const { isLoaded: authLoaded } = useAuth();
  const { data: orgsData, isLoading: orgsLoading } = useUserOrganizations();
  const selectedOrgId = useOrganizationStore((s) => s.selectedOrgId);
  const organizations = orgsData?.organizations ?? [];

  // Use stored org ID, or fall back to first org
  const orgIdToUse = selectedOrgId ?? organizations[0]?.id ?? null;

  const { data: projects = [], isLoading: projectsLoading } =
    useOrganizationProjects(orgIdToUse);

  const project = useMemo(() => {
    if (!projectId) return undefined;
    return projects.find((candidate) => candidate.id === projectId);
  }, [projectId, projects]);

  return {
    project,
    organizationId: project?.organization_id ?? selectedOrgId,
    // Include auth loading state - we can't determine project access until auth loads
    isLoading: !authLoaded || orgsLoading || projectsLoading,
  };
}

/**
 * ProjectKanban page - displays the Kanban board for a specific project
 *
 * URL patterns:
 * - /projects/:projectId - Kanban board with no issue selected
 * - /projects/:projectId/issues/:issueId - Kanban with issue panel open
 * - /projects/:projectId/issues/:issueId/workspaces/:workspaceId - Kanban with workspace session panel open
 * - /projects/:projectId/issues/:issueId/workspaces/create/:draftId - Kanban with workspace create panel
 *
 * Note: issue creation is composer-store state on top of /projects/:projectId.
 *
 * Note: This component is rendered inside SharedAppLayout which provides
 * NavbarContainer, AppBar, and SyncErrorProvider.
 */
export function ProjectKanban() {
  const { projectId, hostId, hasInvalidWorkspaceCreateDraftId } =
    useCurrentKanbanRouteState();
  const appNavigation = useAppNavigation();
  const { t } = useTranslation('common');
  const { isLoaded: authLoaded } = useAuth();
  const issueComposerKey = useMemo(() => {
    if (!projectId) {
      return null;
    }
    return buildKanbanIssueComposerKey(hostId, projectId);
  }, [hostId, projectId]);
  const previousIssueComposerKeyRef = useRef<string | null>(null);

  useEffect(() => {
    const previousKey = previousIssueComposerKeyRef.current;
    if (previousKey && previousKey !== issueComposerKey) {
      closeKanbanIssueComposer(previousKey);
    }

    previousIssueComposerKeyRef.current = issueComposerKey;
  }, [issueComposerKey]);

  // Redirect invalid workspace-create draft URLs back to the closed project view.
  useEffect(() => {
    if (!projectId) return;

    if (hasInvalidWorkspaceCreateDraftId) {
      appNavigation.goToProject(projectId, {
        replace: true,
      });
    }
  }, [projectId, hasInvalidWorkspaceCreateDraftId, appNavigation]);

  // Find the project and get its organization
  const { organizationId, isLoading } = useFindProjectById(
    projectId ?? undefined
  );

  // Show loading while auth state is being determined
  if (!authLoaded || isLoading) {
    return (
      <div className="flex items-center justify-center h-full w-full">
        <p className="text-low">{t('states.loading')}</p>
      </div>
    );
  }

  // If not signed in, prompt user to log in

  if (!projectId || !organizationId) {
    return (
      <div className="flex items-center justify-center h-full w-full">
        <p className="text-low">{t('kanban.noProjectFound')}</p>
      </div>
    );
  }

  return (
    <OrgProvider organizationId={organizationId}>
      <ProjectKanbanInner projectId={projectId} />
    </OrgProvider>
  );
}
