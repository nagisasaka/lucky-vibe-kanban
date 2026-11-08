import { useQueryClient } from '@tanstack/react-query';
import { useWikiAvailability } from '@/shared/hooks/useWikiAvailability';
import type { MainRepo } from '@/shared/lib/projectUtilitiesApi';
import { readUtility } from '@/shared/lib/projectUtilitiesApi';
import { useTranslation } from 'react-i18next';
import {
  useContext,
  useEffect,
  useMemo,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from 'react';
import type { RepoWithTargetBranch, Workspace } from 'shared/types';
import { createHmrContext } from '@/shared/lib/hmrContext';
import { workspacesApi } from '@/shared/lib/api';
import {
  createWikiViewerController,
  type WikiViewerController,
} from '../model/wikiViewerController';
import {
  wikiRepositoryOptions,
  type WikiRepositoryOption,
} from '../model/wikiNavigation';

interface WikiContext {
  workspace: Workspace | undefined;
  projectId?: string;
  repositoryOptions: WikiRepositoryOption[];
  controller: WikiViewerController;
}

const Context = createHmrContext<WikiContext | null>(
  'WorkspaceWikiContext',
  null
);

export interface WorkspaceWikiProviderProps {
  workspace: Workspace | undefined;
  repos: RepoWithTargetBranch[];
  enabled?: boolean;
  projectId?: string;
  mainRepos?: MainRepo[];
  children: ReactNode;
}

export function WorkspaceWikiProvider(props: WorkspaceWikiProviderProps) {
  return (
    <WorkspaceWikiScope
      key={props.projectId ?? props.workspace?.id ?? 'no-workspace'}
      {...props}
    />
  );
}

function WorkspaceWikiScope({
  workspace,
  repos,
  enabled = true,
  children,
  projectId,
  mainRepos,
}: WorkspaceWikiProviderProps) {
  const { t } = useTranslation('common');
  const queryClient = useQueryClient();
  const enabledIds = useWikiAvailability(
    projectId || workspace?.workspace_kind === 'direct_folder'
      ? []
      : repos.map((repo) => repo.id)
  );
  const repositoryOptions = useMemo(
    () =>
      projectId
        ? (mainRepos ?? [])
            .filter((repo) => repo.wiki_enabled === true)
            .map((repo) => ({ id: repo.id, name: repo.name }))
        : workspace &&
            workspace.workspace_kind !== 'direct_folder' &&
            repos.length > 0
          ? wikiRepositoryOptions(
              workspace,
              repos.filter((repo) => enabledIds.includes(repo.id)),
              t('workspaceWiki.directFolder')
            )
          : [],
    [projectId, mainRepos, workspace, repos, enabledIds, t]
  );
  const workspaceId = projectId ?? workspace?.id ?? '';
  const initialRepoId = repositoryOptions[0]?.id ?? '';
  // Keyed by workspace, not repository ordering. A changed source cannot flash
  // old content, while a refreshed repository list preserves valid selection.
  const [controller] = useState(() =>
    createWikiViewerController(
      workspaceId,
      initialRepoId,
      projectId
        ? {
            snapshot: (_, repoId) =>
              readUtility(
                `/api/projects/${projectId}/utilities/wiki?repo_id=${encodeURIComponent(repoId)}`
              ),
          }
        : workspacesApi.wiki
    )
  );

  useEffect(() => {
    if (!projectId) return;
    // Existing settings polling also observes publication completion. Refresh
    // only the selected canonical Wiki, without adding another polling loop.
    return queryClient.getQueryCache().subscribe((event) => {
      if (
        event.type === 'updated' &&
        event.action.type === 'success' &&
        event.query.queryKey.at(-2) === 'repository-memory' &&
        event.query.queryKey.at(-1) === controller.getSnapshot().repoId
      )
        void controller.reload();
    });
  }, [controller, projectId, queryClient]);

  useEffect(() => {
    const current = controller.getSnapshot().repoId;
    if (!repositoryOptions.some((repo) => repo.id === current)) {
      controller.selectRepository(repositoryOptions[0]?.id ?? '');
    }
  }, [controller, repositoryOptions]);
  useEffect(() => {
    controller.setEnabled(enabled && Boolean(workspaceId));
    return () => controller.setEnabled(false);
  }, [controller, enabled, workspaceId]);
  const value = useMemo(
    () => ({ workspace, projectId, repositoryOptions, controller }),
    [workspace, projectId, repositoryOptions, controller]
  );
  return <Context.Provider value={value}>{children}</Context.Provider>;
}

export function useWorkspaceWiki() {
  const context = useContext(Context);
  if (!context) throw new Error('Wiki surfaces require WorkspaceWikiProvider');
  const state = useSyncExternalStore(
    context.controller.subscribe,
    context.controller.getSnapshot,
    context.controller.getSnapshot
  );
  return { ...context, ...state };
}
