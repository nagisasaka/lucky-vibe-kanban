import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { IntegrationRun } from 'shared/types';
import { IntegrationInterventionPanel } from './IntegrationInterventionPanel';
const fixture = vi.hoisted(() => ({
  run: undefined as IntegrationRun | undefined,
  prompt: '',
  send: undefined as (() => Promise<void>) | undefined,
  steer: vi.fn(),
  intervene: vi.fn(),
}));
vi.mock('react', async () => ({
  ...(await vi.importActual('react')),
  useState: () => [fixture.prompt, vi.fn()],
}));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock('@tanstack/react-query', () => ({
  useQuery: () => ({ data: fixture.run }),
  useQueryClient: () => ({ invalidateQueries: vi.fn() }),
  useMutation: ({ mutationFn }: { mutationFn: () => Promise<void> }) => {
    if (!fixture.send) fixture.send = mutationFn;
    return { mutate: vi.fn(), isPending: false };
  },
}));
vi.mock('@/shared/lib/integrationApi', () => ({
  integrationApi: { steer: fixture.steer, intervene: fixture.intervene },
}));
vi.mock('@/features/kanban/ui/IntegrationGateControls', () => ({
  IntegrationGateControls: () => null,
}));
vi.mock('@vibe/ui/components/PrimaryButton', () => ({
  PrimaryButton: ({ children, ...props }: { children: string }) =>
    createElement('button', props, children),
}));
function render() {
  return renderToStaticMarkup(
    createElement(IntegrationInterventionPanel, {
      integrationId: 'integration',
    })
  );
}
beforeEach(() => {
  vi.clearAllMocks();
  fixture.send = undefined;
  fixture.prompt = 'Fix the missing test environment';
  fixture.run = {
    id: 'integration',
    project_id: 'project',
    status: 'integrating',
    agent_run_id: 'agent',
    payload: { publication_intent: false },
  } as IntegrationRun;
});
describe('Integration instructions', () => {
  it('retains the unverified label after completion and reload', () => {
    fixture.run = JSON.parse(
      JSON.stringify({
        ...fixture.run!,
        status: 'succeeded',
        payload: {
          ...fixture.run!.payload,
          skip_validation: true,
          validation: [],
        },
      })
    );
    expect(render()).toContain('integrationReview.unverified');
    fixture.run!.payload.skip_validation = false;
    expect(render()).not.toContain('integrationReview.unverified');
  });
  it('uses the existing steer endpoint and preserves command identity on resend', async () => {
    render();
    await fixture.send!();
    await fixture.send!();
    expect(fixture.steer).toHaveBeenCalledTimes(2);
    expect(fixture.steer.mock.calls[0]).toEqual(fixture.steer.mock.calls[1]);
    expect(fixture.steer.mock.calls[0][0]).toBe('agent');
    expect(fixture.steer.mock.calls[0][1].content).toBe(fixture.prompt);
    expect(fixture.intervene).not.toHaveBeenCalled();
  });
  it.each(['agent', null])(
    'resumes the same blocked Integration with or without a Run (%s)',
    async (agent) => {
      fixture.run = { ...fixture.run!, status: 'blocked', agent_run_id: agent };
      render();
      await fixture.send!();
      await fixture.send!();
      expect(fixture.intervene.mock.calls[0]).toEqual(
        fixture.intervene.mock.calls[1]
      );
      expect(fixture.intervene.mock.calls[0][0]).toBe('integration');
      expect(fixture.intervene.mock.calls[0][1]).toMatchObject({
        project_id: 'project',
        prompt: fixture.prompt,
      });
      expect(fixture.steer).not.toHaveBeenCalled();
    }
  );
  it.each(['publishing', 'recovery_required'])(
    'offers no instruction dispatch during %s',
    (status) => {
      fixture.run = {
        ...fixture.run!,
        status,
        payload: { ...fixture.run!.payload, publication_intent: true },
      };
      expect(render()).toContain('disabled=""');
      expect(fixture.steer).not.toHaveBeenCalled();
      expect(fixture.intervene).not.toHaveBeenCalled();
    }
  );
  it('shows the unexecuted plan, its development evidence and grouped environment findings', () => {
    fixture.run = {
      ...fixture.run!,
      status: 'blocked',
      error: 'Missing tool\nInsufficient build space',
      payload: {
        ...fixture.run!.payload,
        stop: {
          stage: 'planning',
          category: 'validation_environment',
          next_action: 'Repair prerequisites together, then resume',
        },
        validation: [
          {
            command: 'make test',
            cwd: '.',
            required: true,
            evidence: 'Branch A final development verification',
            environment_requirements: 'make available; compiler missing',
            execution_process_id: null,
            exit_code: null,
            result: null,
          },
        ],
        exclusions: ['Replaced the retired development command'],
      },
    };
    const html = render();
    expect(html).toContain('make test');
    expect(html).toContain('Branch A final development verification');
    expect(html).toContain('compiler missing');
    expect(html).toContain('integrationGate.notRun');
    expect(html).toContain('Missing tool\nInsufficient build space');
    expect(html).toContain('Replaced the retired development command');
    expect(html).toContain('<details open=""');
  });
});

it('does not offer instructions that cannot change a frozen review candidate', () => {
  fixture.run = {
    ...fixture.run!,
    status: 'blocked',
    payload: { ...fixture.run!.payload, stop: { category: 'merge_review' } },
  } as IntegrationRun;
  expect(render()).not.toContain('<textarea');
  expect(render()).toContain('integrationGate.cancel');
});

it.each(['publishing', 'recovery_required', 'blocked'])(
  'shows uncertain publication and suppresses controls with intent (%s)',
  (status) => {
    fixture.run = {
      ...fixture.run!,
      status,
      payload: {
        ...fixture.run!.payload,
        published: false,
        publication_intent: true,
        result_commit: 'candidate',
        validation: [],
        stop: { category: 'merge_review' },
        reviews: [
          {
            id: 'review',
            candidate: 'candidate',
            step_index: 1,
            explanation: 'Preserve both changes',
            diff: '+ both',
            decision: null,
          },
        ],
      },
    } as IntegrationRun;
    const html = render();
    expect(html).toContain('integrationPanel.reconciling');
    expect(html).not.toContain('integrationReview.notPublished');
    expect(html).not.toContain('integrationReview.published');
    expect(html).not.toContain('integrationReview.approve');
    expect(html).not.toContain('integrationGate.cancel');
    expect(html).not.toContain('<textarea');
  }
);

it.each([
  [false, false, 'integrationReview.notPublished'],
  [true, false, 'integrationReview.published'],
  [true, true, 'integrationReview.published'],
] as const)(
  'shows publication with published=%s, intent=%s',
  (published, intent, expected) => {
    fixture.run!.payload.published = published;
    fixture.run!.payload.publication_intent = intent;
    const html = render();
    expect(html).toContain(expected);
    expect(html).not.toContain('integrationPanel.reconciling');
    expect(html).not.toContain(
      published
        ? 'integrationReview.notPublished'
        : 'integrationReview.published'
    );
  }
);
