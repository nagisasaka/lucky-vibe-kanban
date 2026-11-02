import type { TFunction } from 'i18next';
import type { WorkflowAttemptResponse } from 'shared/types';
import type { WorkspaceWithStats } from '@vibe/ui/components/IssueWorkspaceCard';

export type TaskAttemptKind = 'single_agent' | 'workflow';
export type TaskAttemptStatusTone =
  | 'draft'
  | 'running'
  | 'waiting'
  | 'succeeded'
  | 'failed'
  | 'canceled'
  | 'neutral';

export interface TaskAttemptView {
  id: string;
  kind: TaskAttemptKind;
  title: string;
  subtitle: string;
  statusLabel: string;
  statusTone: TaskAttemptStatusTone;
  updatedAt: string;
  primaryActionLabel: string;
  localWorkspaceId?: string | null;
  workflowId?: string;
  workflowAttemptId?: string;
  latestRunId?: string | null;
  workspaceId?: string | null;
  filesChanged?: number;
  linesAdded?: number;
  linesRemoved?: number;
  isOwnedByCurrentUser?: boolean;
}

export interface BuildTaskAttemptsInput {
  workspaceAttempts: WorkspaceWithStats[];
  workflowAttempts: WorkflowAttemptResponse[];
}

export function buildTaskAttempts(
  { workspaceAttempts, workflowAttempts }: BuildTaskAttemptsInput,
  t: TFunction<'common'>
): TaskAttemptView[] {
  const workflowWorkspaceIds = getWorkflowAttemptWorkspaceIds(workflowAttempts);
  const singleAgentAttempts = workspaceAttempts.filter(
    (workspace) => !workflowWorkspaceIds.has(workspace.id)
  );

  return [
    ...workflowAttempts.map((attempt) =>
      workflowAttemptToTaskAttempt(attempt, t)
    ),
    ...singleAgentAttempts.map((workspace) =>
      workspaceToTaskAttempt(workspace, t)
    ),
  ].sort(
    (left, right) =>
      new Date(right.updatedAt).getTime() - new Date(left.updatedAt).getTime()
  );
}

export function getWorkflowAttemptWorkspaceIds(
  workflowAttempts: WorkflowAttemptResponse[]
): Set<string> {
  return new Set(
    workflowAttempts
      .map((attempt) => attempt.workspace_id)
      .filter((workspaceId): workspaceId is string => !!workspaceId)
  );
}

export function workflowAttemptToTaskAttempt(
  attempt: WorkflowAttemptResponse,
  t: TFunction<'common'>
): TaskAttemptView {
  return {
    id: attempt.id,
    kind: 'workflow',
    title: attempt.name || t('attempts.workflowFallbackTitle'),
    subtitle: attempt.latest_run_id
      ? t('attempts.workflowRun', { id: attempt.latest_run_id.slice(0, 8) })
      : t('attempts.draftWorkflow'),
    statusLabel: workflowAttemptStatusLabel(attempt.status, t),
    statusTone: workflowAttemptStatusTone(attempt.status),
    updatedAt: attempt.updated_at,
    primaryActionLabel: t('attempts.openCanvas'),
    workflowId: attempt.workflow_id,
    workflowAttemptId: attempt.id,
    latestRunId: attempt.latest_run_id,
    workspaceId: attempt.workspace_id,
  };
}

function workspaceToTaskAttempt(
  workspace: WorkspaceWithStats,
  t: TFunction<'common'>
): TaskAttemptView {
  return {
    id: workspace.id,
    kind: 'single_agent',
    title: workspace.name || t('attempts.singleAgentFallbackTitle'),
    subtitle: workspace.localWorkspaceId
      ? t('attempts.workspace', { id: workspace.localWorkspaceId.slice(0, 8) })
      : t('attempts.remoteWorkspace'),
    statusLabel: workspaceStatusLabel(workspace, t),
    statusTone: workspaceStatusTone(workspace),
    updatedAt: workspace.latestProcessCompletedAt ?? workspace.updatedAt,
    primaryActionLabel: t('attempts.openSession'),
    localWorkspaceId: workspace.localWorkspaceId,
    filesChanged: workspace.filesChanged,
    linesAdded: workspace.linesAdded,
    linesRemoved: workspace.linesRemoved,
    isOwnedByCurrentUser: workspace.isOwnedByCurrentUser,
  };
}

export function workflowAttemptStatusLabel(
  status: WorkflowAttemptResponse['status'],
  t: TFunction<'common'>
): string {
  switch (status) {
    case 'draft':
      return t('attempts.status.draft');
    case 'ready':
      return t('attempts.status.ready');
    case 'running':
      return t('attempts.status.running');
    case 'awaiting_human':
      return t('attempts.status.awaitingHuman');
    case 'awaiting_arena':
      return t('attempts.status.awaitingArena');
    case 'cancelling':
      return t('attempts.status.cancelling');
    case 'succeeded':
      return t('attempts.status.succeeded');
    case 'failed':
      return t('attempts.status.failed');
    case 'canceled':
      return t('attempts.status.canceled');
  }
}

export function workflowAttemptStatusTone(
  status: WorkflowAttemptResponse['status']
): TaskAttemptStatusTone {
  switch (status) {
    case 'draft':
      return 'draft';
    case 'ready':
      return 'neutral';
    case 'running':
      return 'running';
    case 'awaiting_human':
    case 'awaiting_arena':
    case 'cancelling':
      return 'waiting';
    case 'succeeded':
      return 'succeeded';
    case 'failed':
      return 'failed';
    case 'canceled':
      return 'canceled';
  }
}

function workspaceStatusLabel(
  workspace: WorkspaceWithStats,
  t: TFunction<'common'>
): string {
  if (workspace.archived) return t('attempts.status.archived');
  switch (workspace.latestProcessStatus) {
    case 'pending':
    case 'starting':
      return t('attempts.status.preparing');
    case 'running':
      return t('attempts.status.running');
    case 'awaiting_input':
      return t('attempts.status.awaitingInput');
    case 'awaiting_approval':
      return t('attempts.status.awaitingApproval');
    case 'cancelling':
      return t('attempts.status.cancelling');
    case 'succeeded':
      return t('attempts.status.completed');
    case 'failed':
    case 'crashed':
    case 'audit_failed':
      return t('attempts.status.failed');
    case 'cancelled':
      return t('attempts.status.canceled');
    default:
      return t('attempts.status.active');
  }
}

function workspaceStatusTone(
  workspace: WorkspaceWithStats
): TaskAttemptStatusTone {
  if (workspace.archived) return 'canceled';
  switch (workspace.latestProcessStatus) {
    case 'pending':
    case 'starting':
      return 'waiting';
    case 'running':
      return 'running';
    case 'awaiting_input':
    case 'awaiting_approval':
    case 'cancelling':
      return 'waiting';
    case 'succeeded':
      return 'succeeded';
    case 'failed':
    case 'crashed':
    case 'audit_failed':
      return 'failed';
    case 'cancelled':
      return 'canceled';
    default:
      return 'neutral';
  }
}
