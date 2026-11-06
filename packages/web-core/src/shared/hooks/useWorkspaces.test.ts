import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { WorkspaceSummary, WorkspaceWithStatus } from 'shared/types';
import { useWorkspaces, type UseWorkspacesResult } from './useWorkspaces';

const fixture = vi.hoisted(() => ({
  summaries: new Map<string, WorkspaceSummary>(),
  archivedWorkspace: undefined as WorkspaceWithStatus | undefined,
  queries: [] as Array<{
    refetchInterval:
      | number
      | ((query: {
          state: { data?: Map<string, WorkspaceSummary> };
        }) => number);
  }>,
  workspace: {} as WorkspaceWithStatus,
}));

vi.mock('@/shared/providers/HostIdProvider', () => ({
  useHostId: () => null,
}));
vi.mock('@/shared/hooks/useJsonPatchWsStream', () => ({
  useJsonPatchWsStream: (endpoint: string) => ({
    data: {
      workspaces: endpoint.endsWith('archived=false')
        ? { [fixture.workspace.id]: fixture.workspace }
        : fixture.archivedWorkspace
          ? { [fixture.archivedWorkspace.id]: fixture.archivedWorkspace }
          : {},
    },
    isInitialized: true,
    isConnected: true,
    error: null,
  }),
}));
vi.mock('@tanstack/react-query', () => ({
  keepPreviousData: (data: unknown) => data,
  useQuery: (options: (typeof fixture.queries)[number]) => {
    fixture.queries.push(options);
    return { data: fixture.summaries };
  },
}));

function readWorkspaces() {
  let result: UseWorkspacesResult | undefined;
  function Probe() {
    result = useWorkspaces();
    return null;
  }
  renderToStaticMarkup(createElement(Probe));
  return result!;
}

describe('workspace sidebar runtime state', () => {
  beforeEach(() => {
    fixture.archivedWorkspace = undefined;
    fixture.queries = [];
    fixture.workspace = {
      id: 'workspace',
      name: 'OpenWiki maintenance',
      created_at: '2026-09-17T00:00:00Z',
      updated_at: '2026-09-17T00:00:00Z',
      is_running: true,
      archived: false,
      usage: 'interactive',
    } as WorkspaceWithStatus;
    fixture.summaries = new Map();
  });

  it('keeps human workspaces even when their name resembles an internal execution', () => {
    const result = readWorkspaces();
    expect(result.workspaces.map((workspace) => workspace.id)).toEqual([
      'workspace',
    ]);
  });

  it.each([
    'integration',
    'openwiki_sync',
    'openwiki_bootstrap',
    'resource_mediation',
  ])(
    'shows %s in the normal workspace list even after its agent exits',
    (kind) => {
      fixture.workspace.usage = 'execution_only';
      fixture.workspace.is_running = false;
      fixture.workspace.execution_owner = {
        kind,
        run_id: 'run',
        repository_id: 'repo',
        result: null,
      };
      const result = readWorkspaces();
      expect(result.workspaces.map((workspace) => workspace.id)).toEqual([
        'workspace',
      ]);
      expect(result.archivedWorkspaces).toEqual([]);
    }
  );

  it('converges after a terminal AgentRun even when the legacy workspace stream stays running', () => {
    expect(readWorkspaces().workspaces[0].isRunning).toBe(true);
    fixture.summaries = new Map([
      [
        'workspace',
        {
          workspace_id: 'workspace',
          is_running: false,
          latest_process_status: 'succeeded',
        } as WorkspaceSummary,
      ],
    ]);
    expect(readWorkspaces().workspaces[0].isRunning).toBe(false);
  });

  it.each(['succeeded', null] as const)(
    'does not infer inactivity from the latest AgentRun (%s) when another agent or script is active',
    (latestStatus) => {
      fixture.workspace.is_running = false;
      fixture.summaries.set('workspace', {
        workspace_id: 'workspace',
        is_running: true,
        latest_process_status: latestStatus,
      } as WorkspaceSummary);
      expect(readWorkspaces().workspaces[0].isRunning).toBe(true);
    }
  );

  it('retains streaming status before summary fetch or with an older host response', () => {
    fixture.summaries.set('workspace', {
      workspace_id: 'workspace',
      latest_process_status: 'succeeded',
    } as WorkspaceSummary);
    expect(readWorkspaces().workspaces[0].isRunning).toBe(true);
  });
});

describe('archived summary cadence', () => {
  function interval() {
    readWorkspaces();
    expect(fixture.queries.at(-2)!.refetchInterval).toBe(15000);
    const archived = fixture.queries.at(-1)!.refetchInterval;
    if (typeof archived !== 'function')
      throw new Error('Expected dynamic archive interval');
    return archived({ state: { data: fixture.summaries } });
  }

  beforeEach(() => {
    fixture.queries = [];
    fixture.summaries = new Map();
    fixture.archivedWorkspace = {
      id: 'archive',
      archived: true,
      is_running: false,
    } as WorkspaceWithStatus;
  });

  it('uses 60 seconds normally while retaining archive metadata', () => {
    expect(interval()).toBe(60000);
    expect(readWorkspaces().archivedWorkspaces[0].id).toBe('archive');
  });

  it.each(['is_running', 'has_pending_approval'] as const)(
    'keeps 15 seconds for known %s',
    (field) => {
      fixture.summaries.set('archive', {
        workspace_id: 'archive',
        [field]: true,
      } as WorkspaceSummary);
      expect(interval()).toBe(15000);
      fixture.summaries.clear();
      expect(interval()).toBe(60000);
    }
  );

  it('keeps 15 seconds when metadata reports activity before summary catches up', () => {
    fixture.archivedWorkspace!.is_running = true;
    fixture.summaries.set('archive', {
      workspace_id: 'archive',
      is_running: false,
    } as WorkspaceSummary);
    expect(interval()).toBe(15000);
  });
});

// Summary and metadata can arrive in either order.
it('keeps known summary approvals fresh before archived metadata arrives', () => {
  fixture.queries = [];
  fixture.archivedWorkspace = undefined;
  fixture.summaries = new Map([
    [
      'archive',
      {
        workspace_id: 'archive',
        has_pending_approval: true,
      } as WorkspaceSummary,
    ],
  ]);
  readWorkspaces();
  const interval = fixture.queries.at(-1)!.refetchInterval;
  expect(
    typeof interval === 'function' &&
      interval({ state: { data: fixture.summaries } })
  ).toBe(15000);
});
