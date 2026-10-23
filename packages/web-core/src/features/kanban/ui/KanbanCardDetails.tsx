import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { WorkspaceWithStats } from '@vibe/ui/components/IssueWorkspaceCard';

// Reuse the already loaded workspace summaries. Flags are independent: a
// pending approval must remain visible even if another workspace is running.
export function KanbanCardDetails({
  count,
  workspaces,
  description,
  children,
}: {
  count: number;
  workspaces: WorkspaceWithStats[];
  description?: string | null;
  children: ReactNode;
}) {
  const { t } = useTranslation('common');
  const flags = {
    approval: workspaces.some(
      (w) =>
        w.hasPendingApproval || w.latestProcessStatus === 'awaiting_approval'
    ),
    input: workspaces.some((w) => w.latestProcessStatus === 'awaiting_input'),
    running: workspaces.some((w) => w.isRunning),
    failed: workspaces.some((w) =>
      ['failed', 'crashed', 'audit_failed'].includes(
        w.latestProcessStatus ?? ''
      )
    ),
    unseen: workspaces.some((w) => w.hasUnseenActivity),
    devServer: workspaces.some((w) => w.hasRunningDevServer),
  };
  return (
    <div className="mt-half min-w-0">
      <div
        className="flex flex-wrap gap-half text-sm text-brand"
        aria-live="polite"
      >
        {Object.entries(flags)
          .filter(([, value]) => value)
          .map(([key]) => (
            <span
              key={key}
              className={key === 'failed' ? 'text-error' : undefined}
            >
              {t(`kanban.compact.${key}`)}
            </span>
          ))}
      </div>
      <details onClick={(event) => event.stopPropagation()}>
        <summary className="cursor-pointer text-sm text-low rounded-sm focus-visible:ring-2 focus-visible:ring-brand">
          {t('kanban.compact.workspaces', { count })}
        </summary>
        {description && (
          <p className="text-sm text-normal whitespace-pre-wrap break-words mt-half">
            {description}
          </p>
        )}
        {children}
      </details>
    </div>
  );
}
