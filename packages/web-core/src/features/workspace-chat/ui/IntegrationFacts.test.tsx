import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { beforeEach, expect, it, vi } from 'vitest';
import type { IntegrationRun, Workspace } from 'shared/types';
import { IntegrationFacts } from './IntegrationFacts';
const fixture = vi.hoisted(() => ({
  run: undefined as IntegrationRun | undefined,
}));
vi.mock('@tanstack/react-query', () => ({
  useQuery: () => ({ data: fixture.run }),
}));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock('@/shared/lib/integrationApi', () => ({
  integrationApi: { get: vi.fn() },
}));
const workspace = {
  execution_owner: { kind: 'integration', run_id: 'job' },
} as Workspace;
const render = () =>
  renderToStaticMarkup(createElement(IntegrationFacts, { workspace }));
beforeEach(() => {
  fixture.run = {
    target_ref: 'refs/heads/main',
    payload: {
      conversational: true,
      result_commit: 'R2',
      evidence: [
        {
          request_id: 'failed',
          code_commit: 'R1',
          result: 'failed',
          actor: 'host_execution',
          reason: 'test failure',
          remaining_risk: 'old candidate',
          command: 'test',
        },
        {
          request_id: 'skip',
          code_commit: 'R2',
          result: 'skipped',
          actor: 'agent',
          reason: 'tested source adopted',
          remaining_risk: 'no fresh environment test',
          command: null,
        },
      ],
      published: false,
      publication_intent: false,
    },
  } as IntegrationRun;
});
it('keeps failures and reasoned skips with their code SHA without claiming a pass or publication', () => {
  const html = render();
  expect(html).toContain('R1');
  expect(html).toContain('R2');
  expect(html).toContain('integrationFacts.failed');
  expect(html).toContain('integrationFacts.skipped');
  expect(html).toContain('no fresh environment test');
  expect(html).not.toContain('integrationFacts.passed');
  expect(html).not.toContain('integrationFacts.published');
});
it('uses the host publication receipt, distinguishing intent and Wiki commit', () => {
  fixture.run!.payload.publication_intent = true;
  expect(render()).toContain('integrationFacts.reconciling');
  fixture.run!.payload.published = true;
  fixture.run!.payload.publication_request = {
    wiki_commit: 'WIKI',
    code_commit: 'R2',
    candidate_commit: 'WIKI',
  } as IntegrationRun['payload']['publication_request'];
  const html = render();
  expect(html).toContain('integrationFacts.published');
  expect(html).toContain('font-bold text-success');
  expect(html).toContain('WIKI');
  expect(html).not.toContain('integrationFacts.reconciling');
});
it('does not reinterpret retained legacy execution history', () => {
  fixture.run!.payload.conversational = false;
  expect(render()).toBe('');
});

it('shows pending writer settlement without treating request acceptance as publication', () => {
  fixture.run!.payload.publication_request = {
    candidate_commit: 'R2',
    code_commit: 'R2',
  } as IntegrationRun['payload']['publication_request'];
  expect(render()).toContain('integrationFacts.pendingSettlement');
  expect(render()).not.toContain('integrationFacts.published');
});
