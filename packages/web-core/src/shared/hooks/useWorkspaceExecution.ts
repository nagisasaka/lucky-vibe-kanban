import { useMemo, useCallback } from 'react';
import { useMutation, useMutationState } from '@tanstack/react-query';
import { workspacesApi } from '@/shared/lib/api';
import { useExecutionProcessesContext } from '@/shared/hooks/useExecutionProcessesContext';
import type { AttemptData } from '@/shared/lib/types';

export function useWorkspaceExecution(workspaceId?: string) {
  const stopMutationKey = useMemo(
    () => ['stopWorkspaceExecution', workspaceId] as const,
    [workspaceId]
  );

  const stopMutation = useMutation({
    mutationKey: stopMutationKey,
    mutationFn: async () => {
      if (!workspaceId) return;
      await workspacesApi.stop(workspaceId);
    },
  });

  const isStopping =
    useMutationState({
      filters: {
        mutationKey: stopMutationKey,
        status: 'pending',
      },
    }).length > 0;

  const {
    executionProcessesVisible: executionProcesses,
    isAttemptRunningVisible: isAttemptRunning,
    isLoading: streamLoading,
  } = useExecutionProcessesContext();

  const attemptData: AttemptData = useMemo(
    () => ({ processes: executionProcesses }),
    [executionProcesses]
  );

  const stopExecution = useCallback(async () => {
    if (!workspaceId || isStopping) return;

    try {
      await stopMutation.mutateAsync();
    } catch (error) {
      console.error('Failed to stop executions:', error);
      throw error;
    }
  }, [workspaceId, isStopping, stopMutation]);

  return {
    // Data
    processes: executionProcesses,
    attemptData,

    // Status
    isAttemptRunning,
    isLoading: streamLoading,
    isFetching: streamLoading,

    // Actions
    stopExecution,
    isStopping,
  };
}
