import { useTranslation } from 'react-i18next';
import { useMemo, useState } from 'react';
import type { WorkspaceFilePreviewTarget } from '@/features/workspace-files';
import { ArrowLeft, GitCompare } from 'lucide-react';
import { Button } from '@vibe/ui/components/Button';
import { cn } from '@/shared/lib/utils';
import { useChangesViewActions } from '@/shared/hooks/useChangesView';
import {
  useWorkspaceFilesSelection,
  WorkspaceFileBrowserPanel,
  WorkspaceFilePreviewPanel,
} from '@/features/workspace-files';

interface WorkspaceFilesSurfaceContainerProps {
  workspaceId: string;
  className?: string;
  mobile?: boolean;
}

export function WorkspaceFilesSurfaceContainer({
  workspaceId,
  className,
  mobile = false,
}: WorkspaceFilesSurfaceContainerProps) {
  const { t } = useTranslation('common');
  const { target, openTarget, clearTarget } =
    useWorkspaceFilesSelection(workspaceId);
  const { findMatchingDiffPath, viewFileInChanges } = useChangesViewActions();
  // Dismiss only the current selection. A new open request (including the
  // same file from chat) must reveal the preview again.
  const [dismissedTarget, setDismissedTarget] =
    useState<WorkspaceFilePreviewTarget | null>(null);

  const matchingDiffPath = useMemo(() => {
    if (!target) return null;
    return findMatchingDiffPath(target.path);
  }, [findMatchingDiffPath, target]);

  const handleViewChanges = () => {
    if (!matchingDiffPath) return;
    viewFileInChanges(matchingDiffPath);
  };

  if (mobile) {
    const showPreview = target && target !== dismissedTarget;

    return (
      <div className={cn('flex h-full min-h-0 flex-col bg-primary', className)}>
        {showPreview ? (
          <>
            <div className="flex shrink-0 items-center gap-half border-b border-border bg-secondary px-base py-half">
              <Button
                type="button"
                size="xs"
                variant="secondary"
                onClick={() => setDismissedTarget(target)}
              >
                <ArrowLeft className="mr-half size-3" />
                {t('fileTree.title')}
              </Button>
              <div className="min-w-0 flex-1 truncate text-sm text-low">
                {target.path}
              </div>
              {matchingDiffPath && (
                <Button
                  type="button"
                  size="xs"
                  variant="secondary"
                  onClick={handleViewChanges}
                >
                  <GitCompare className="mr-half size-3" />
                  {t('sections.changes')}
                </Button>
              )}
            </div>
            <WorkspaceFilePreviewPanel
              target={target}
              className="min-h-0 flex-1"
              onClearTarget={() => {
                clearTarget();
                setDismissedTarget(target);
              }}
            />
          </>
        ) : (
          <WorkspaceFileBrowserPanel
            workspaceId={workspaceId}
            selectedTarget={target}
            source="file-tree"
            onSelectFile={(nextTarget) => {
              openTarget(nextTarget);
            }}
            className="border-r-0"
          />
        )}
      </div>
    );
  }

  return (
    <div className={cn('flex h-full min-h-0 flex-col bg-primary', className)}>
      {target && matchingDiffPath && (
        <div className="flex shrink-0 items-center justify-end border-b border-border bg-secondary px-base py-half">
          <Button
            type="button"
            size="xs"
            variant="secondary"
            onClick={handleViewChanges}
          >
            <GitCompare className="mr-half size-3" />
            {t('sections.viewChanges')}
          </Button>
        </div>
      )}
      <WorkspaceFilePreviewPanel
        target={target}
        className="min-h-0 flex-1"
        onClearTarget={clearTarget}
      />
    </div>
  );
}
