import {
  executionOwnerLabel,
  executionStatusLabel,
} from '../model/executionPresentation';
import { useTranslation } from 'react-i18next';
import { useEffect, useState } from 'react';
import { useMutation } from '@tanstack/react-query';
import type { Session, Workspace } from 'shared/types';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { useWorkspaceOwnerView } from '@/shared/hooks/useWorkspaceOwner';
import { useWorkspacePendingApproval } from '../model/hooks/useWorkspacePendingApproval';
import { useCanonicalAgentSession } from '../model/contexts/EntriesContext';
import { canonicalAgentControls } from '../model/canonicalAgentControls';

/** Active execution controls. Completed reservations return to the normal composer. */
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
  const { t } = useTranslation('tasks');
  const wiki = ['openwiki_sync', 'openwiki_bootstrap'].includes(
    workspace.execution_owner?.kind ?? ''
  );
  const { timeline } = useCanonicalAgentSession();
  const activeRun = timeline?.activeRun;
  const [instruction, setInstruction] = useState('');
  const steer = useMutation({
    mutationFn: async () => {
      if (!activeRun || !instruction.trim()) return;
      await canonicalAgentControls.steer(
        activeRun.summary.agent_run_id,
        instruction.trim()
      );
      setInstruction('');
    },
  });
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
  const sessionSelector = (
    <label className="flex items-center gap-base text-normal">
      {t('executionInspection.session')}
      <select
        aria-label={t('executionInspection.sessionLabel')}
        className="bg-primary border border-border rounded-sm p-half min-w-0 flex-1"
        value={selectedSessionId ?? ''}
        onChange={(event) => onSelectSession(event.target.value)}
      >
        {!selectedSessionId && (
          <option value="">{t('executionInspection.selectSession')}</option>
        )}
        {sessions.map((session) => (
          <option key={session.id} value={session.id}>
            {session.name ?? session.id}
          </option>
        ))}
      </select>
    </label>
  );
  return (
    <section
      className="border-t border-border bg-secondary p-base flex flex-col gap-half text-sm"
      aria-label={t('executionInspection.panelLabel')}
    >
      <div className="flex items-center justify-between gap-base">
        <strong className="text-high">
          {wiki
            ? t('executionInspection.wiki')
            : t('executionInspection.execution')}
        </strong>
        <PrimaryButton
          variant="secondary"
          disabled={!run?.can_stop || execution.stop.isPending}
          onClick={() => execution.stop.mutate()}
        >
          {run?.stop_requested || execution.stop.isPending
            ? t('executionInspection.stopping')
            : t('executionInspection.stop')}
        </PrimaryButton>
      </div>
      <p className="text-low">{t('executionInspection.help')}</p>
      <>
        <p className="text-normal">
          {run
            ? executionStatusLabel(t, run.status)
            : t('executionInspection.loading')}{' '}
          ·{' '}
          {executionOwnerLabel(
            t,
            run?.owner?.kind ?? workspace.execution_owner?.kind
          )}
        </p>
        {run?.owner?.run_id && (
          <p className="font-mono text-xs text-low break-all">
            {t('executionInspection.runId', { id: run.owner.run_id })}
          </p>
        )}
        {/* Integration publication is shown below from the full run, including its intent. */}
        {run && (
          <p className="text-low">
            {run.owner?.result?.no_op && run.status === 'succeeded'
              ? t('executionInspection.noChanges')
              : run.published
                ? t('executionInspection.published')
                : t('executionInspection.unconfirmed')}
          </p>
        )}
      </>
      {run && !run.files_available && (
        <p className="text-warning">{t('executionInspection.missingFiles')}</p>
      )}
      {[
        run?.error && t('executionInspection.reportedError'),
        execution.error && t('executionInspection.loadError'),
        execution.stop.error && t('executionInspection.stopError'),
        response.error && t('executionInspection.responseError'),
        steer.error && t('executionInspection.steerError'),
      ]
        .filter(Boolean)
        .map((message, index) => (
          <p key={index} role="alert" className="text-error break-words">
            {message}
          </p>
        ))}
      {sessionSelector}
      {wiki && activeRun && run?.can_stop && !run.stop_requested && (
        <div className="flex flex-col gap-half">
          <textarea
            aria-label={t('executionInspection.wikiInstructions')}
            className="bg-primary border border-border rounded-sm p-half"
            value={instruction}
            onChange={(event) => setInstruction(event.target.value)}
            placeholder={t('executionInspection.wikiPlaceholder')}
          />
          <PrimaryButton
            disabled={steer.isPending || !instruction.trim()}
            onClick={() => steer.mutate()}
          >
            {t('executionInspection.sendInstructions')}
          </PrimaryButton>
        </div>
      )}
      {pending && (
        <div className="border border-border p-base flex flex-col gap-half">
          <p>
            {pending.kind === 'input'
              ? pending.prompt
              : t('executionInspection.approvalRequested', {
                  tool: pending.toolName,
                })}
          </p>
          <textarea
            key={pending.controlId}
            aria-label={t('executionInspection.responseLabel')}
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
              {pending.kind === 'input'
                ? t('executionInspection.respond')
                : t('executionInspection.approve')}
            </PrimaryButton>
            {pending.kind === 'approval' && (
              <PrimaryButton
                variant="secondary"
                disabled={
                  response.isPending || !run || run.status === 'unknown'
                }
                onClick={() => response.mutate(false)}
              >
                {t('executionInspection.deny')}
              </PrimaryButton>
            )}
          </div>
        </div>
      )}
    </section>
  );
}
