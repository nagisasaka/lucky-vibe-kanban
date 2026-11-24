import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { splitMarkdownFrontmatter } from '../../model/markdownFrontmatter';
import { MarkdownPreview } from '@/shared/components/MarkdownPreview';
import { getActualTheme } from '@/shared/lib/theme';
import { useTheme } from '@/shared/hooks/useTheme';

interface MarkdownFileViewerProps {
  content: string;
  contentKey?: string;
}

export function MarkdownFileViewer({
  content,
  contentKey,
}: MarkdownFileViewerProps) {
  const { t } = useTranslation('common');
  const { body, metadata } = useMemo(
    () => splitMarkdownFrontmatter(content),
    [content]
  );
  const { theme } = useTheme();
  const actualTheme = getActualTheme(theme);

  return (
    <div className="h-full min-h-0 overflow-auto bg-primary px-double py-base">
      {metadata !== null && (
        <details
          key={contentKey ?? content}
          className="mx-auto mb-base max-w-[820px] rounded border border-border bg-secondary text-sm text-normal"
        >
          <summary className="cursor-pointer rounded px-base py-half text-low focus-visible:outline focus-visible:outline-2 focus-visible:outline-brand">
            {t('workspaceFiles.metadata')}
          </summary>
          <pre className="overflow-x-auto whitespace-pre-wrap break-words border-t border-border p-base font-ibm-plex-mono text-sm">
            <code>{metadata}</code>
          </pre>
        </details>
      )}
      <MarkdownPreview
        outline
        contentKey={contentKey}
        content={body}
        theme={actualTheme}
        className="mx-auto max-w-[820px] [&_pre]:max-w-full"
      />
    </div>
  );
}
