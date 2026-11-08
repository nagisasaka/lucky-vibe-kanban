import { useCallback, useEffect, useMemo } from 'react';
import type { BaseCodingAgent, ExecutorDiscoveredOptions } from 'shared/types';
import { useJsonPatchWsStream } from '@/shared/hooks/useJsonPatchWsStream';
import { useHostId } from '@/shared/providers/HostIdProvider';
import { agentsApi } from '@/shared/lib/api';

type ExecutorDiscoveryStreamState = {
  options: ExecutorDiscoveredOptions | null;
};

const defaultOptions: ExecutorDiscoveredOptions = {
  model_selector: {
    providers: [],
    models: [],
    default_model: null,
    agents: [],
    permissions: [],
  },
  slash_commands: [],
  skills: [],
  skill_errors: [],
  loading_models: true,
  loading_agents: true,
  loading_slash_commands: true,
  loading_skills: true,
  error: null,
};

type DiscoveryScope = {
  workspaceId?: string;
  sessionId?: string;
  repoId?: string;
  repoIds?: string[];
  variant?: string | null;
};

export interface ExecutorDiscoverySnapshot {
  scope: DiscoveryScope & {
    agent: BaseCodingAgent | null | undefined;
    hostId: string | null;
  };
  options: ExecutorDiscoveredOptions | null;
  error: string | null;
  isConnected: boolean;
  isInitialized: boolean;
}

export function useExecutorDiscovery(
  agent: BaseCodingAgent | null | undefined,
  opts?: DiscoveryScope,
  shared?: ExecutorDiscoverySnapshot
): ExecutorDiscoverySnapshot {
  const hostId = useHostId();
  const { workspaceId, sessionId, repoId, variant } = opts ?? {};
  const repoIdsKey = opts?.repoIds?.join(',') ?? '';
  const matchesShared =
    !!shared &&
    shared.scope.agent === agent &&
    shared.scope.hostId === hostId &&
    shared.scope.workspaceId === workspaceId &&
    shared.scope.sessionId === sessionId &&
    shared.scope.repoId === repoId &&
    (shared.scope.repoIds?.join(',') ?? '') === repoIdsKey &&
    shared.scope.variant === variant;
  const endpoint = useMemo(() => {
    if (!agent || matchesShared) return undefined;
    return agentsApi.getDiscoveredOptionsStreamUrl(agent, {
      workspaceId,
      sessionId,
      repoId,
      repoIds: repoIdsKey ? repoIdsKey.split(',') : undefined,
      variant,
    });
  }, [
    agent,
    workspaceId,
    sessionId,
    repoId,
    repoIdsKey,
    variant,
    matchesShared,
  ]);

  const initialData = useCallback(
    (): ExecutorDiscoveryStreamState => ({
      options: { ...defaultOptions },
    }),
    []
  );

  const { data, error, isConnected, isInitialized } =
    useJsonPatchWsStream<ExecutorDiscoveryStreamState>(
      endpoint,
      !!endpoint,
      initialData
    );

  // Prefer the backend-reported error from the data payload. Only fall back
  // to the WebSocket transport error when no data has been received yet —
  // transient connection failures (e.g. React StrictMode double-mount or
  // Safari/macOS 26 WebSocket instability) should not persist once data
  // has successfully loaded.
  const combinedError = data?.options?.error ?? (isInitialized ? null : error);

  useEffect(() => {
    if (combinedError) {
      console.error(
        'Failed to fetch executor discovery options',
        combinedError
      );
    }
  }, [combinedError]);

  if (matchesShared) return shared;

  return {
    scope: {
      agent,
      hostId,
      workspaceId,
      sessionId,
      repoId,
      repoIds: opts?.repoIds,
      variant,
    },
    options: data?.options ?? null,
    error: combinedError,
    isConnected,
    isInitialized,
  };
}

export function useModelSelectorConfig(
  agent: BaseCodingAgent | null | undefined,
  opts?: DiscoveryScope,
  shared?: ExecutorDiscoverySnapshot
) {
  const { options, error, isConnected, isInitialized } = useExecutorDiscovery(
    agent,
    opts,
    shared
  );

  return {
    config: options?.model_selector ?? null,
    loadingModels: options?.loading_models ?? false,
    loadingAgents: options?.loading_agents ?? false,
    error,
    isConnected,
    isInitialized,
  };
}

export function useExecutorTooling(
  agent: BaseCodingAgent | null | undefined,
  opts?: DiscoveryScope,
  shared?: ExecutorDiscoverySnapshot
) {
  const { options, error, isConnected, isInitialized } = useExecutorDiscovery(
    agent,
    opts,
    shared
  );

  return {
    commands: options?.slash_commands ?? [],
    skills: options?.skills ?? [],
    skillErrors: options?.skill_errors ?? [],
    discoveringCommands: options?.loading_slash_commands ?? false,
    discoveringSkills: options?.loading_skills ?? false,
    error,
    isConnected,
    isInitialized,
  };
}
