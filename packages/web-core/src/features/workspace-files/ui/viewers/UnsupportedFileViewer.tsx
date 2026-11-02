import { useTranslation } from 'react-i18next';
import { Binary, ExternalLink } from 'lucide-react';
import { Button } from '@vibe/ui/components/Button';
import { formatFileSize } from '@vibe/ui/lib/formatFileSize';
import { useOpenInEditor } from '@/shared/hooks/useOpenInEditor';
import { useWorkspaceRecord } from '@/shared/hooks/useWorkspaceRecord';
import type { WorkspaceFileContent } from 'shared/types';
import type { WorkspaceFilePreviewTarget } from '../../model/types';
import { WorkspaceFileEmptyState } from '../WorkspaceFileEmptyState';

interface UnsupportedFileViewerProps {
  target: WorkspaceFilePreviewTarget;
  content?: WorkspaceFileContent | null;
  rawUrl?: string | null;
}

export function UnsupportedFileViewer({
  target,
  content,
  rawUrl,
}: UnsupportedFileViewerProps) {
  const { t, i18n } = useTranslation('common');
  const openInEditor = useOpenInEditor(target.workspaceId);
  const { data: workspace } = useWorkspaceRecord(target.workspaceId);

  return (
    <WorkspaceFileEmptyState
      icon={<Binary className="size-5" />}
      title={t('workspaceFiles.preview_not_available')}
      description={t(
        'workspaceFiles.this_file_is_binary_or_not_supported_by_the_read_only_preview'
      )}
      action={
        <div className="flex max-w-full flex-col items-center gap-base">
          <dl className="grid max-w-[420px] grid-cols-[auto_1fr] gap-x-base gap-y-half text-left text-sm">
            <dt className="text-low">{t('workspaceFiles.path')}</dt>
            <dd className="min-w-0 truncate font-ibm-plex-mono text-normal">
              {target.path}
            </dd>
            {content?.mime_type && (
              <>
                <dt className="text-low">{t('workspaceFiles.mime')}</dt>
                <dd className="min-w-0 truncate text-normal">
                  {content.mime_type}
                </dd>
              </>
            )}
            {content && (
              <>
                <dt className="text-low">{t('workspaceFiles.size')}</dt>
                <dd className="text-normal">
                  {formatFileSize(
                    content.size_bytes,
                    i18n.resolvedLanguage || i18n.language
                  )}
                </dd>
              </>
            )}
          </dl>
          <div className="flex items-center gap-half">
            {workspace?.usage === 'interactive' && (
              <Button
                type="button"
                size="xs"
                variant="secondary"
                onClick={() =>
                  void openInEditor({
                    filePath: target.path,
                    repoId: target.repoId,
                  })
                }
                className="gap-half"
              >
                <ExternalLink className="size-3" />
                {t('workspaceFiles.open_editor')}
              </Button>
            )}
            {rawUrl && (
              <Button
                asChild
                size="xs"
                variant="secondary"
                className="gap-half"
              >
                <a href={rawUrl} target="_blank" rel="noopener noreferrer">
                  {t('workspaceFiles.open_raw')}
                </a>
              </Button>
            )}
          </div>
        </div>
      }
    />
  );
}
