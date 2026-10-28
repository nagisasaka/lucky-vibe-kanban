import { cn } from '../lib/cn';
import { useTranslation } from 'react-i18next';
import type { AgentRunStatus } from 'shared/types';
import {
  GitPullRequestIcon,
  DotsThreeIcon,
  LinkBreakIcon,
  TrashIcon,
  PlayIcon,
  HandIcon,
  TriangleIcon,
  CircleIcon,
} from '@phosphor-icons/react';
import { UserAvatar, type UserAvatarUser } from './UserAvatar';
import { RunningDots } from './RunningDots';
import { Tooltip } from './Tooltip';
import { ChatElapsedTime } from './ChatElapsedTime';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from './DropdownMenu';

export interface WorkspacePr {
  number: number;
  url: string;
  status: 'open' | 'merged' | 'closed';
}

export interface WorkspaceWithStats {
  id: string;
  localWorkspaceId: string | null;
  name: string | null;
  archived: boolean;
  filesChanged?: number;
  linesAdded?: number;
  linesRemoved?: number;
  prs: WorkspacePr[];
  owner: UserAvatarUser | null;
  updatedAt: string;
  isOwnedByCurrentUser: boolean;
  // Shared self-hosted access is independent from historical ownership.
  canManageWorkspace?: boolean;
  isRunning?: boolean;
  hasPendingApproval?: boolean;
  hasRunningDevServer?: boolean;
  hasUnseenActivity?: boolean;
  latestProcessStartedAt?: string;
  latestProcessCompletedAt?: string;
  latestProcessStatus?: AgentRunStatus;
}

export interface IssueWorkspaceCardProps {
  workspace: WorkspaceWithStats;
  onClick?: () => void;
  onUnlink?: () => void;
  onDelete?: () => void;
  showOwner?: boolean;
  showStatusBadge?: boolean;
  showNoPrText?: boolean;
  className?: string;
}

export interface IssueWorkspaceCreateCardProps {
  onClick?: () => void;
  className?: string;
  shouldAnimateCreateButton?: boolean;
}

interface IssueWorkspaceCardContainerProps {
  highlightDevServer?: boolean;
  onClick?: () => void;
  className?: string;
  children: React.ReactNode;
}

function IssueWorkspaceCardContainer({
  onClick,
  className,
  children,
  highlightDevServer = false,
}: IssueWorkspaceCardContainerProps) {
  return (
    <div
      className={cn(
        'flex flex-col gap-half p-base rounded-sm transition-all duration-150',
        highlightDevServer ? 'bg-[hsl(var(--info)/0.12)]' : 'bg-panel',
        onClick &&
          (highlightDevServer
            ? 'cursor-pointer hover:bg-[hsl(var(--info)/0.18)]'
            : 'cursor-pointer hover:bg-secondary/70'),
        className
      )}
      onClick={
        onClick
          ? (e) => {
              e.stopPropagation();
              onClick();
            }
          : undefined
      }
      role={onClick ? 'button' : undefined}
      tabIndex={onClick ? 0 : undefined}
      onKeyDown={
        onClick
          ? (e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                e.stopPropagation();
                onClick();
              }
            }
          : undefined
      }
    >
      {children}
    </div>
  );
}

export function IssueWorkspaceCard({
  workspace,
  onClick,
  onUnlink,
  onDelete,
  showOwner = true,
  showStatusBadge = true,
  showNoPrText = true,
  className,
}: IssueWorkspaceCardProps) {
  const { t } = useTranslation('common');
  const isRunning = workspace.isRunning ?? false;
  const hasPendingApproval =
    workspace.hasPendingApproval ||
    workspace.latestProcessStatus === 'awaiting_approval';
  const needsInput = workspace.latestProcessStatus === 'awaiting_input';
  const hasRunningDevServer = workspace.hasRunningDevServer ?? false;
  const hasUnseenActivity = workspace.hasUnseenActivity ?? false;
  const isPreparing = ['pending', 'starting'].includes(
    workspace.latestProcessStatus ?? ''
  );
  const isFailed =
    workspace.latestProcessStatus === 'failed' ||
    workspace.latestProcessStatus === 'crashed' ||
    workspace.latestProcessStatus === 'audit_failed';

  const activity = hasPendingApproval
    ? 'approval'
    : needsInput
      ? 'input'
      : isRunning && !isPreparing
        ? 'running'
        : isFailed
          ? 'failed'
          : null;
  const showReadState =
    !isPreparing &&
    !isRunning &&
    !hasPendingApproval &&
    !needsInput &&
    (hasUnseenActivity ||
      isFailed ||
      workspace.latestProcessStatus === 'succeeded' ||
      workspace.latestProcessStatus === 'cancelled');
  return (
    <IssueWorkspaceCardContainer
      onClick={onClick}
      highlightDevServer={hasRunningDevServer}
      className={className}
    >
      {/* Row 1: Status badge + Name (left), Owner avatar + menu (right) */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-half min-w-0">
          {showStatusBadge && (
            <span
              className={cn(
                'px-1.5 py-0.5 rounded text-xs font-medium shrink-0',
                workspace.archived
                  ? 'bg-secondary text-low'
                  : 'bg-success/10 text-success'
              )}
            >
              {workspace.archived
                ? t('workspaces.archived')
                : t('workspaces.active')}
            </span>
          )}
          {workspace.name && (
            <span className="text-sm text-high truncate">{workspace.name}</span>
          )}
        </div>

        <div className="flex items-center gap-half">
          {showOwner && workspace.owner && (
            <UserAvatar
              user={workspace.owner}
              className="h-5 w-5 text-[10px] border-2 border-panel"
            />
          )}
          {(onUnlink || onDelete) && (
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <button
                  onClick={(e) => e.stopPropagation()}
                  className="p-0.5 rounded hover:bg-secondary transition-colors"
                  aria-label={t('workspaces.more')}
                >
                  <DotsThreeIcon
                    className="size-icon-xs text-low"
                    weight="bold"
                  />
                </button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                {onUnlink && (
                  <DropdownMenuItem
                    onClick={(e) => {
                      e.stopPropagation();
                      onUnlink();
                    }}
                  >
                    <LinkBreakIcon className="size-icon-xs" />
                    {t('workspaces.unlinkFromIssue')}
                  </DropdownMenuItem>
                )}
                {onDelete && (
                  <DropdownMenuItem
                    onClick={(e) => {
                      e.stopPropagation();
                      onDelete();
                    }}
                    className="text-destructive focus:text-destructive"
                  >
                    <TrashIcon className="size-icon-xs" />
                    {t('workspaces.deleteWorkspace')}
                  </DropdownMenuItem>
                )}
              </DropdownMenuContent>
            </DropdownMenu>
          )}
        </div>
      </div>

      {/* Row 2: Live status + stats (left), PR buttons (right) */}
      <div className="flex flex-wrap items-center justify-between gap-half min-w-0">
        <div className="flex items-center flex-wrap gap-half text-sm text-low min-w-0 flex-1">
          <div className="flex items-center gap-half shrink-0">
            {hasRunningDevServer && (
              <Tooltip content={t('kanban.compact.devServer')}>
                <span role="img" aria-label={t('kanban.compact.devServer')}>
                  <PlayIcon
                    aria-hidden="true"
                    className="size-icon-xs text-brand shrink-0"
                    weight="fill"
                  />
                </span>
              </Tooltip>
            )}

            {activity && (
              <Tooltip content={t(`kanban.compact.${activity}`)}>
                <span role="img" aria-label={t(`kanban.compact.${activity}`)}>
                  {hasPendingApproval || needsInput ? (
                    <HandIcon
                      aria-hidden="true"
                      className="size-icon-xs text-brand shrink-0"
                      weight="fill"
                    />
                  ) : isRunning ? (
                    <RunningDots />
                  ) : (
                    <TriangleIcon
                      aria-hidden="true"
                      className="size-icon-xs text-error shrink-0"
                      weight="fill"
                    />
                  )}
                </span>
              </Tooltip>
            )}
            {isPreparing && <span className="text-low">Preparing…</span>}
            {showReadState && (
              <Tooltip
                content={t(
                  `kanban.compact.${hasUnseenActivity ? 'unseen' : 'read'}`
                )}
              >
                <span
                  role="img"
                  aria-label={t(
                    `kanban.compact.${hasUnseenActivity ? 'unseen' : 'read'}`
                  )}
                >
                  <CircleIcon
                    aria-hidden="true"
                    className="size-icon-xs text-brand shrink-0"
                    weight={hasUnseenActivity ? 'fill' : 'regular'}
                  />
                </span>
              </Tooltip>
            )}
            {!isPreparing && (
              <ChatElapsedTime
                unknownLabel="—"
                startedAt={workspace.latestProcessStartedAt}
                endedAt={workspace.latestProcessCompletedAt}
                active={
                  !!workspace.latestProcessStatus &&
                  [
                    'running',
                    'awaiting_input',
                    'awaiting_approval',
                    'cancelling',
                  ].includes(workspace.latestProcessStatus)
                }
              />
            )}
          </div>

          <span className="whitespace-nowrap shrink-0">
            {workspace.filesChanged == null
              ? t('workspaces.filesChangedUnknown')
              : t('workspaces.filesChanged', { count: workspace.filesChanged })}
          </span>
          <Tooltip content={t('workspaces.textLineCounts')}>
            <span className="flex items-center gap-base" tabIndex={0}>
              <span className="text-success whitespace-nowrap shrink-0">
                +{workspace.linesAdded ?? '—'}
              </span>
              <span className="text-error whitespace-nowrap shrink-0">
                -{workspace.linesRemoved ?? '—'}
              </span>
            </span>
          </Tooltip>
        </div>

        <div className="hidden sm:flex flex-wrap items-center gap-half min-w-0">
          {workspace.prs.length > 0 ? (
            workspace.prs.map((pr) => (
              <a
                key={pr.number}
                href={pr.url}
                target="_blank"
                rel="noopener noreferrer"
                onClick={(e) => e.stopPropagation()}
                className={cn(
                  'flex items-center gap-half px-1.5 py-0.5 rounded text-xs font-medium transition-colors',
                  pr.status === 'merged'
                    ? 'bg-merged/10 text-merged hover:bg-merged/20'
                    : pr.status === 'closed'
                      ? 'bg-error/10 text-error hover:bg-error/20'
                      : 'bg-success/10 text-success hover:bg-success/20'
                )}
              >
                <GitPullRequestIcon className="size-icon-2xs" weight="bold" />
                <span>#{pr.number}</span>
              </a>
            ))
          ) : showNoPrText ? (
            <span className="text-xs text-low whitespace-nowrap">
              {t('kanban.noPrCreated')}
            </span>
          ) : null}
        </div>
      </div>
    </IssueWorkspaceCardContainer>
  );
}

export function IssueWorkspaceCreateCard({
  onClick,
  className,
  shouldAnimateCreateButton = false,
}: IssueWorkspaceCreateCardProps) {
  const { t } = useTranslation('common');

  return (
    <IssueWorkspaceCardContainer
      className={cn('border border-dashed border-border', className)}
    >
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-half min-w-0">
          <span className="px-1.5 py-0.5 rounded text-xs font-medium shrink-0 bg-secondary text-low">
            {t('workspaces.draft')}
          </span>
        </div>
      </div>

      <div className="flex items-center justify-between gap-base">
        <span className="text-sm text-low truncate">
          {t('workspaces.newWorkspace')}
        </span>
        <button
          type="button"
          onClick={onClick}
          disabled={!onClick}
          className={cn(
            'shrink-0 rounded-sm px-base py-half text-cta h-cta flex items-center bg-brand-secondary text-on-brand hover:bg-brand-hover transition-colors disabled:opacity-50 disabled:cursor-not-allowed',
            shouldAnimateCreateButton && 'create-issue-attention'
          )}
        >
          {t('buttons.create')}
        </button>
      </div>
    </IssueWorkspaceCardContainer>
  );
}
