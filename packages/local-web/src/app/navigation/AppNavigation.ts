import { router } from '@web/app/router';
import type { FileRouteTypes } from '@web/routeTree.gen';
import {
  type AppDestination,
  type AppNavigation,
  type NavigationTransition,
} from '@/shared/lib/routes/appNavigation';

type LocalRouteId = FileRouteTypes['id'];

function getPathParam(
  routeParams: Record<string, string>,
  key: string
): string | null {
  const value = routeParams[key];
  return value ? value : null;
}

function parseLocalHostIdFromPathname(pathname: string): string | null {
  const segments = pathname.split('/').filter(Boolean);
  const hostsIndex = segments.indexOf('hosts');
  if (hostsIndex === -1) {
    return null;
  }
  return segments[hostsIndex + 1] ?? null;
}

function resolveLocalDestinationFromPath(path: string): AppDestination | null {
  const { pathname } = new URL(path, 'http://localhost');
  const { foundRoute, routeParams } = router.getMatchedRoutes(pathname);

  if (!foundRoute) {
    return null;
  }

  switch (foundRoute.id as LocalRouteId) {
    case '/':
      return { kind: 'root' };
    case '/onboarding':
      return { kind: 'onboarding' };

    case '/_app/workspaces':
      return { kind: 'workspaces' };

    case '/_app/workspaces_/create':
      return { kind: 'workspaces-create' };

    case '/_app/workspaces_/$workspaceId_/workflow':
    case '/_app/workspaces_/$workspaceId': {
      const workspaceId = getPathParam(routeParams, 'workspaceId');
      return workspaceId ? { kind: 'workspace', workspaceId } : null;
    }

    case '/workspaces/$workspaceId/vscode': {
      const workspaceId = getPathParam(routeParams, 'workspaceId');
      return workspaceId ? { kind: 'workspace-vscode', workspaceId } : null;
    }

    case '/_app/projects/$projectId': {
      const projectId = getPathParam(routeParams, 'projectId');
      return projectId ? { kind: 'project', projectId } : null;
    }
    case '/_app/projects/$projectId_/workflows': {
      const projectId = getPathParam(routeParams, 'projectId');
      return projectId ? { kind: 'project-workflows', projectId } : null;
    }
    case '/_app/projects/$projectId_/workflows_/$workflowId/edit': {
      const projectId = getPathParam(routeParams, 'projectId');
      const workflowId = getPathParam(routeParams, 'workflowId');
      return projectId && workflowId
        ? { kind: 'project-workflow-edit', projectId, workflowId }
        : null;
    }
    case '/_app/projects/$projectId_/workflow-runs/$runId': {
      const projectId = getPathParam(routeParams, 'projectId');
      const runId = getPathParam(routeParams, 'runId');
      return projectId && runId
        ? { kind: 'project-workflow-run', projectId, runId }
        : null;
    }
    case '/_app/projects/$projectId_/issues/$issueId': {
      const projectId = getPathParam(routeParams, 'projectId');
      const issueId = getPathParam(routeParams, 'issueId');
      return projectId && issueId
        ? { kind: 'project-issue', projectId, issueId }
        : null;
    }
    case '/_app/projects/$projectId_/issues/$issueId_/workspaces/$workspaceId': {
      const projectId = getPathParam(routeParams, 'projectId');
      const issueId = getPathParam(routeParams, 'issueId');
      const workspaceId = getPathParam(routeParams, 'workspaceId');
      return projectId && issueId && workspaceId
        ? {
            kind: 'project-issue-workspace',
            projectId,
            issueId,
            workspaceId,
          }
        : null;
    }

    case '/_app/projects/$projectId_/issues/$issueId_/workspaces/create/$draftId': {
      const projectId = getPathParam(routeParams, 'projectId');
      const issueId = getPathParam(routeParams, 'issueId');
      const draftId = getPathParam(routeParams, 'draftId');
      return projectId && issueId && draftId
        ? {
            kind: 'project-issue-workspace-create',
            projectId,
            issueId,
            draftId,
          }
        : null;
    }

    case '/_app/projects/$projectId_/workspaces/create/$draftId': {
      const projectId = getPathParam(routeParams, 'projectId');
      const draftId = getPathParam(routeParams, 'draftId');
      return projectId && draftId
        ? {
            kind: 'project-workspace-create',
            projectId,
            draftId,
          }
        : null;
    }

    default:
      return null;
  }
}

function destinationToLocalTarget(
  destination: AppDestination,
  options: { currentHostId: string | null }
) {
  const destinationHostId =
    'hostId' in destination ? (destination.hostId ?? null) : null;
  const effectiveHostId = destinationHostId ?? options.currentHostId;

  if (effectiveHostId)
    throw new Error('Remote Access hosts are no longer supported');
  switch (destination.kind) {
    case 'root':
      return { to: '/' } as const;
    case 'onboarding':
      return { to: '/onboarding' } as const;

    case 'onboarding-sign-in':
    case 'workspaces':
      return { to: '/workspaces' } as const;
    case 'workspaces-create':
      return { to: '/workspaces/create' } as const;
    case 'workspace':
      return {
        to: '/workspaces/$workspaceId',
        params: { workspaceId: destination.workspaceId },
      } as const;
    case 'workspace-vscode':
      return {
        to: '/workspaces/$workspaceId/vscode',
        params: { workspaceId: destination.workspaceId },
      } as const;
    case 'export':
      throw new Error('Cloud export is no longer available');
    case 'project':
      return {
        to: '/projects/$projectId',
        params: { projectId: destination.projectId },
      } as const;
    case 'project-workflows':
      return {
        to: '/projects/$projectId/workflows',
        params: { projectId: destination.projectId },
      } as const;
    case 'project-workflow-edit':
      return {
        to: '/projects/$projectId/workflows/$workflowId/edit',
        params: {
          projectId: destination.projectId,
          workflowId: destination.workflowId,
        },
      } as const;
    case 'project-workflow-run':
      return {
        to: '/projects/$projectId/workflow-runs/$runId',
        params: {
          projectId: destination.projectId,
          runId: destination.runId,
        },
      } as const;
    case 'project-issue':
      return {
        to: '/projects/$projectId/issues/$issueId',
        params: {
          projectId: destination.projectId,
          issueId: destination.issueId,
        },
      } as const;
    case 'project-issue-workspace':
      return {
        to: '/projects/$projectId/issues/$issueId/workspaces/$workspaceId',
        params: {
          projectId: destination.projectId,
          issueId: destination.issueId,
          workspaceId: destination.workspaceId,
        },
      } as const;
    case 'project-issue-workspace-create':
      return {
        to: '/projects/$projectId/issues/$issueId/workspaces/create/$draftId',
        params: {
          projectId: destination.projectId,
          issueId: destination.issueId,
          draftId: destination.draftId,
        },
      } as const;
    case 'project-workspace-create':
      return {
        to: '/projects/$projectId/workspaces/create/$draftId',
        params: {
          projectId: destination.projectId,
          draftId: destination.draftId,
        },
      } as const;
  }
}

export function createLocalAppNavigation(): AppNavigation {
  const navigateTo = (
    destination: AppDestination,
    transition?: NavigationTransition
  ) => {
    const currentHostId =
      typeof window === 'undefined'
        ? null
        : parseLocalHostIdFromPathname(router.state.location.pathname);

    void router.navigate({
      ...destinationToLocalTarget(destination, { currentHostId }),
      ...(transition?.replace !== undefined
        ? { replace: transition.replace }
        : {}),
    });
  };

  const navigation: AppNavigation = {
    resolveFromPath: (path) => resolveLocalDestinationFromPath(path),
    goToRoot: (transition) => navigateTo({ kind: 'root' }, transition),
    goToOnboarding: (transition) =>
      navigateTo({ kind: 'onboarding' }, transition),
    goToOnboardingSignIn: (transition) =>
      navigateTo({ kind: 'onboarding-sign-in' }, transition),
    goToWorkspaces: (transition) =>
      navigateTo({ kind: 'workspaces' }, transition),
    goToWorkspacesCreate: (transition) =>
      navigateTo({ kind: 'workspaces-create' }, transition),
    goToWorkspace: (workspaceId, transition) =>
      navigateTo({ kind: 'workspace', workspaceId }, transition),
    goToWorkspaceVsCode: (workspaceId, transition) =>
      navigateTo({ kind: 'workspace-vscode', workspaceId }, transition),
    goToExport: (transition) => navigateTo({ kind: 'export' }, transition),
    goToProject: (projectId, transition) =>
      navigateTo({ kind: 'project', projectId }, transition),
    goToProjectWorkflows: (projectId, transition) =>
      navigateTo({ kind: 'project-workflows', projectId }, transition),
    goToProjectWorkflowEdit: (projectId, workflowId, transition) =>
      navigateTo(
        { kind: 'project-workflow-edit', projectId, workflowId },
        transition
      ),
    goToProjectWorkflowRun: (projectId, runId, transition) =>
      navigateTo(
        { kind: 'project-workflow-run', projectId, runId },
        transition
      ),
    goToProjectIssue: (projectId, issueId, transition) =>
      navigateTo({ kind: 'project-issue', projectId, issueId }, transition),
    goToProjectIssueWorkspace: (projectId, issueId, workspaceId, transition) =>
      navigateTo(
        { kind: 'project-issue-workspace', projectId, issueId, workspaceId },
        transition
      ),
    goToProjectIssueWorkspaceCreate: (
      projectId,
      issueId,
      draftId,
      transition
    ) =>
      navigateTo(
        { kind: 'project-issue-workspace-create', projectId, issueId, draftId },
        transition
      ),
    goToProjectWorkspaceCreate: (projectId, draftId, transition) =>
      navigateTo(
        { kind: 'project-workspace-create', projectId, draftId },
        transition
      ),
  };

  return navigation;
}

export const localAppNavigation = createLocalAppNavigation();
