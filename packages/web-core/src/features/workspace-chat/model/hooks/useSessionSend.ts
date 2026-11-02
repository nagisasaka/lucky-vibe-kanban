import { errorDiagnostic } from '@/shared/lib/errorDiagnostic';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import type { ExecutorConfig, SelectedSkill } from 'shared/types';
import { sessionsApi } from '@/shared/lib/api';
import { useCreateSession } from './useCreateSession';
import { goalPromptError, type GoalPromptError } from '../goalValidation';
import { useTranslation } from 'react-i18next';
import { useEntries } from '../contexts/EntriesContext';
import {
  chatMessageScope,
  deliveryFailure,
  usePendingChatMessages,
} from '../pendingChatMessages';
import { useHostId } from '@/shared/providers/HostIdProvider';

interface UseSessionSendOptions {
  /** Session ID for existing sessions */
  sessionId: string | undefined;
  /** Workspace ID for creating new sessions */
  workspaceId: string | undefined;
  /** Whether in new session mode */
  isNewSessionMode: boolean;
  /** Callback when session is selected (to exit new session mode) */
  onSelectSession?: (sessionId: string) => void;
  /** Unified executor config (executor + variant + overrides) */
  executorConfig?: ExecutorConfig | null;
}

interface UseSessionSendResult {
  /** Send a message; the caller decides whether to select a newly created Session. */
  send: (
    message: string,
    selectedSkills?: SelectedSkill[],
    options?: {
      resumeSessionId?: string | null;
      resumeScopePath?: string | null;
      executorConfig?: ExecutorConfig;
    }
  ) => Promise<{ createdSessionId?: string } | null>;
  /** Whether a send operation is in progress */
  isSending: boolean;
  /** Error message if send failed */
  error: string | null;
  /** Clear the error */
  clearError: () => void;
}

/**
 * Hook for sending messages in SessionChatBoxContainer.
 * Handles both new session creation and existing session follow-up.
 *
 * Unlike useFollowUpSend, this hook:
 * - Takes message/variant as parameters to send() (not captured in closure)
 * - Returns the created identity on success (caller handles scoped cleanup)
 * - Has no prompt composition (no conflict/review/clicked markdown)
 */
export function useSessionSend({
  sessionId,
  workspaceId,
  isNewSessionMode,
  executorConfig,
}: UseSessionSendOptions): UseSessionSendResult {
  const { t, i18n } = useTranslation('tasks');
  const hostId = useHostId();
  const { entries } = useEntries();
  const entriesRef = useRef(entries);
  entriesRef.current = entries;
  const queryClient = useQueryClient();
  const scope = JSON.stringify([
    hostId,
    workspaceId,
    sessionId,
    isNewSessionMode,
  ]);
  const inFlight = useRef(new Set<string>());
  const currentScope = useRef(scope);
  currentScope.current = scope;
  const { mutateAsync: createSession, isPending: isCreatingSession } =
    useCreateSession();
  const [isSendingFollowUp, setIsSendingFollowUp] = useState(false);
  const [error, setError] = useState<
    | GoalPromptError
    | 'no_executor'
    | 'no_workspace'
    | 'submission_unconfirmed'
    | null
  >(null);
  const [errorDetail, setErrorDetail] = useState<string | null>(null);
  useEffect(() => {
    setError(null);
    setErrorDetail(null);
  }, [scope]);

  const send = useCallback(
    async (
      message: string,
      selectedSkills: SelectedSkill[] = [],
      options: {
        resumeSessionId?: string | null;
        resumeScopePath?: string | null;
        executorConfig?: ExecutorConfig;
      } = {}
    ): Promise<{ createdSessionId?: string } | null> => {
      const trimmed = message.trim();
      const effectiveConfig = options.executorConfig ?? executorConfig;
      if (!trimmed) return null;
      if (!effectiveConfig) {
        setError('no_executor');
        return null;
      }

      setError(null);
      setErrorDetail(null);
      const validationError = goalPromptError(trimmed, effectiveConfig);
      if (validationError) {
        setError(validationError);
        return null;
      }

      if (inFlight.current.has(scope)) return null;
      inFlight.current.add(scope);
      const pending = usePendingChatMessages.getState();
      const pendingId = pending.begin(
        chatMessageScope(
          hostId,
          workspaceId,
          isNewSessionMode ? undefined : sessionId
        ),
        trimmed,
        entriesRef.current
      );
      try {
        if (isNewSessionMode) {
          // New session flow
          if (!workspaceId) {
            pending.update(pendingId, {
              state: 'failed',
              error: 'No workspace selected',
            });
            setError('no_workspace');
            return null;
          }
          try {
            const session = await createSession({
              hostId,
              workspaceId,
              prompt: trimmed,
              selectedSkills,
              executorConfig: effectiveConfig,
              resumeSessionId: options.resumeSessionId,
              resumeScopePath: options.resumeScopePath,
            });
            pending.update(pendingId, {
              state: 'accepted',
              runId: session.acceptedRunId,
              scope: chatMessageScope(hostId, workspaceId, session.id),
            });
            return { createdSessionId: session.id };
          } catch (e: unknown) {
            pending.update(pendingId, deliveryFailure(e));
            if (currentScope.current === scope) {
              setErrorDetail(errorDiagnostic(e));
              setError('submission_unconfirmed');
            }
            return null;
          }
        } else {
          // Existing session flow
          if (!sessionId) {
            pending.remove([pendingId]);
            return null;
          }
          setIsSendingFollowUp(true);
          try {
            const accepted = await sessionsApi.followUp(
              sessionId,
              {
                prompt: trimmed,
                selected_skills: selectedSkills,
                executor_config: effectiveConfig,
                resume_session_id: options.resumeSessionId || undefined,
                resume_scope_path: options.resumeScopePath || undefined,
              },
              hostId
            );
            pending.update(pendingId, {
              state: 'accepted',
              runId: accepted.agent_run_id,
            });
            void queryClient.invalidateQueries({
              queryKey: ['session-executor-config', hostId, sessionId],
            });
            return {};
          } catch (e: unknown) {
            pending.update(pendingId, deliveryFailure(e));
            if (currentScope.current === scope) {
              setErrorDetail(errorDiagnostic(e));
              setError('submission_unconfirmed');
            }
            return null;
          } finally {
            setIsSendingFollowUp(false);
          }
        }
      } finally {
        inFlight.current.delete(scope);
      }
    },
    [
      sessionId,
      workspaceId,
      isNewSessionMode,
      createSession,
      scope,
      hostId,
      queryClient,
      executorConfig,
    ]
  );

  const clearError = useCallback(() => {
    setError(null);
    setErrorDetail(null);
  }, []);

  return {
    send,
    isSending: isSendingFollowUp || isCreatingSession,
    error:
      error === null
        ? null
        : typeof error === 'object'
          ? t('sessionRuntime.goalObjectiveTooLong', {
              formattedCount: new Intl.NumberFormat(
                i18n.resolvedLanguage || i18n.language
              ).format(error.count),
              maximum: new Intl.NumberFormat(
                i18n.resolvedLanguage || i18n.language
              ).format(error.maximum),
            })
          : error === 'no_executor'
            ? t('sessionRuntime.noExecutor')
            : error === 'no_workspace'
              ? t('sessionRuntime.noWorkspace')
              : `${t('sessionRuntime.submissionUnconfirmed')} ${errorDetail ?? ''}`,
    clearError,
  };
}
