import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

/** A provider-authored plan snapshot, separate from published reasoning. */
export function ChatProviderPlan({
  content,
  steps,
  renderMarkdown,
}: {
  content: string;
  steps?: { step: string; status: string }[];
  renderMarkdown: (content: string) => ReactNode;
}) {
  const { t } = useTranslation('common');
  if (!content.trim() && !steps) return null;
  return (
    <section
      className="rounded-sm border border-border p-base text-sm"
      aria-label={t('conversation.plan')}
    >
      <div className="mb-half text-xs text-low">{t('conversation.plan')}</div>
      {steps &&
        (steps.length ? (
          <ul className="space-y-half">
            {steps.map((step, index) => (
              <li key={index} className="flex gap-base">
                <span className="shrink-0 text-xs text-low">
                  {[
                    'pending',
                    'inProgress',
                    'in_progress',
                    'completed',
                  ].includes(step.status)
                    ? t(
                        `conversation.planStep.${step.status === 'in_progress' ? 'inProgress' : step.status}`
                      )
                    : step.status}
                </span>
                <span className="whitespace-pre-wrap">{step.step}</span>
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-xs text-low">{t('conversation.planStep.empty')}</p>
        ))}
      {content.trim() && renderMarkdown(content)}
    </section>
  );
}
