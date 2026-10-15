import { useTranslation } from 'react-i18next';
import { IntegrationInterventionPanel } from './IntegrationInterventionPanel';
import { useEffect, useState } from 'react';
import { useMutation } from '@tanstack/react-query';
import type { Session, Workspace } from 'shared/types';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { useWorkspaceOwnerView } from '@/shared/hooks/useWorkspaceOwner';
import { useWorkspacePendingApproval } from '@/features/workspace-chat/model/hooks/useWorkspacePendingApproval';
import { canonicalAgentControls } from '@/features/workspace-chat/model/canonicalAgentControls';

/** Owner inspection preserves existing question/tool controls. Only Integration
 * exposes dedicated instructions; no general composer or session creation. */
export function ExecutionInspectionPanel({
  workspace,
  sessions,
  selectedSessionId,
  onSelectSession,
}: {
  workspace: Workspace;
  sessions: Session[];
  selectedSessionId?: string;
  onSelectSession: (id: string) => void;
}) {
  const { t } = useTranslation('common');
  const integration = workspace.execution_owner?.kind === 'integration';
  const execution = useWorkspaceOwnerView(workspace.id);
  const pending = useWorkspacePendingApproval();
  const [answer, setAnswer] = useState('');
  useEffect(() => {
    setAnswer('');
  }, [pending?.controlId]);
  const response = useMutation({
    mutationFn: async (approved: boolean) => {
      if (!pending) return;
      if (pending.kind === 'approval') {
        if (approved)
          await canonicalAgentControls.approve(
            pending.agentRunId,
            pending.approvalId
          );
        else
          await canonicalAgentControls.deny(
            pending.agentRunId,
            pending.approvalId,
            answer || undefined
          );
      } else {
        await canonicalAgentControls.submitInput(
          pending.agentRunId,
          pending.inputId,
          answer
        );
      }
      setAnswer('');
    },
  });
  const run = execution.data;
  return (
    <section
      className={`border-t border-border bg-secondary p-base flex flex-col gap-half text-sm${integration ? ' max-h-[65vh] overflow-y-auto' : ''}`}
      aria-label="Execution inspection"
    >
      <div className="flex items-center justify-between gap-base">
        <strong className="text-high">
          {integration
            ? t('integrationReview.title')
            : 'Execution-only · Inspection'}
        </strong>
        <PrimaryButton
          variant="secondary"
          disabled={!run?.can_stop || execution.stop.isPending}
          onClick={() => execution.stop.mutate()}
        >
          {run?.stop_requested || execution.stop.isPending
            ? 'Stopping…'
            : 'Stop execution'}
        </PrimaryButton>
      </div>
      {!integration && (
        <p className="text-low">
          The owner controls this environment. Files are shown from this
          execution workspace. Session creation and development actions remain
          owner-controlled.
        </p>
      )}
      <p className="text-normal">
        {run?.status ?? 'Loading execution…'} ·{' '}
        {run?.owner?.kind ?? workspace.execution_owner?.kind ?? 'Unknown owner'}
      </p>
      {run?.owner?.run_id && (
        <p className="font-mono text-xs text-low break-all">
          Run: {run.owner.run_id}
        </p>
      )}
      {run && (
        <p className="text-low">
          {run.owner?.result?.no_op && run.status === 'succeeded'
            ? 'No changes required (successful reconciliation)'
            : run.published
              ? 'Published result'
              : 'Publication not confirmed; files may be unpublished'}
        </p>
      )}
      {run && !run.files_available && (
        <p className="text-warning">
          Files are missing or unprepared. Inspection will not recreate the
          worktree. Saved logs remain available.
        </p>
      )}
      {[
        run?.error,
        execution.error?.message,
        execution.stop.error?.message,
        response.error?.message,
      ]
        .filter(Boolean)
        .map((error) => (
          <p key={error} role="alert" className="text-error break-words">
            {error}
          </p>
        ))}
      <label className="flex items-center gap-base text-normal">
        Session
        <select
          aria-label="Execution session"
          className="bg-primary border border-border rounded-sm p-half min-w-0 flex-1"
          value={selectedSessionId ?? ''}
          onChange={(event) => onSelectSession(event.target.value)}
        >
          {!selectedSessionId && <option value="">Select saved session</option>}
          {sessions.map((session) => (
            <option key={session.id} value={session.id}>
              {session.name ?? session.id}
            </option>
          ))}
        </select>
      </label>
      {run?.owner?.kind === 'integration' && run.owner.run_id && (
        <IntegrationInterventionPanel integrationId={run.owner.run_id} />
      )}
      {pending && (
        <div className="border border-border p-base flex flex-col gap-half">
          <p>
            {pending.kind === 'input'
              ? pending.prompt
              : `Approval requested: ${pending.toolName}`}
          </p>
          <textarea
            key={pending.controlId}
            aria-label="Owner request response"
            className="bg-primary border border-border rounded-sm p-half"
            value={answer}
            onChange={(event) => setAnswer(event.target.value)}
          />
          <div className="flex gap-half">
            <PrimaryButton
              disabled={
                response.isPending ||
                !run ||
                run.status === 'unknown' ||
                (pending.kind === 'input' && !answer.trim())
              }
              onClick={() => response.mutate(true)}
            >
              {pending.kind === 'input' ? 'Respond to request' : 'Approve'}
            </PrimaryButton>
            {pending.kind === 'approval' && (
              <PrimaryButton
                variant="secondary"
                disabled={
                  response.isPending || !run || run.status === 'unknown'
                }
                onClick={() => response.mutate(false)}
              >
                Deny
              </PrimaryButton>
            )}
          </div>
        </div>
      )}
    </section>
  );
}
