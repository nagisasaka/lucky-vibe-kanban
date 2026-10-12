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
  it('offers no instruction dispatch during publication', () => {
    fixture.run = {
      ...fixture.run!,
      status: 'publishing',
      payload: { ...fixture.run!.payload, publication_intent: true },
    };
    expect(render()).toContain('disabled=""');
    expect(fixture.steer).not.toHaveBeenCalled();
    expect(fixture.intervene).not.toHaveBeenCalled();
  });
});
