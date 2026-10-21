import { useRef } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import type { IntegrationRun } from 'shared/types';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { integrationApi } from '@/shared/lib/integrationApi';

export function IntegrationResolutionReview({ run }: { run: IntegrationRun }) {
  const { t } = useTranslation('common');
  const client = useQueryClient();
  const requests = useRef(new Map<string, string>());
  const approve = useMutation({
    mutationFn: (id: string) => {
      const request = requests.current.get(id) ?? crypto.randomUUID();
      requests.current.set(id, request);
      return integrationApi.decide(run.id, {
        project_id: run.project_id,
        gate_id: id,
        request_id: request,
        decision: 'continue',
      });
    },
    onSettled: () => client.invalidateQueries({ queryKey: ['integrations'] }),
  });
  return (
    <div className="space-y-base">
      {(run.payload.reviews ?? []).map((review) => (
        <section
          key={review.id}
          className="border border-border rounded-sm p-base space-y-half"
        >
          <strong>
            {t('integrationReview.title')} · {review.step_index + 1}
          </strong>
          <p className="whitespace-pre-wrap break-words">
            {review.explanation.trim() ||
              t('integrationReview.missingExplanation')}
          </p>
          <p className="text-low">{t('integrationReview.explanationHelp')}</p>
          {!review.decision &&
            review.candidate !== run.payload.result_commit && (
              <p role="status">{t('integrationReview.stale')}</p>
            )}
          {review.candidate === run.payload.result_commit &&
            !run.payload.validation.length && (
              <p>{t('integrationReview.notValidated')}</p>
            )}
          <details>
            <summary className="cursor-pointer focus-visible:outline focus-visible:outline-2">
              {t('integrationReview.details')}
            </summary>
            <p className="text-xs break-all">
              {t('integrationReview.candidate')}: {review.candidate}
            </p>
            <p>{t('integrationReview.diffHelp')}</p>
            <pre
              aria-label={t('integrationReview.diff')}
              className="overflow-auto max-h-96 text-xs bg-primary p-half"
            >
              {review.diff || t('integrationReview.emptyDiff')}
            </pre>
          </details>
          {review.candidate === run.payload.result_commit && (
            <ul>
              {run.payload.validation.map((check, index) => (
                <li key={index}>
                  {check.command} —{' '}
                  {check.result ?? t('integrationGate.notRun')}
                </li>
              ))}
            </ul>
          )}
          {review.decision ? (
            <p>
              {t(
                `integrationReview.${review.decision === 'continue' ? 'approved' : 'rejected'}`
              )}
            </p>
          ) : (
            !run.cancel_requested &&
            !run.payload.publication_intent &&
            run.status === 'blocked' &&
            run.payload.stop?.category === 'merge_review' &&
            review.candidate === run.payload.result_commit && (
              <>
                <PrimaryButton
                  disabled={approve.isPending}
                  onClick={() => approve.mutate(review.id)}
                >
                  {t('integrationReview.approve')}
                </PrimaryButton>
                <p>{t('integrationReview.rejectHelp')}</p>
              </>
            )
          )}
        </section>
      ))}
      {approve.error && (
        <p role="alert">
          {t('integrationReview.approvalFailed')} {approve.error.message}
        </p>
      )}
    </div>
  );
}
