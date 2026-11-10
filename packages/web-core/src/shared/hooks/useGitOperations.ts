import { errorDiagnostic } from '@/shared/lib/errorDiagnostic';
import { useRebase } from '@/shared/hooks/useRebase';
import { usePush } from '@/shared/hooks/usePush';
import { useForcePush } from '@/shared/hooks/useForcePush';
import { useChangeTargetBranch } from '@/shared/hooks/useChangeTargetBranch';
import { useGitOperationsError } from '@/shared/hooks/GitOperationsContext';
import { Result } from '@/shared/lib/api';
import type { GitOperationError, PushWorkspaceRequest } from 'shared/types';
import { ForcePushDialog } from '@/shared/dialogs/command-bar/ForcePushDialog';

export function useGitOperations(
  workspaceId: string | undefined,
  repoId: string | undefined
) {
  const { setError } = useGitOperationsError();

  const rebase = useRebase(
    workspaceId,
    repoId,
    () => setError(null),
    (err: Result<void, GitOperationError>) => {
      if (!err.success) {
        const data = err?.error;
        const isConflict =
          data?.type === 'merge_conflicts' ||
          data?.type === 'rebase_in_progress';
        if (!isConflict) {
          setError('rebase', errorDiagnostic(err));
        }
      }
    }
  );

  const forcePush = useForcePush(
    workspaceId,
    () => setError(null),
    (error) => setError('forcePush', errorDiagnostic(error))
  );

  const push = usePush(
    workspaceId,
    () => setError(null),
    async (_err: unknown, errorData, params?: PushWorkspaceRequest) => {
      // Handle typed push errors
      if (errorData?.type === 'force_push_required') {
        // Show confirmation dialog - dialog handles the force push internally
        if (workspaceId && params?.repo_id) {
          await ForcePushDialog.show({ workspaceId, repoId: params.repo_id });
        }
        return;
      }

      setError('push', errorDiagnostic(_err));
    }
  );

  const changeTargetBranch = useChangeTargetBranch(
    workspaceId,
    repoId,
    () => setError(null),
    (error) => setError('targetBranch', errorDiagnostic(error))
  );

  const isAnyLoading =
    rebase.isPending ||
    push.isPending ||
    forcePush.isPending ||
    changeTargetBranch.isPending;

  return {
    actions: {
      rebase: rebase.mutateAsync,
      push: push.mutateAsync,
      forcePush: forcePush.mutateAsync,
      changeTargetBranch: changeTargetBranch.mutateAsync,
    },
    isAnyLoading,
    states: {
      rebasePending: rebase.isPending,
      pushPending: push.isPending,
      forcePushPending: forcePush.isPending,
      changeTargetBranchPending: changeTargetBranch.isPending,
    },
  };
}
