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
      <p>{t('integrationGate.instructionBoundary')}</p>
      {run?.error && <p role="alert">{run.error}</p>}
      {run?.payload.stop && (
        <p>
          {run.payload.stop.stage}: {run.payload.stop.next_action}
        </p>
      )}
      {run && <IntegrationGateControls run={run} />}
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
          (run.status === 'integrating' && !prompt.trim()) ||
          run.payload.publication_intent
        }
        onClick={() => send.mutate()}
      >
        {run?.status === 'integrating'
          ? t('integrationGate.send')
          : t('integrationGate.resume')}
      </PrimaryButton>
      {run?.status === 'blocked' && !run.payload.publication_intent && (
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
