import { useTranslation } from 'react-i18next';
import { useMemo, useState } from 'react';
import {
  Copy,
  Download,
  ExternalLink,
  Loader2,
  RefreshCw,
  X,
} from 'lucide-react';
import { Tooltip } from '@vibe/ui/components/Tooltip';
import { cn } from '@/shared/lib/utils';
import { formatFileSize } from '@vibe/ui/lib/formatFileSize';
import { writeClipboardViaBridge } from '@/shared/lib/clipboard';
import { getFileIcon } from '@/shared/lib/fileTypeIcon';
import { getActualTheme } from '@/shared/lib/theme';
import { useTheme } from '@/shared/hooks/useTheme';
import { useOpenInEditor } from '@/shared/hooks/useOpenInEditor';
import { useHostId } from '@/shared/providers/HostIdProvider';
import { useWorkspaceRecord } from '@/shared/hooks/useWorkspaceRecord';
import type { WorkspaceFileContent } from 'shared/types';
import type {
  WorkspaceFileDisplayInfo,
  WorkspaceFilePreviewTarget,
} from '../model/types';
import { scopeWorkspaceFileRawUrl } from '../model/workspaceFileRawUrl';

interface WorkspaceFilePreviewHeaderProps {
  target: WorkspaceFilePreviewTarget;
  content?: WorkspaceFileContent | null;
  displayInfo?: WorkspaceFileDisplayInfo | null;
  isRefreshing?: boolean;
  onRefresh: () => void;
  onClearTarget?: () => void;
}

export function WorkspaceFilePreviewHeader({
  target,
  content,
  displayInfo,
  isRefreshing,
  onRefresh,
  onClearTarget,
}: WorkspaceFilePreviewHeaderProps) {
  const { t, i18n } = useTranslation('common');
  const { theme } = useTheme();
  const actualTheme = getActualTheme(theme);
  const hostId = useHostId();
  const [copyState, setCopyState] = useState<
    'idle' | 'copied' | 'requested' | 'error'
  >('idle');
  const openInEditor = useOpenInEditor(target.workspaceId);
  const { data: workspace } = useWorkspaceRecord(target.workspaceId);
  const fileName = content?.name || target.path.split('/').pop() || target.path;
  const repoName = content?.repo_name ?? target.repoId;
  const rawUrl = scopeWorkspaceFileRawUrl(content?.raw_url, hostId);

  const FileIcon = useMemo(
    () => getFileIcon(fileName, actualTheme),
    [actualTheme, fileName]
  );

  const handleCopyPath = async () => {
    try {
      const confirmed = await writeClipboardViaBridge(target.path);
      setCopyState(confirmed ? 'copied' : 'requested');
      window.setTimeout(() => setCopyState('idle'), 1500);
    } catch {
      setCopyState('error');
    }
  };

  const handleOpenEditor = () => {
    void openInEditor({ filePath: target.path, repoId: target.repoId });
  };

  return (
    <div className="sticky top-0 z-10 border-b border-border bg-primary">
      <div className="flex min-h-[48px] flex-wrap items-center gap-base px-base py-half">
        <div className="flex size-8 shrink-0 items-center justify-center rounded border border-border bg-secondary">
          <FileIcon className="size-5" />
        </div>
        <div className="min-w-0 flex-1 basis-[160px]">
          <div className="flex min-w-0 items-center gap-half">
            <div className="truncate text-base font-semibold text-high">
              {fileName}
            </div>
            <span className="shrink-0 rounded border border-border bg-secondary px-1.5 py-0.5 text-xs text-low">
              {repoName}
            </span>
          </div>
          <div className="mt-0.5 flex min-w-0 items-center gap-half text-xs text-low">
            <span className="min-w-0 truncate font-ibm-plex-mono">
              {target.path}
            </span>
            {content && (
              <>
                <span className="shrink-0">|</span>
                <span className="shrink-0">
                  {formatFileSize(
                    content.size_bytes,
                    i18n.resolvedLanguage || i18n.language
                  )}
                </span>
              </>
            )}
            {displayInfo && (
              <>
                <span className="shrink-0">|</span>
                <span className="shrink-0">{displayInfo.label}</span>
              </>
            )}
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-half">
          <HeaderIconButton
            label={
              copyState === 'requested'
                ? t('workspaceFiles.copyRequested')
                : copyState === 'error'
                  ? t('workspaceFiles.copyFailed')
                  : copyState === 'copied'
                    ? t('workspaceFiles.copied_path')
                    : t('workspaceFiles.copy_path')
            }
            onClick={() => void handleCopyPath()}
          >
            <Copy className="size-4" />
          </HeaderIconButton>
          {workspace?.usage === 'interactive' && (
            <HeaderIconButton
              label={t('workspaceFiles.open_in_editor')}
              onClick={handleOpenEditor}
            >
              <ExternalLink className="size-4" />
            </HeaderIconButton>
          )}
          <HeaderIconButton
            label={t('workspaceFiles.refresh_preview')}
            onClick={onRefresh}
            disabled={isRefreshing}
          >
            <RefreshCw
              className={cn('size-4', isRefreshing && 'animate-spin')}
            />
          </HeaderIconButton>
          <Tooltip
            content={
              rawUrl
                ? t('workspaceFiles.open_raw_file')
                : t('workspaceFiles.raw_file_unavailable')
            }
          >
            <a
              href={rawUrl ?? undefined}
              target="_blank"
              rel="noopener noreferrer"
              aria-label={t('workspaceFiles.open_raw_file')}
              className={cn(
                'flex size-8 items-center justify-center rounded border border-border bg-secondary text-low transition-colors hover:text-normal focus:outline-none focus:ring-1 focus:ring-brand',
                !rawUrl && 'pointer-events-none opacity-50'
              )}
            >
              <Download className="size-4" />
            </a>
          </Tooltip>
          {onClearTarget && (
            <HeaderIconButton
              label={t('workspaceFiles.close_preview')}
              onClick={onClearTarget}
            >
              <X className="size-4" />
            </HeaderIconButton>
          )}
        </div>
      </div>
      {isRefreshing && (
        <div className="flex h-6 items-center gap-half border-t border-border px-base text-xs text-low">
          <Loader2 className="size-3 animate-spin" />
          {t('workspaceFiles.refreshing_preview')}
        </div>
      )}
    </div>
  );
}

interface HeaderIconButtonProps {
  label: string;
  disabled?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}

function HeaderIconButton({
  label,
  disabled,
  onClick,
  children,
}: HeaderIconButtonProps) {
  return (
    <Tooltip content={label}>
      <button
        type="button"
        onClick={onClick}
        disabled={disabled}
        aria-label={label}
        className="flex size-8 items-center justify-center rounded border border-border bg-secondary text-low transition-colors hover:text-normal focus:outline-none focus:ring-1 focus:ring-brand disabled:cursor-not-allowed disabled:opacity-50"
      >
        {children}
      </button>
    </Tooltip>
  );
}
