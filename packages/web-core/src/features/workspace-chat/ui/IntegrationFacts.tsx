import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import type { Workspace } from 'shared/types';
import { integrationApi } from '@/shared/lib/integrationApi';

/** Host facts in the normal conversation; no execution controls or model-text parsing. */
export function IntegrationFacts({ workspace }: { workspace: Workspace }) {
  const { t } = useTranslation('common');
  const id =
    workspace.execution_owner?.kind === 'integration'
      ? workspace.execution_owner.run_id
      : undefined;
  const { data: run } = useQuery({
    queryKey: ['integrations', id],
    queryFn: () => integrationApi.get(id!),
    enabled: !!id,
    refetchInterval: 2000,
  });
  if (!run?.payload.conversational) return null;
  return (
    <div className="my-base px-double text-sm" aria-live="polite">
      <p>
        {t('integrationFacts.target')}: {run.target_ref}
      </p>
      {run.payload.result_commit && (
        <p>
          {t('integrationFacts.candidate')}:{' '}
          <code>{run.payload.result_commit}</code>
        </p>
      )}
      {run.payload.evidence.map((e) => (
        <p key={e.request_id}>
          {t(`integrationFacts.${e.result}`, { defaultValue: e.result })}:{' '}
          <code>{e.code_commit}</code>
          {' · '}
          {e.command ?? t('integrationFacts.validation')}
          {' · '}
          {e.actor}
          {' · '}
          {e.reason}
          {e.remaining_risk && ` · ${e.remaining_risk}`}
        </p>
      ))}
      {run.payload.publication_request?.wiki_commit && (
        <p>
          {t('integrationFacts.wiki')}:{' '}
          <code>{run.payload.publication_request.wiki_commit}</code>
        </p>
      )}
      {run.payload.published ? (
        <p className="font-bold text-success">
          {t('integrationFacts.published')}:{' '}
          <code>{run.payload.result_commit}</code>
        </p>
      ) : (
        <p>
          {t(
            run.payload.publication_intent
              ? 'integrationFacts.reconciling'
              : run.payload.publication_request
                ? 'integrationFacts.pendingSettlement'
                : 'integrationFacts.unpublished'
          )}
        </p>
      )}
      {run.error && (
        <>
          <p role="status">
            {run.payload.stop?.next_action ?? t('integrationFacts.stopped')}
          </p>
          <details>
            <summary>{t('integrationFacts.details')}</summary>
            <p>{run.error}</p>
          </details>
        </>
      )}
    </div>
  );
}
