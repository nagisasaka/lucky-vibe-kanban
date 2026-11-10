import { useTranslation } from 'react-i18next';
import type { IntegrationRun } from 'shared/types';
import { savedIntegrationInput } from '../model/integrationInitialMessage';

/** The saved LVK envelope is displayed as product input, preserving its model role. */
export function IntegrationInitialMessage({ run }: { run: IntegrationRun }) {
  const { t } = useTranslation('common');
  const saved = savedIntegrationInput(run);
  return (
    <section className="space-y-base rounded border border-border p-base bg-panel">
      <h3 className="text-high font-medium">{t('integrationPanel.title')}</h3>
      <p className="text-low">{t('integrationPanel.generatedInput')}</p>
      <ul>
        {run.payload.sources.map((source) => (
          <li key={source.selection.workspace_id} className="break-all">
            {source.title} · {source.selection.workspace_id} · {source.commit}
          </li>
        ))}
      </ul>
      <p className="break-all">
        {t('integrationFacts.target')}: {run.target_ref} /{' '}
        {run.payload.observed_target}
      </p>
      <p>
        {t('integrationPanel.validationPolicy')}:{' '}
        {run.payload.skip_validation
          ? t('integrationPanel.skipRequested')
          : t('integrationPanel.riskBased')}
      </p>
      <p>
        {t('integrationPanel.wikiPolicy')}:{' '}
        {run.payload.wiki_enabled
          ? t('integrationPanel.wikiSameCandidate')
          : t('integrationPanel.wikiDisabled')}
      </p>
      <details>
        <summary>{t('integrationPanel.additionalInstructions')}</summary>
        <pre className="whitespace-pre-wrap break-words">
          {run.payload.instructions}
        </pre>
      </details>
      <details>
        <summary>{t('integrationPanel.savedInput')}</summary>
        <p className="break-all">
          {t('integrationPanel.savedInputIdentity', {
            run: saved?.agent_run_id,
            message: saved?.input.message_id,
          })}
        </p>
        <pre className="whitespace-pre-wrap break-words">
          {saved?.input.content}
        </pre>
      </details>
    </section>
  );
}
