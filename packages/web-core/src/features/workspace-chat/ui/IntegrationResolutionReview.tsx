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
    onSuccess: () => client.invalidateQueries({ queryKey: ['integrations'] }),
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
          <p className="whitespace-pre-wrap">{review.explanation}</p>
          <p className="text-xs break-all">{review.candidate}</p>
          <p>{t('integrationReview.diffHelp')}</p>
          <pre
            aria-label={t('integrationReview.diff')}
            className="overflow-auto max-h-96 text-xs bg-primary p-half"
          >
            {review.diff || t('integrationReview.emptyDiff')}
          </pre>
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
      {approve.error && <p role="alert">{approve.error.message}</p>}
    </div>
  );
}
