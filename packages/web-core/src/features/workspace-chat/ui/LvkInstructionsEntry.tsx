import { useTranslation } from 'react-i18next';
import type { PatchTypeWithKey } from '@/shared/hooks/useConversationHistory/types';

export function LvkInstructionsEntry({
  delivery,
  runId,
}: {
  delivery?: NonNullable<PatchTypeWithKey['instructionDelivery']>;
  runId?: string;
}) {
  const { t } = useTranslation('common');
  return (
    <details
      className="my-half rounded border border-border text-sm"
      data-testid="lvk-instructions"
    >
      <summary className="cursor-pointer px-base py-half text-low focus-visible:outline focus-visible:outline-2">
        {t('lvkInstructions.title')}
      </summary>
      <div className="space-y-base border-t border-border p-base">
        {!delivery && (
          <p className="text-low">{t('lvkInstructions.missingExplanation')}</p>
        )}
        {delivery && (
          <>
            <p className="text-low">
              {t(`lvkInstructions.${delivery.status}`)}
            </p>
            <dl className="break-all text-xs text-low">
              <dt>{t('lvkInstructions.route')}</dt>
              <dd>
                {delivery.route} · {delivery.role} · {delivery.reason}
              </dd>
              <dt>{t('lvkInstructions.runNativeSession')}</dt>
              <dd>
                {runId ?? '—'} / {delivery.nativeSessionId ?? '—'}
              </dd>
              <dt>{t('lvkInstructions.timing')}</dt>
              <dd>
                {delivery.attemptedAt ?? '—'} → {delivery.updatedAt}
              </dd>
            </dl>
            <pre className="whitespace-pre-wrap break-words font-ibm-plex-mono text-xs text-normal">
              {delivery.content}
            </pre>
          </>
        )}
      </div>
    </details>
  );
}
