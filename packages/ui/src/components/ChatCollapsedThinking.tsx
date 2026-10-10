import type { ReactNode } from 'react';
import { ChatDotsIcon } from '@phosphor-icons/react';
import { useTranslation } from 'react-i18next';
import { cn } from '../lib/cn';
import { ActivityText } from './ActivityText';
import { ChatElapsedTime } from './ChatElapsedTime';

export interface ThinkingEntry {
  content: string;
  expansionKey: string;
  startedAt?: string | null;
  endedAt?: string | null;
}

export interface ChatCollapsedThinkingRenderProps {
  content: string;
  workspaceId?: string;
  className?: string;
}

interface ChatCollapsedThinkingProps {
  entries: ThinkingEntry[];
  className?: string;
  workspaceId?: string;
  startedAt?: string | null;
  endedAt?: string | null;
  renderMarkdown: (props: ChatCollapsedThinkingRenderProps) => ReactNode;
}

/** Displays completed thinking content inline with its timing. */
export function ChatCollapsedThinking({
  entries,
  className,
  workspaceId,
  startedAt,
  endedAt,
  renderMarkdown,
}: ChatCollapsedThinkingProps) {
  const { t } = useTranslation('common');

  if (entries.length === 0) return null;

  return (
    <div className={cn('flex flex-col', className)}>
      <div
        className="flex min-w-0 items-center gap-base py-0.5 text-sm text-low"
        data-scroll-anchor-target=""
      >
        <ChatDotsIcon className="size-icon-base shrink-0" />
        <ActivityText active={false} className="min-w-0 flex-1 truncate">
          {t('conversation.processed')}
        </ActivityText>
        <ChatElapsedTime startedAt={startedAt} endedAt={endedAt} />
      </div>

      <div className="ml-6 mt-1 flex min-w-0 flex-col gap-base">
        {entries.map((entry) => (
          <div key={entry.expansionKey} className="text-sm text-low">
            {renderMarkdown({
              content: entry.content,
              workspaceId: workspaceId,
              className: 'text-sm',
            })}
          </div>
        ))}
      </div>
    </div>
  );
}
