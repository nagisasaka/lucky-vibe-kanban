import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Repo } from 'shared/types';
import type { MachineClient } from '@/shared/lib/machineClient';
import { RepositoryPushSettings } from './RepositoryPushSettings';

vi.mock('react-i18next', async (importOriginal) => ({
  ...(await importOriginal<typeof import('react-i18next')>()),
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock('@tanstack/react-query', () => ({
  useQuery: ({ queryKey }: { queryKey: string[] }) => ({
    data: queryKey.includes('push-branches')
      ? [
          { name: 'main', is_remote: false },
          { name: 'dev', is_remote: false },
          { name: 'origin/secret', is_remote: true },
        ]
      : [{ name: 'origin', url: 'https://github.com/example/repo.git' }],
  }),
}));
vi.mock('@vibe/ui/components/PrimaryButton', () => ({
  PrimaryButton: ({ value, disabled }: { value: string; disabled: boolean }) =>
    createElement('button', { disabled }, value),
}));

const client = {
  target: { id: 'local' },
  queryScopeKey: ['machine', 'local'],
  pushRepository: vi.fn(),
  previewRepositoryPush: vi.fn(),
  verifyRepositoryPush: vi.fn(),
} as unknown as MachineClient;
function render() {
  return renderToStaticMarkup(
    createElement(RepositoryPushSettings, {
      repo: {
        id: 'repo-1',
        display_name: 'Example',
        path: '/fixture/repo',
      } as Repo,
      client,
      projectId: 'project-1',
      projects: [],
    })
  );
}
afterEach(() => {
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});
describe('Repository push explicit publication boundary', () => {
  it('renders local main and dev without starting preview or publication', () => {
    const html = render();
    expect(html).toContain('value="main"');
    expect(html).toContain('value="dev"');
    expect(html).not.toContain('origin/secret');
    expect(html).toContain('/fixture/repo');
    expect(client.pushRepository).not.toHaveBeenCalled();
    expect(client.previewRepositoryPush).not.toHaveBeenCalled();
  });
  it('retains a lost response request and exposes verification while locking new publication', () => {
    vi.stubGlobal('sessionStorage', { getItem: () => 'request-before-reload' });
    const html = render();
    expect(html).toContain('request-before-reload');
    expect(html).toContain('settings.repositoryPush.unknown');
    expect(html).toContain('settings.repositoryPush.recheck');
    expect(html).toMatch(/<select[^>]*disabled/);
    expect(client.pushRepository).not.toHaveBeenCalled();
    expect(client.verifyRepositoryPush).not.toHaveBeenCalled();
  });
});
