import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { IntegrationRun } from 'shared/types';
import { IntegrationResolutionReview } from './IntegrationResolutionReview';
const fixture = vi.hoisted(() => ({
  decide: vi.fn(),
  approve: undefined as undefined | ((id: string) => Promise<unknown>),
}));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock('@tanstack/react-query', () => ({
  useQueryClient: () => ({ invalidateQueries: vi.fn() }),
  useMutation: ({
    mutationFn,
  }: {
    mutationFn: (id: string) => Promise<unknown>;
  }) => {
    fixture.approve = mutationFn;
    return { mutate: vi.fn(), isPending: false };
  },
}));
vi.mock('@/shared/lib/integrationApi', () => ({
  integrationApi: { decide: fixture.decide },
}));
vi.mock('@vibe/ui/components/PrimaryButton', () => ({
  PrimaryButton: ({ children, ...props }: { children: string }) =>
    createElement('button', props, children),
}));
const run = {
  id: 'run',
  project_id: 'project',
  status: 'blocked',
  payload: {
    result_commit: 'candidate',
    stop: { category: 'merge_review' },
    validation: [{ command: 'test', result: 'passed' }],
    reviews: [
      {
        id: 'review',
        candidate: 'candidate',
        step_index: 1,
        diff: '+ both features',
        explanation: 'Preserve A and B',
        decision: null,
      },
    ],
  },
} as unknown as IntegrationRun;
const render = (value = run) =>
  renderToStaticMarkup(
    createElement(IntegrationResolutionReview, { run: value })
  );
beforeEach(() => vi.clearAllMocks());
describe('merge resolution review', () => {
  it('shows the real diff, explanation, test evidence and revision instructions', () => {
    const html = render();
    expect(html).toContain('+ both features');
    expect(html).toContain('Preserve A and B');
    expect(html).toContain('passed');
    expect(html).toContain('integrationReview.approve');
    expect(html).toContain('integrationReview.rejectHelp');
  });
  it('sends explicit approval for the displayed review with stable retry identity', async () => {
    render();
    await fixture.approve!('review');
    await fixture.approve!('review');
    expect(fixture.decide.mock.calls[0]).toEqual(fixture.decide.mock.calls[1]);
    expect(fixture.decide.mock.calls[0]).toMatchObject([
      'run',
      { gate_id: 'review', project_id: 'project', decision: 'continue' },
    ]);
  });
  it('keeps historical evidence but hides approval when the candidate has changed', () => {
    const html = render({
      ...run,
      payload: { ...run.payload, result_commit: 'new candidate' },
    });
    expect(html).toContain('+ both features');
    expect(html).not.toContain('<button');
  });
});
