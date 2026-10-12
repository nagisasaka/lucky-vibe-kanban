import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import type { IntegrationRun } from 'shared/types';
import { IntegrationGateControls } from './IntegrationGateControls';
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock('@tanstack/react-query', () => ({
  useQueryClient: () => ({ invalidateQueries: vi.fn() }),
  useMutation: () => ({ mutate: vi.fn(), isPending: false }),
}));
vi.mock('@vibe/ui/components/Button', () => ({
  Button: ({ children, ...props }: { children: string }) =>
    createElement('button', props, children),
}));
vi.mock('@/shared/lib/integrationApi', () => ({ integrationApi: {} }));
const run = {
  id: 'integration',
  project_id: 'project',
  status: 'blocked',
  target_ref: 'refs/heads/main',
  payload: {
    result_commit: 'candidate',
    gates: [
      {
        id: 'gate',
        scope: JSON.stringify({
          candidate: 'candidate',
          target: 'refs/heads/main',
        }),
        risk: 'Unverified regression risk',
        evidence: { command: 'npm test', cwd: '.', result: 'failed' },
        decision: null,
      },
    ],
  },
} as unknown as IntegrationRun;
describe('candidate-bound confirmation', () => {
  it('displays failed evidence and risk and requires an acknowledgement before continuing', () => {
    const html = renderToStaticMarkup(
      createElement(IntegrationGateControls, { run })
    );
    expect(html).toContain('npm test');
    expect(html).toContain('failed');
    expect(html).toContain('candidate');
    expect(html).toContain('Unverified regression risk');
    expect(html).toMatch(/<button disabled="">integrationGate.continue/);
    expect(html).toContain('integrationGate.retry');
    expect(html).toContain('integrationGate.cancel');
  });
  it('restores approved evidence without displaying a successful test or another approval button', () => {
    const approved = structuredClone(run);
    approved.payload.gates[0].decision = 'continue';
    const html = renderToStaticMarkup(
      createElement(IntegrationGateControls, { run: approved })
    );
    expect(html).toContain('failed');
    expect(html).toContain('integrationGate.excepted');
    expect(html).not.toContain('integrationGate.continue');
    expect(html).not.toContain('checkbox');
  });
  it('never offers approval while the Integration is executing', () => {
    const html = renderToStaticMarkup(
      createElement(IntegrationGateControls, {
        run: { ...run, status: 'integrating' },
      })
    );
    expect(html).not.toContain('checkbox');
    expect(html).not.toContain('<button');
  });
  it('keeps historical approval attached to its original candidate', () => {
    const changed = structuredClone(run);
    changed.payload.result_commit = 'new-candidate';
    changed.payload.gates[0].decision = 'continue';
    const html = renderToStaticMarkup(
      createElement(IntegrationGateControls, { run: changed })
    );
    expect(html).toContain('candidate · refs/heads/main');
    expect(html).not.toContain('new-candidate');
  });
});
