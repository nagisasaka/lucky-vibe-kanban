import { useTranslation } from 'react-i18next';
import type { WikiQualityWarning } from 'shared/types';

export function WikiQualityWarnings({
  warnings,
}: {
  warnings?: WikiQualityWarning[] | null;
}) {
  const { t } = useTranslation('common');
  if (!warnings?.length) return null;
  return (
    <details className="border-b p-base text-sm text-normal" role="status">
      <summary>{t('workspaceWiki.qualityWarnings')}</summary>
      {warnings.map((warning, index) => (
        <div
          key={`${warning.node_id}-${index}`}
          className="space-y-half py-half"
        >
          <p>{warning.summary}</p>
          {warning.findings.map((finding, findingIndex) => (
            <div key={findingIndex}>
              <p>
                {finding.title}: {finding.description}
              </p>
              <p className="break-all text-low">
                {finding.evidence_paths.join(', ')}
              </p>
            </div>
          ))}
        </div>
      ))}
    </details>
  );
}
