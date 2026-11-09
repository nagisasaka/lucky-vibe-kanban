import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { describe, expect, it, vi } from 'vitest';
import { ProjectWorkspaceDefaultContext } from './ProjectWorkspaceDefaultContext';
import { getProjectWorkspaceDefaultOrThrow } from '@/shared/hooks/useProjectRepoDefaults';
import { repoApi } from '@/shared/lib/api';

const { queries } = vi.hoisted(() => ({ queries: [] as unknown[] }));
vi.mock('@tanstack/react-query', async (original) => {
  const actual = await original<typeof import('@tanstack/react-query')>();
  return {
    ...actual,
    useQuery: (options: Parameters<typeof actual.useQuery>[0]) => {
      queries.push(options);
      return actual.useQuery(options);
    },
  };
});
vi.mock('@/shared/hooks/useProjectRepoDefaults', async (original) => ({
  ...(await original<object>()),
  getProjectWorkspaceDefaultOrThrow: vi.fn(),
}));
vi.mock('@/shared/lib/api', () => ({ repoApi: { getById: vi.fn() } }));
vi.mock('@/shared/dialogs/settings/SettingsDialog', () => ({
  SettingsDialog: { show: vi.fn() },
}));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

describe('project workspace context', () => {
  it.each(['', '  ', 'main'])(
    'displays a repository preset with branch %j',
    async (branch) => {
      vi.mocked(getProjectWorkspaceDefaultOrThrow).mockResolvedValue({
        kind: 'git',
        repo: { repo_id: 'repo-1', target_branch: branch },
      });
      vi.mocked(repoApi.getById).mockResolvedValue({
        path: '/synthetic/repo',
      } as Awaited<ReturnType<typeof repoApi.getById>>);
      const client = new QueryClient();
      const render = () =>
        renderToStaticMarkup(
          createElement(
            QueryClientProvider,
            { client },
            createElement(ProjectWorkspaceDefaultContext, {
              projectId: 'project-1',
              organizationId: 'org-1',
            })
          )
        );
      try {
        render();
        await client.fetchQuery(
          queries.at(-1) as Parameters<typeof client.fetchQuery>[0]
        );
        const markup = render();
        expect(markup).toContain('/synthetic/repo');
        expect(markup).not.toContain('projectWorkspaceDefault.loadFailed');
        expect(markup).toContain(
          `aria-label="/synthetic/repo${branch.trim() ? ' / main' : ''}"`
        );
      } finally {
        client.clear();
      }
    }
  );
});
