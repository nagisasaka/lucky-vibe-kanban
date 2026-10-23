import { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import {
  AgentProviderReadiness,
  AgentProviderDiagnosticKind,
  type AgentGarageEntry,
  type AgentProviderCapability,
  type AgentProviderPolicy,
  type BaseCodingAgent,
} from 'shared/types';
import { agentsApi } from '@/shared/lib/api';
import {
  deriveAgentProviderOptions,
  type AgentProviderOption,
} from '@/shared/lib/agentProviderOptions';
import { filterVisibleAgents } from '@/shared/lib/agentVisibility';
import { useUserSystem } from '@/shared/hooks/useUserSystem';

export const agentProviderPolicyKeys = {
  all: ['agent-provider-policy'] as const,
  garage: () => [...agentProviderPolicyKeys.all, 'garage'] as const,
};

const EMPTY_REQUIRED_CAPABILITIES: readonly AgentProviderCapability[] = [];

export function useAgentProviderPolicy(
  executor: BaseCodingAgent | null | undefined,
  variant?: string | null,
  scope?: { workspaceId?: string; sessionId?: string }
): AgentProviderPolicy | null {
  const { data: garage } = useQuery<AgentGarageEntry[]>({
    queryKey: agentProviderPolicyKeys.garage(),
    queryFn: agentsApi.getGarage,
    enabled: !!executor,
    staleTime: 5 * 60 * 1000,
  });

  const isClaude = executor === 'CLAUDE_CODE';
  const { data: profilePolicy, isError: profileError } = useQuery({
    queryKey: [
      ...agentProviderPolicyKeys.all,
      'profile',
      executor,
      variant ?? null,
      scope?.workspaceId ?? null,
      scope?.sessionId ?? null,
    ],
    queryFn: () =>
      agentsApi.getProfilePolicy({
        executor: executor!,
        variant: variant ?? null,
        workspace_id: scope?.workspaceId,
        session_id: scope?.sessionId,
      }),
    enabled: isClaude,
    staleTime: 0,
    retry: false,
  });

  return useMemo(() => {
    if (!executor) return null;
    if (isClaude) {
      if (profileError) {
        return {
          executor,
          readiness: AgentProviderReadiness.UNKNOWN,
          capabilities: [],
          legacy: false,
          disabled: false,
          diagnostics: [
            {
              kind: AgentProviderDiagnosticKind.UNKNOWN,
              message:
                'Unable to check the selected agent profile. Reopen the session to retry.',
            },
          ],
        };
      }
      return profilePolicy ?? null;
    }
    return garage?.find((entry) => entry.executor === executor)?.policy ?? null;
  }, [executor, garage, isClaude, profileError, profilePolicy]);
}

interface UseAgentProviderOptionsInput {
  executors?: readonly BaseCodingAgent[] | null;
  fallbackExecutors?: readonly BaseCodingAgent[];
  preserveExecutors?: readonly (BaseCodingAgent | null | undefined)[];
  requiredCapabilities?: readonly AgentProviderCapability[];
  enabled?: boolean;
}

interface UseAgentProviderOptionsResult {
  options: AgentProviderOption[];
  garage: AgentGarageEntry[] | null;
  isLoading: boolean;
  isError: boolean;
}

export function useAgentProviderOptions({
  executors,
  fallbackExecutors,
  preserveExecutors,
  requiredCapabilities = EMPTY_REQUIRED_CAPABILITIES,
  enabled = true,
}: UseAgentProviderOptionsInput = {}): UseAgentProviderOptionsResult {
  const { config } = useUserSystem();
  const {
    data: garage,
    isLoading,
    isError,
  } = useQuery<AgentGarageEntry[]>({
    queryKey: agentProviderPolicyKeys.garage(),
    queryFn: agentsApi.getGarage,
    enabled,
    staleTime: 5 * 60 * 1000,
  });

  const visibleExecutors = useMemo(() => {
    if (executors !== undefined && executors !== null) {
      return filterVisibleAgents({
        agents: executors,
        hiddenAgents: config?.hidden_agents,
        preserveAgents: preserveExecutors,
      });
    }

    if (garage !== undefined && garage !== null) {
      return filterVisibleAgents({
        agents: garage.map((entry) => entry.executor),
        hiddenAgents: config?.hidden_agents,
        preserveAgents: preserveExecutors,
      });
    }

    return undefined;
  }, [config?.hidden_agents, executors, garage, preserveExecutors]);

  const visibleFallbackExecutors = useMemo(
    () =>
      fallbackExecutors
        ? filterVisibleAgents({
            agents: fallbackExecutors,
            hiddenAgents: config?.hidden_agents,
            preserveAgents: preserveExecutors,
          })
        : fallbackExecutors,
    [config?.hidden_agents, fallbackExecutors, preserveExecutors]
  );

  const options = useMemo(
    () =>
      deriveAgentProviderOptions({
        garage,
        executors: visibleExecutors,
        fallbackExecutors: visibleFallbackExecutors,
        requiredCapabilities,
      }),
    [garage, requiredCapabilities, visibleExecutors, visibleFallbackExecutors]
  );

  return {
    options,
    garage: garage ?? null,
    isLoading,
    isError,
  };
}
