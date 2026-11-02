import type { AgentRunStatus } from 'shared/types';
import type { UseTranslationResponse } from 'react-i18next';
export type SessionRuntimeTranslator = UseTranslationResponse<
  'tasks',
  undefined
>['t'];
export function getSessionRuntimeActionBlockedReasonLabel(
  reason: string | null | undefined,
  t: SessionRuntimeTranslator
): string | null {
  if (!reason) return null;
  const labels: Record<string, string> = {
    no_content: t('tasks:sessionRuntime.blocked.no_content'),
    no_workspace: t('tasks:sessionRuntime.blocked.no_workspace'),
    no_session: t('tasks:sessionRuntime.blocked.no_session'),
    no_executor: t('tasks:sessionRuntime.blocked.no_executor'),
    provider_not_ready: t('tasks:sessionRuntime.blocked.provider_not_ready'),
    provider_capability_missing: t(
      'tasks:sessionRuntime.blocked.provider_capability_missing'
    ),
    runtime_starting: t('tasks:sessionRuntime.blocked.runtime_starting'),
    runtime_busy: t('tasks:sessionRuntime.blocked.runtime_busy'),
    runtime_cancelling: t('tasks:sessionRuntime.blocked.runtime_cancelling'),
    runtime_terminal: t('tasks:sessionRuntime.blocked.runtime_terminal'),
    approval_timed_out: t('tasks:sessionRuntime.blocked.approval_timed_out'),
    approval_required: t('tasks:sessionRuntime.blocked.approval_required'),
    question_required: t('tasks:sessionRuntime.blocked.question_required'),
    queue_already_present: t(
      'tasks:sessionRuntime.blocked.queue_already_present'
    ),
    queue_empty: t('tasks:sessionRuntime.blocked.queue_empty'),
    unknown_runtime: t('tasks:sessionRuntime.blocked.unknown_runtime'),
  };
  return Object.prototype.hasOwnProperty.call(labels, reason)
    ? labels[reason]
    : t('tasks:sessionRuntime.blocked.unavailable');
}

export function getSessionRunStatusLabel(
  status: string | null | undefined,
  t: SessionRuntimeTranslator
): string {
  const labels: Record<AgentRunStatus, string> = {
    pending: t('tasks:sessionRuntime.runStatus.pending'),
    starting: t('tasks:sessionRuntime.runStatus.starting'),
    running: t('tasks:sessionRuntime.runStatus.running'),
    awaiting_input: t('tasks:sessionRuntime.runStatus.awaiting_input'),
    awaiting_approval: t('tasks:sessionRuntime.runStatus.awaiting_approval'),
    cancelling: t('tasks:sessionRuntime.runStatus.cancelling'),
    succeeded: t('tasks:sessionRuntime.runStatus.succeeded'),
    failed: t('tasks:sessionRuntime.runStatus.failed'),
    cancelled: t('tasks:sessionRuntime.runStatus.cancelled'),
    crashed: t('tasks:sessionRuntime.runStatus.crashed'),
    audit_failed: t('tasks:sessionRuntime.runStatus.audit_failed'),
  };
  return status && Object.prototype.hasOwnProperty.call(labels, status)
    ? labels[status as AgentRunStatus]
    : t('tasks:sessionRuntime.runStatus.unknown');
}
