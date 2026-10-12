import { useRef, useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import type { IntegrationRun } from 'shared/types';
import { Button } from '@vibe/ui/components/Button';
import { integrationApi } from '@/shared/lib/integrationApi';

function confirmationTarget(scope: string): string {
  try {
    const value = JSON.parse(scope);
    return [value.candidate, value.target]
      .filter((item): item is string => typeof item === 'string')
      .join(' · ');
  } catch {
    return '—';
  }
}

export function IntegrationGateControls({ run }: { run: IntegrationRun }) {
  const { t } = useTranslation('common');
  const client = useQueryClient();
  const [acknowledged, setAcknowledged] = useState<string | null>(null);
  const requests = useRef(new Map<string, string>());
  const mutation = useMutation({
    mutationFn: async ({
      gateId,
      decision,
    }: {
      gateId: string;
      decision: 'continue' | 'retry' | 'cancel';
    }) => {
      const key = `${gateId}:${decision}`;
      const requestId = requests.current.get(key) ?? crypto.randomUUID();
      requests.current.set(key, requestId);
      return integrationApi.decide(run.id, {
        project_id: run.project_id,
        gate_id: gateId,
        request_id: requestId,
        decision,
      });
    },
    onSuccess: () => client.invalidateQueries({ queryKey: ['integrations'] }),
  });
  return (
    <div className="space-y-half">
      {(run.payload.gates ?? []).map((gate) => (
        <div key={gate.id} className="border p-base space-y-half">
          <p>
            {t('integrationGate.check')}: {gate.evidence.command} (
            {gate.evidence.cwd}) —{' '}
            {gate.evidence.result ?? t('integrationGate.notRun')}
          </p>
          <p className="break-all">{confirmationTarget(gate.scope)}</p>
          <p>{gate.risk}</p>
          {gate.decision ? (
            <p>
              {gate.decision === 'continue'
                ? t('integrationGate.excepted')
                : gate.decision}{' '}
              · {gate.actor} · {gate.decided_at}
            </p>
          ) : (
            run.status === 'blocked' && (
              <>
                <label className="flex gap-half">
                  <input
                    type="checkbox"
                    checked={acknowledged === gate.id}
                    onChange={(e) =>
                      setAcknowledged(e.target.checked ? gate.id : null)
                    }
                  />
                  {t('integrationGate.acknowledge')}
                </label>
                <div className="flex gap-half">
                  <Button
                    disabled={mutation.isPending}
                    onClick={() =>
                      mutation.mutate({ gateId: gate.id, decision: 'retry' })
                    }
                  >
                    {t('integrationGate.retry')}
                  </Button>
                  <Button
                    disabled={acknowledged !== gate.id || mutation.isPending}
                    onClick={() =>
                      mutation.mutate({ gateId: gate.id, decision: 'continue' })
                    }
                  >
                    {t('integrationGate.continue')}
                  </Button>
                  <Button
                    disabled={mutation.isPending}
                    onClick={() =>
                      mutation.mutate({ gateId: gate.id, decision: 'cancel' })
                    }
                  >
                    {t('integrationGate.cancel')}
                  </Button>
                </div>
              </>
            )
          )}
        </div>
      ))}
      {mutation.error && <p role="alert">{mutation.error.message}</p>}
    </div>
  );
}
