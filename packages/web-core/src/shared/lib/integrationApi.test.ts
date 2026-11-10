import i18n from '@/i18n/config';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { IntegrationSelection, WorkspaceActivity } from 'shared/types';

const { transport } = vi.hoisted(() => ({ transport: vi.fn() }));
vi.mock('@/shared/lib/localApiTransport', () => ({
  makeLocalApiRequest: transport,
}));
vi.mock('@/shared/lib/api', () => ({
  handleApiResponse: async (response: Response) => (await response.json()).data,
}));
import {
  integrationApi,
  integrationCanCancel,
  integrationCanRecover,
  integrationStatusLabel,
  integrationWikiLabel,
  integrationDoneLabel,
  integrationSelectionIsCurrent,
} from './integrationApi';

const doneLabel = (status: string, published: boolean, result: string | null) =>
  integrationDoneLabel(
    status,
    published,
    result,
    i18n.getFixedT('en', 'common')
  );

const selection: IntegrationSelection = {
  card_id: 'card-a',
  workspace_id: 'workspace-a',
  expected_commit: 'a'.repeat(40),
};
const activity = {
  current_issue_id: 'card-a',
  workspace_id: 'workspace-a',
  observed_head_oid: 'a'.repeat(40),
  active_agent_runs: 0,
  active_scripts: 0,
} as unknown as WorkspaceActivity;

describe('formal Integration selection and controls', () => {
  it('does not promise a later Done update for cancelled or held unpublished work', () => {
    for (const status of ['cancelled', 'blocked', 'failed'])
      expect(doneLabel(status, false, null)).toBe('Done not applied');
    expect(doneLabel('integrating', false, null)).toBe('Done pending');
    expect(doneLabel('post_processing', true, null)).toBe('Done pending');
    expect(doneLabel('recovery_required', false, null)).toBe(
      'Done awaiting reconciliation'
    );
    expect(doneLabel('succeeded', true, 'applied')).toBe('applied');
    expect(doneLabel('post_processing', true, 'conditions changed')).toBe(
      'conditions changed'
    );
  });
  it('keeps adopted HEAD fixed when polling observes a newer commit or relink', () => {
    expect(integrationSelectionIsCurrent([selection], [activity])).toBe(true);
    for (const changed of [
      { observed_head_oid: 'b'.repeat(40) },
      { current_issue_id: 'card-b' },
      { workspace_id: 'workspace-b' },
    ]) {
      expect(
        integrationSelectionIsCurrent(
          [selection],
          [{ ...activity, ...changed } as WorkspaceActivity]
        )
      ).toBe(false);
    }
    expect(
      integrationSelectionIsCurrent(
        [selection],
        [{ ...activity, active_agent_runs: 1n, active_scripts: 1n }]
      )
    ).toBe(true);
    expect(selection.expected_commit).toBe('a'.repeat(40));
    expect(integrationSelectionIsCurrent([], [activity])).toBe(false);
    expect(
      integrationSelectionIsCurrent(
        [{ ...selection, expected_commit: '' }],
        [activity]
      )
    ).toBe(false);
    expect(integrationSelectionIsCurrent([selection], [])).toBe(false);
  });
  it('limits Auto-merge to the Workspace recorded merge target', () => {
    const recorded = { ...activity, target_branch: 'stable' };
    expect(
      integrationSelectionIsCurrent([selection], [recorded], 'stable')
    ).toBe(true);
    expect(integrationSelectionIsCurrent([selection], [recorded], 'main')).toBe(
      false
    );
  });
  it('never exposes cancel as Git undo during or after publication', () => {
    for (const state of ['queued', 'integrating', 'validating', 'blocked'])
      expect(integrationCanCancel(state)).toBe(true);
    for (const state of [
      'publishing',
      'post_processing',
      'succeeded',
      'failed',
      'cancelled',
      'recovery_required',
      'cancelling',
    ])
      expect(integrationCanCancel(state)).toBe(false);
  });
  it('does not cancel a blocked publication reconciliation', () => {
    expect(integrationCanCancel('blocked', true)).toBe(false);
  });
  it('offers receipt recovery for the exact backend prefix and eligible state', () => {
    expect(
      integrationCanRecover('succeeded', true, 'Wiki receipt pending: retry')
    ).toBe(true);
    expect(
      integrationCanRecover('succeeded', false, 'Wiki receipt pending: retry')
    ).toBe(false);
    expect(
      integrationCanRecover('blocked', true, 'Wiki receipt pending: retry')
    ).toBe(false);
    expect(
      integrationCanRecover(
        'succeeded',
        true,
        'Wiki included in published candidate'
      )
    ).toBe(false);
    expect(integrationCanRecover('post_processing', true, null)).toBe(true);
    expect(integrationCanRecover('recovery_required', false, null)).toBe(true);
  });
  it('describes published work and Done held for source changes separately', () => {
    for (const reason of [
      'source HEAD changed',
      'source Run active',
      'source HEAD changed and Run active',
    ]) {
      expect(doneLabel('succeeded', true, `not changed: ${reason}`)).toContain(
        'Published; Done held'
      );
    }
  });
  beforeEach(() => transport.mockReset());
  it('follows activity pagination without selecting or starting anything', async () => {
    transport
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            data: { activities: [activity], next_workspace: 'next-id' },
          })
        )
      )
      .mockResolvedValueOnce(
        new Response(
          JSON.stringify({
            data: {
              activities: [{ ...activity, workspace_id: 'workspace-b' }],
              next_workspace: null,
            },
          })
        )
      );
    const result = await integrationApi.activities('repo-id');
    expect(result.map((a) => a.workspace_id)).toEqual([
      'workspace-a',
      'workspace-b',
    ]);
    expect(transport.mock.calls.map(([path]) => path)).toEqual([
      '/api/repos/repo-id/parallel-context?limit=100',
      '/api/repos/repo-id/parallel-context?limit=100&after_workspace=next-id',
    ]);
    expect(transport.mock.calls.every(([, init]) => !init.method)).toBe(true);
  });
  it('retries the exact explicit request identity and source OID', async () => {
    transport.mockImplementation(
      async () => new Response(JSON.stringify({ data: { id: 'same-run' } }))
    );
    const request = {
      request_id: 'stable-request',
      project_id: 'project',
      repository_id: 'repo',
      target_branch: 'test/integration',
      selections: [selection],
      executor_config: null,
    };
    await integrationApi.create(request);
    await integrationApi.create(request);
    expect(transport.mock.calls[0][1].body).toBe(
      transport.mock.calls[1][1].body
    );
    expect(JSON.parse(transport.mock.calls[0][1].body).selections).toEqual([
      selection,
    ]);
  });
});

it('localizes owned integration receipts and statuses without displaying arbitrary receipt bodies', () => {
  for (const language of ['en', 'ja']) {
    const t = i18n.getFixedT(language, 'common');
    expect(integrationDoneLabel('succeeded', true, 'done', t)).toBe(
      language === 'ja' ? 'Doneを反映済み' : 'Done applied'
    );
    expect(integrationStatusLabel('queued', t)).toBe(
      language === 'ja' ? '待機中' : 'Queued'
    );
    expect(integrationStatusLabel('constructor', t)).toBe(
      integrationStatusLabel('future-status', t)
    );
    expect(
      integrationWikiLabel('Wiki receipt pending: Raw secret /internal/path', t)
    ).toBe(language === 'ja' ? '記録を確認中' : 'receipt pending');
    expect(integrationWikiLabel('Raw future receipt', t)).not.toContain(
      'Raw future receipt'
    );
    expect(integrationWikiLabel('disabled', t)).toBe(
      language === 'ja' ? '無効' : 'disabled'
    );
  }
});
