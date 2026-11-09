import { useTranslation } from 'react-i18next';
import { FileSearch, Loader2 } from 'lucide-react';
import { Button } from '@vibe/ui/components/Button';
import { cn } from '@/shared/lib/utils';
import { useHostId } from '@/shared/providers/HostIdProvider';
import { useWorkspaceFileContent } from '../model/useWorkspaceFileContent';
import type { WorkspaceFilePreviewTarget } from '../model/types';
import { classifyWorkspaceFile } from '../model/fileClassification';
import { scopeWorkspaceFileRawUrl } from '../model/workspaceFileRawUrl';
import { WorkspaceFileEmptyState } from './WorkspaceFileEmptyState';
import { WorkspaceFilePreviewHeader } from './WorkspaceFilePreviewHeader';
import { CodeTextViewer } from './viewers/CodeTextViewer';
import { MarkdownFileViewer } from './viewers/MarkdownFileViewer';
import { ImageFileViewer } from './viewers/ImageFileViewer';
import { UnsupportedFileViewer } from './viewers/UnsupportedFileViewer';

interface WorkspaceFilePreviewPanelProps {
  target: WorkspaceFilePreviewTarget | null;
  className?: string;
  onClearTarget?: () => void;
}

export function WorkspaceFilePreviewPanel({
  target,
  className,
  onClearTarget,
}: WorkspaceFilePreviewPanelProps) {
  const { t } = useTranslation('common');
  const hostId = useHostId();
  const { data, isLoading, isFetching, error, refresh, refetch } =
    useWorkspaceFileContent(target);
  const displayInfo = data
    ? classifyWorkspaceFile(data, {
        svgSource: t('workspaceFiles.kindSvg'),
        image: t('workspaceFiles.kindImage'),
        text: t('workspaceFiles.kindText'),
        unsupported: t('workspaceFiles.kindUnsupported'),
      })
    : null;
  const scopedRawUrl = scopeWorkspaceFileRawUrl(data?.raw_url, hostId);

  if (!target) {
    return (
      <div className={cn('flex h-full min-h-0 flex-col bg-primary', className)}>
        <WorkspaceFileEmptyState
          icon={<FileSearch className="size-5" />}
          title={t('workspaceFiles.select_a_file')}
          description={t(
            'workspaceFiles.open_a_workspace_file_to_inspect_it_here'
          )}
        />
      </div>
    );
  }

  const handleRefresh = () => {
    void refresh();
  };

  return (
    <div className={cn('flex h-full min-h-0 flex-col bg-primary', className)}>
      <WorkspaceFilePreviewHeader
        target={target}
        content={data}
        displayInfo={displayInfo}
        isRefreshing={isFetching && !isLoading}
        onRefresh={handleRefresh}
        onClearTarget={onClearTarget}
      />
      {data?.truncated && (
        <div className="shrink-0 border-b border-border bg-secondary px-base py-half text-sm text-low">
          {t('workspaceFiles.large_file_preview_truncated_by_the_backend')}
        </div>
      )}
      <div className="min-h-0 flex-1">
        {isLoading ? (
          <WorkspaceFileEmptyState
            icon={<Loader2 className="size-5 animate-spin" />}
            title={t('workspaceFiles.loading_preview')}
            description={t(
              'workspaceFiles.reading_file_content_from_the_workspace'
            )}
          />
        ) : error ? (
          <WorkspaceFileEmptyState
            title={t('workspaceFiles.could_not_load_preview')}
            description={`${t('workspaceFiles.previewUnconfirmed')} ${error instanceof Error ? error.message : String(error)}`}
            action={
              <Button
                type="button"
                size="xs"
                variant="secondary"
                onClick={() => void refetch()}
              >
                {t('workspaceFiles.retry')}
              </Button>
            }
          />
        ) : !data ? (
          <WorkspaceFileEmptyState
            title={t('workspaceFiles.file_missing')}
            description={t(
              'workspaceFiles.the_file_was_not_returned_by_the_workspace_api'
            )}
          />
        ) : displayInfo?.viewKind === 'markdown' && !target.line ? (
          <MarkdownFileViewer
            content={data.content ?? ''}
            contentKey={JSON.stringify([
              target.workspaceId,
              target.repoId,
              target.path,
            ])}
          />
        ) : displayInfo?.viewKind === 'image' ? (
          <ImageFileViewer src={scopedRawUrl} alt={data.name} />
        ) : displayInfo?.viewKind === 'code' ||
          displayInfo?.viewKind === 'text' ||
          displayInfo?.viewKind === 'markdown' ? (
          <CodeTextViewer
            content={data.content ?? ''}
            language={displayInfo.language}
            line={target.line}
          />
        ) : (
          <UnsupportedFileViewer
            target={target}
            content={data}
            rawUrl={scopedRawUrl}
          />
        )}
      </div>
    </div>
  );
}
