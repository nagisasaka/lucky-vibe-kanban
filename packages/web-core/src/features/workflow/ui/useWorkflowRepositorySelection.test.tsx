import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useWorkflowRepositorySelection } from './useWorkflowRepositorySelection';

const { defaults, repo, show } = vi.hoisted(() => ({
  defaults: vi.fn(),
  repo: vi.fn(),
  show: vi.fn(),
}));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));
vi.mock('@/shared/hooks/useUserContext', () => ({
  useUserContext: () => ({ workspaces: [] }),
}));
vi.mock('@/shared/hooks/useWorkspaceContext', () => ({
  useWorkspaceContext: () => ({ activeWorkspaces: [], archivedWorkspaces: [] }),
}));
vi.mock('@/shared/hooks/useCurrentKanbanRouteState', () => ({
  useCurrentKanbanRouteState: () => ({ hostId: 'fixture-host' }),
}));
vi.mock('@/shared/lib/workspaceDefaults', () => ({
  getWorkspaceDefaults: defaults,
}));
vi.mock('@/shared/lib/api', () => ({ repoApi: { getById: repo } }));
vi.mock('@/shared/dialogs/shared/WorkspaceTargetDialog', () => ({
  WorkspaceTargetDialog: { show },
}));

function selection() {
  let select!: ReturnType<
    typeof useWorkflowRepositorySelection
  >['selectWorkflowRepositories'];
  function Probe() {
    select = useWorkflowRepositorySelection({
      projectId: 'project',
      issueId: 'issue',
      issueTitle: 'Synthetic',
    }).selectWorkflowRepositories;
    return null;
  }
  renderToStaticMarkup(createElement(Probe));
  return select;
}

describe('Workflow repository selection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    defaults.mockResolvedValue({
      preferredRepos: [
        { repo_id: 'b', target_branch: 'base-b' },
        { repo_id: 'a', target_branch: 'base-a' },
      ],
    });
    repo.mockResolvedValue({ id: 'b', path: '/fixture/repo b' });
  });

  it('seeds only the preferred Repo and returns the user-confirmed Repo', async () => {
    show.mockResolvedValue({
      kind: 'confirmed',
      selection: {
        mode: 'worktree',
        repo: { id: 'a' },
        targetBranch: 'chosen-a',
      },
    });
    expect(await selection()()).toEqual([
      { repo_id: 'a', target_branch: 'chosen-a' },
    ]);
    expect(repo).toHaveBeenCalledWith('b', 'fixture-host');
    expect(show).toHaveBeenCalledWith(
      expect.objectContaining({
        singleRepository: true,
        initialRepos: [{ repo_id: 'b', target_branch: 'base-b' }],
        initialPath: '/fixture/repo b',
        hostId: 'fixture-host',
        projectId: 'project',
      })
    );
  });

  it('keeps cancellation empty and lets the picker handle an absent preference', async () => {
    defaults.mockResolvedValue(null);
    show.mockResolvedValue({ kind: 'canceled' });
    expect(await selection()()).toBeNull();
    expect(repo).not.toHaveBeenCalled();
    expect(show).toHaveBeenCalledWith(
      expect.objectContaining({
        initialRepos: undefined,
        singleRepository: true,
      })
    );
  });
});
