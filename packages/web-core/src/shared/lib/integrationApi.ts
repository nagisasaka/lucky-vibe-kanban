import type {
  CreateIntegrationRequest,
  SteerAgentRunRequest,
  RunState,
  IntegrationRun,
  IntegrationSelection,
  ParallelContextPage,
  Repo,
  WorkspaceActivity,
} from 'shared/types';
import { handleApiResponse } from '@/shared/lib/api';
import { makeLocalApiRequest } from '@/shared/lib/localApiTransport';

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  return handleApiResponse<T>(
    await makeLocalApiRequest(path, {
      ...init,
      headers: { 'Content-Type': 'application/json', ...init?.headers },
    })
  );
}

export const integrationApi = {
  steer: (agent: string, body: SteerAgentRunRequest) =>
    request<RunState>(`/api/agent-runs/${agent}/steer`, {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  get: (id: string) => request<IntegrationRun>(`/api/integrations/${id}`),
  repositories: (projectId: string) =>
    request<Repo[]>(
      `/api/integrations/repositories?project_id=${encodeURIComponent(projectId)}`
    ),
  list: (projectId: string) =>
    request<IntegrationRun[]>(
      `/api/integrations?project_id=${encodeURIComponent(projectId)}`
    ),
  create: (body: CreateIntegrationRequest) =>
    request<IntegrationRun>('/api/integrations', {
      method: 'POST',
      body: JSON.stringify(body),
    }),
  cancel: (id: string) =>
    request<IntegrationRun>(`/api/integrations/${id}/cancel`, {
      method: 'POST',
    }),
  recover: (id: string) =>
    request<IntegrationRun>(`/api/integrations/${id}/recover`, {
      method: 'POST',
    }),
  activities: async (repositoryId: string): Promise<WorkspaceActivity[]> => {
    const activities: WorkspaceActivity[] = [];
    let after: string | null = null;
    do {
      const page: ParallelContextPage = await request(
        `/api/repos/${repositoryId}/parallel-context?limit=100${after ? `&after_workspace=${after}` : ''}`
      );
      activities.push(...page.activities);
      after = page.next_workspace;
    } while (after);
    return activities;
  },
};

export function integrationCanCancel(
  status: string,
  publicationIntent = false
): boolean {
  return (
    !publicationIntent &&
    ['queued', 'integrating', 'validating', 'blocked'].includes(status)
  );
}

export function integrationDoneLabel(
  status: string,
  published: boolean,
  doneResult: string | null,
  t: (key: string) => string
): string {
  if (doneResult === 'done') return t('common:integrationPanel.doneApplied');
  if (doneResult === 'not changed: Card reserved by another Auto-merge')
    return t('common:integrationPanel.doneReserved');
  if (doneResult === 'not changed: Card or destination column is unavailable')
    return t('common:integrationPanel.doneUnavailable');
  if (doneResult) return doneResult;
  if (!published && ['cancelled', 'blocked', 'failed'].includes(status))
    return t('common:integrationPanel.doneNotApplied');
  if (status === 'recovery_required')
    return t('common:integrationPanel.doneReconciling');
  return t('common:integrationPanel.donePending');
}

export function integrationStatusLabel(
  status: string,
  t: (key: string) => string
): string {
  const labels: Record<string, string> = {
    queued: t('common:integrationPanel.status.queued'),
    integrating: t('common:integrationPanel.status.integrating'),
    blocked: t('common:integrationPanel.status.blocked'),
    validating: t('common:integrationPanel.status.validating'),
    publishing: t('common:integrationPanel.status.publishing'),
    post_processing: t('common:integrationPanel.status.post_processing'),
    succeeded: t('common:integrationPanel.status.succeeded'),
    cancelled: t('common:integrationPanel.status.cancelled'),
    cancelling: t('common:integrationPanel.status.cancelling'),
    failed: t('common:integrationPanel.status.failed'),
    recovery_required: t('common:integrationPanel.status.recovery_required'),
  };
  return Object.hasOwn(labels, status)
    ? labels[status]
    : t('common:integrationPanel.unknownStatus');
}

export function integrationWikiLabel(
  result: string | null,
  t: (key: string) => string
): string {
  if (!result) return t('common:integrationPanel.notScheduled');
  if (result === 'disabled') return t('common:integrationPanel.wikiDisabled');
  if (result === 'Wiki included in published candidate')
    return t('common:integrationPanel.wikiIncluded');
  if (
    result.startsWith('Wiki receipt pending:') ||
    result.startsWith('semantic outbox pending:')
  )
    return t('common:integrationPanel.wikiPending');
  return t('common:integrationPanel.wikiResultRecorded');
}

export function integrationSelectionIsCurrent(
  selected: IntegrationSelection[],
  activities: WorkspaceActivity[],
  targetBranch?: string
): boolean {
  return (
    selected.length > 0 &&
    selected.every(
      (s) =>
        !!s.expected_commit &&
        activities.some(
          (a) =>
            (!targetBranch || a.target_branch === targetBranch) &&
            a.current_issue_id === s.card_id &&
            a.workspace_id === s.workspace_id &&
            a.observed_head_oid === s.expected_commit
        )
    )
  );
}
