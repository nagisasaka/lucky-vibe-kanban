import { Fragment, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { splitPromptContext } from '../lib/splitPromptContext';

export function PromptContext({
  content,
  renderContent,
}: {
  content: string;
  renderContent: (content: string) => ReactNode;
}) {
  const { t } = useTranslation('tasks');
  const [copyStatus, setCopyStatus] = useState<'copied' | 'copyFailed' | null>(
    null
  );
  const parts = splitPromptContext(content);
  if (!parts.some((part) => part.auxiliary)) return renderContent(content);

  return (
    <div className="space-y-base">
      {parts.map((part, index) =>
        part.auxiliary ? (
          <details key={index} className="text-sm text-low">
            <summary className="cursor-pointer focus-visible:outline-brand">
              {t('conversation.promptContext.auxiliary')}
            </summary>
            <pre className="whitespace-pre-wrap break-words font-ibm-plex-mono text-xs text-normal">
              {part.content}
            </pre>
          </details>
        ) : (
          <Fragment key={index}>{renderContent(part.content)}</Fragment>
        )
      )}
      <details className="text-sm text-low">
        <summary className="cursor-pointer focus-visible:outline-brand">
          {t('conversation.promptContext.fullText')}
        </summary>
        <pre className="whitespace-pre-wrap break-words font-ibm-plex-mono text-xs text-normal">
          {content}
        </pre>
      </details>
      <button
        type="button"
        className="text-xs text-low underline focus-visible:outline-brand"
        onClick={async () => {
          try {
            await navigator.clipboard.writeText(content);
            setCopyStatus('copied');
          } catch {
            setCopyStatus('copyFailed');
          }
        }}
      >
        {t('conversation.promptContext.copyFullText')}
      </button>
      {copyStatus && (
        <span role="status" className="ml-base text-xs text-low">
          {t(`conversation.promptContext.${copyStatus}`)}
        </span>
      )}
    </div>
  );
}
