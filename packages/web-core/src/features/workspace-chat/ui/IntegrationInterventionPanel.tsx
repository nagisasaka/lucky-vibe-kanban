import { IntegrationResolutionReview } from './IntegrationResolutionReview';
import { useRef, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { integrationApi } from '@/shared/lib/integrationApi';
import { IntegrationGateControls } from '@/features/kanban/ui/IntegrationGateControls';

export function IntegrationInterventionPanel({
  integrationId,
}: {
  integrationId: string;
}) {
  const { t } = useTranslation('common');
  const client = useQueryClient();
  const [prompt, setPrompt] = useState('');
  const identity = useRef<{
    prompt: string;
    id: string;
    createdAt: string;
    agentId: string | null;
  } | null>(null);
  const query = useQuery({
    queryKey: ['integrations', integrationId],
    queryFn: () => integrationApi.get(integrationId),
    refetchInterval: 3000,
  });
  const run = query.data;
  const reviewing = run?.payload.stop?.category === 'merge_review';
  const send = useMutation({
    mutationFn: async () => {
      if (!run) return;
      if (identity.current?.prompt !== prompt)
        identity.current = {
          prompt,
          id: crypto.randomUUID(),
          createdAt: new Date().toISOString(),
          agentId: run.status === 'integrating' ? run.agent_run_id : null,
        };
      if (identity.current.agentId) {
        await integrationApi.steer(identity.current.agentId, {
          content: prompt,
          command_id: identity.current.id,
          idempotency_key: `integration:${run.id}:instruction:${identity.current.id}`,
          correlation_id: run.id,
          created_at: identity.current.createdAt,
        });
      } else {
        await integrationApi.intervene(run.id, {
          project_id: run.project_id,
          request_id: identity.current.id,
          prompt,
        });
      }
    },
    onSuccess: () => {
      setPrompt('');
      identity.current = null;
      void client.invalidateQueries({ queryKey: ['integrations'] });
    },
  });
  const cancel = useMutation({
    mutationFn: () => integrationApi.cancel(integrationId),
    onSuccess: () => client.invalidateQueries({ queryKey: ['integrations'] }),
  });
  return (
    <div className="space-y-half">
      <p>
        {t('integrationReview.progress', {
          current: (run?.payload.step_index ?? 0) + 1,
          total: run?.payload.sources?.length ?? 0,
        })}{' '}
        · {run?.status}
      </p>
      {run && <IntegrationResolutionReview run={run} />}
      <p>{t('integrationGate.instructionBoundary')}</p>
      {run?.error && (
        <p role="alert" className="whitespace-pre-wrap">
          {run.error}
        </p>
      )}
      {run?.payload.stop && (
        <p>
          {run.payload.stop.stage}: {run.payload.stop.next_action}
        </p>
      )}
      {run && <IntegrationGateControls run={run} />}
      {(!!run?.payload.validation?.length ||
        !!run?.payload.exclusions?.length) && (
        <details open={run?.status === 'blocked'}>
          <summary>{t('integrationReview.validationPlan')}</summary>
          {run?.payload.validation?.map((check, index) => (
            <div key={index} className="border p-half space-y-half">
              <pre className="whitespace-pre-wrap break-words text-xs">
                {check.command}
              </pre>
              <p>
                {check.cwd} · {check.result ?? t('integrationGate.notRun')}
              </p>
              <p className="whitespace-pre-wrap">
                {t('integrationReview.evidence')}: {check.evidence}
              </p>
              <p className="whitespace-pre-wrap">
                {t('integrationReview.prerequisites')}:{' '}
                {check.environment_requirements}
              </p>
            </div>
          ))}
          {!!run?.payload.exclusions?.length && (
            <div>
              <p>{t('integrationReview.exclusions')}</p>
              <ul>
                {run.payload.exclusions.map((reason, index) => (
                  <li key={index}>{reason}</li>
                ))}
              </ul>
            </div>
          )}
        </details>
      )}
      {!!run?.payload.steps?.length && (
        <details>
          <summary>{t('integrationReview.tests')}</summary>
          {run.payload.steps.map((step, index) => (
            <div key={`${index}:${step.result_commit}`}>
              <p>
                {index + 1}. {step.summary}
              </p>
              {step.validation.map((check, i) => (
                <p key={i}>
                  {check.command} — {check.result}
                </p>
              ))}
            </div>
          ))}
        </details>
      )}
      {!!run?.payload.history?.length && (
        <details>
          <summary>{t('integrationGate.history')}</summary>
          <pre className="whitespace-pre-wrap break-words text-xs">
            {JSON.stringify(run.payload.history, null, 2)}
          </pre>
        </details>
      )}
      <textarea
        aria-label={t('integrationGate.instruction')}
        value={prompt}
        onChange={(e) => setPrompt(e.target.value)}
        className="bg-primary border border-border rounded-sm p-half w-full"
      />
      <PrimaryButton
        disabled={
          send.isPending ||
          !run ||
          !['blocked', 'integrating'].includes(run.status) ||
          ((run.status === 'integrating' || reviewing) && !prompt.trim()) ||
          run.payload.publication_intent
        }
        onClick={() => send.mutate()}
      >
        {reviewing
          ? t('integrationReview.revise')
          : run?.status === 'integrating'
            ? t('integrationGate.send')
            : t('integrationGate.resume')}
      </PrimaryButton>
      {run &&
        !['succeeded', 'cancelled', 'failed'].includes(run.status) &&
        !run.payload.publication_intent && (
          <PrimaryButton
            variant="secondary"
            disabled={cancel.isPending || send.isPending}
            onClick={() => cancel.mutate()}
          >
            {t('integrationGate.cancel')}
          </PrimaryButton>
        )}
      {(send.error || query.error || cancel.error) && (
        <p role="alert">
          {send.error?.message ?? query.error?.message ?? cancel.error?.message}
        </p>
      )}
    </div>
  );
}
