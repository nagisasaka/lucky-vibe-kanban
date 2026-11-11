import { describe, expect, it } from 'vitest';
import { AgentRunStatus, type WorkflowAttemptResponse } from 'shared/types';
import type { WorkspaceWithStats } from '@vibe/ui/components/IssueWorkspaceCard';
import i18n from '@/i18n/config';
import { buildTaskAttempts } from './taskAttempt';

const english = i18n.getFixedT('en', 'common');
function workspace(
  values: Partial<WorkspaceWithStats> = {}
): WorkspaceWithStats {
  return {
    id: 'remote-workspace-1',
    localWorkspaceId: 'workspace-1',
    name: 'Codex try',
    archived: false,
    updatedAt: '2026-05-14T01:00:00Z',
    prs: [],
    owner: null,
    isOwnedByCurrentUser: true,
    ...values,
  };
}
function workflow(
  values: Partial<WorkflowAttemptResponse> = {}
): WorkflowAttemptResponse {
  return {
    id: 'workflow-attempt-1',
    project_id: 'project-1',
    issue_id: 'issue-1',
    workflow_id: 'workflow-1',
    latest_run_id: 'run-1',
    workspace_id: 'workspace-2',
    name: 'Plan -> Implement',
    status: 'running',
    created_at: '2026-05-14T00:00:00Z',
    updated_at: '2026-05-14T02:00:00Z',
    ...values,
  };
}

describe('task attempt view model', () => {
  it('merges single-agent and workflow attempts newest first', () => {
    const attempts = buildTaskAttempts(
      {
        workspaceAttempts: [
          workspace({
            latestProcessStatus: AgentRunStatus.succeeded,
            filesChanged: 2,
            linesAdded: 10,
            linesRemoved: 1,
          }),
        ],
        workflowAttempts: [workflow()],
      },
      english
    );
    expect(attempts.map((attempt) => attempt.id)).toEqual([
      'workflow-attempt-1',
      'remote-workspace-1',
    ]);
    expect(attempts[0]).toMatchObject({
      kind: 'workflow',
      title: 'Plan -> Implement',
      statusLabel: 'Running',
      primaryActionLabel: 'Open results',
    });
    expect(attempts[1]).toMatchObject({
      kind: 'single_agent',
      title: 'Codex try',
      statusLabel: 'Completed',
      primaryActionLabel: 'Open session',
    });
  });

  it('does not duplicate workflow-backed workspaces as single-agent attempts', () => {
    const attempts = buildTaskAttempts(
      {
        workspaceAttempts: [
          workspace({
            id: 'workflow-workspace-1',
            localWorkspaceId: 'local-workflow-workspace-1',
            name: 'Workflow backing workspace',
            updatedAt: '2026-05-14T03:00:00Z',
            latestProcessStatus: AgentRunStatus.running,
          }),
          workspace({
            id: 'single-agent-workspace-1',
            localWorkspaceId: 'local-single-agent-workspace-1',
            name: 'Manual attempt',
            updatedAt: '2026-05-14T02:00:00Z',
            latestProcessStatus: AgentRunStatus.succeeded,
          }),
        ],
        workflowAttempts: [
          workflow({
            latest_run_id: null,
            workspace_id: 'workflow-workspace-1',
            name: 'Workflow attempt',
            status: 'draft',
            updated_at: '2026-05-14T04:00:00Z',
          }),
        ],
      },
      english
    );
    expect(attempts.map((attempt) => attempt.id)).toEqual([
      'workflow-attempt-1',
      'single-agent-workspace-1',
    ]);
  });

  it('projects canonical waiting, failure, and cancellation states', () => {
    const attempts = buildTaskAttempts(
      {
        workspaceAttempts: [
          workspace({
            id: 'waiting-workspace',
            name: 'Waiting agent',
            latestProcessStatus: AgentRunStatus.awaiting_approval,
            updatedAt: '2026-05-14T03:00:00Z',
          }),
          workspace({
            id: 'crashed-workspace',
            name: 'Crashed agent',
            latestProcessStatus: AgentRunStatus.crashed,
            updatedAt: '2026-05-14T02:00:00Z',
          }),
          workspace({
            id: 'cancelled-workspace',
            name: 'Cancelled agent',
            latestProcessStatus: AgentRunStatus.cancelled,
            updatedAt: '2026-05-14T01:00:00Z',
          }),
        ],
        workflowAttempts: [],
      },
      english
    );
    expect(
      attempts.map(({ statusLabel, statusTone }) => ({
        statusLabel,
        statusTone,
      }))
    ).toEqual([
      { statusLabel: 'Waiting for approval', statusTone: 'waiting' },
      { statusLabel: 'Failed', statusTone: 'failed' },
      { statusLabel: 'Canceled', statusTone: 'canceled' },
    ]);
  });
});

it.each([AgentRunStatus.pending, AgentRunStatus.starting])(
  'shows %s as preparing without claiming a concurrency wait',
  (status) => {
    const [attempt] = buildTaskAttempts(
      {
        workspaceAttempts: [
          workspace({
            id: 'reserved',
            localWorkspaceId: 'local',
            name: 'Agent',
            latestProcessStatus: status,
          }),
        ],
        workflowAttempts: [],
      },
      english
    );
    expect(attempt).toMatchObject({
      statusLabel: 'Preparing',
      statusTone: 'waiting',
    });
  }
);

it.each(['en', 'ja'])(
  'preserves names matching owned fallback copy and localizes only missing names in %s',
  (locale) => {
    const input = {
      workspaceAttempts: [
        workspace({ name: 'Single agent attempt' }),
        workspace({ id: 'raw-title', name: 'Raw {{count}} 原文' }),
      ],
      workflowAttempts: [workflow({ name: 'Workflow attempt' })],
    };
    const before = structuredClone(input);
    const t = i18n.getFixedT(locale, 'common');
    const rows = buildTaskAttempts(input, t);
    expect(rows.find((row) => row.id === 'workflow-attempt-1')).toMatchObject({
      title: 'Workflow attempt',
      workflowId: 'workflow-1',
      latestRunId: 'run-1',
    });
    expect(rows.find((row) => row.id === 'remote-workspace-1')).toMatchObject({
      title: 'Single agent attempt',
      localWorkspaceId: 'workspace-1',
    });
    expect(rows.find((row) => row.id === 'raw-title')?.title).toBe(
      'Raw {{count}} 原文'
    );
    expect(input).toEqual(before);
    const empty = buildTaskAttempts(
      {
        workspaceAttempts: [
          workspace({
            name: null,
            latestProcessStatus: AgentRunStatus.awaiting_input,
          }),
        ],
        workflowAttempts: [workflow({ name: '', status: 'cancelling' })],
      },
      t
    );
    expect(empty[0].title).toBe(
      locale === 'ja' ? 'ワークフロー試行' : 'Workflow attempt'
    );
    expect(empty[0].statusLabel).toBe(
      locale === 'ja' ? 'キャンセル中' : 'Cancelling'
    );
    expect(empty[1].title).toBe(
      locale === 'ja' ? '単一エージェント試行' : 'Single-agent attempt'
    );
    expect(empty[1].statusLabel).toBe(
      locale === 'ja' ? '入力待ち' : 'Waiting for input'
    );
  }
);
