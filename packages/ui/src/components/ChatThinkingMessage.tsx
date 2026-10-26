import type { ReactNode } from 'react';
import { ChatDotsIcon } from '@phosphor-icons/react';
import { useTranslation } from 'react-i18next';
import { cn } from '../lib/cn';
import { ActivityText } from './ActivityText';
import { ChatElapsedTime } from './ChatElapsedTime';

export interface ChatThinkingMessageRenderProps {
  content: string;
  workspaceId?: string;
  className?: string;
}

interface ChatThinkingMessageProps {
  content: string;
  className?: string;
  workspaceId?: string;
  startedAt?: string | null;
  endedAt?: string | null;
  active?: boolean;
  label?: string;
  provider?: string | null;
  partial?: boolean;
  renderMarkdown: (props: ChatThinkingMessageRenderProps) => ReactNode;
}

export function ChatThinkingMessage({
  content,
  className,
  workspaceId,
  startedAt,
  endedAt,
  active = !endedAt,
  label,
  provider,
  partial,
  renderMarkdown,
}: ChatThinkingMessageProps) {
  const { t } = useTranslation('common');
  if (!content.trim()) return null;

  return (
    <div
      className={cn('flex items-start gap-base text-sm text-low', className)}
    >
      <ChatDotsIcon className="shrink-0 size-icon-base pt-0.5" />
      <details className="min-w-0 flex-1">
        <summary className="cursor-pointer rounded-sm focus-visible:outline-brand">
          <span className="inline-flex max-w-full items-center gap-base">
            <ActivityText
              active={active}
              className="min-w-0 flex-1 truncate text-sm"
              title={label}
            >
              {content.split(/\r?\n/).find((line) => line.trim())}
            </ActivityText>
            {provider && (
              <span className="text-xs">
                {provider === 'codex'
                  ? 'Codex'
                  : provider === 'claude_code'
                    ? 'Claude Code'
                    : provider}
              </span>
            )}
            {partial && (
              <span className="text-xs">{t('conversation.partial')}</span>
            )}
            <ChatElapsedTime
              startedAt={startedAt}
              endedAt={endedAt}
              active={active}
            />
          </span>
        </summary>
        {renderMarkdown({
          content,
          workspaceId: workspaceId,
          className: 'text-sm',
        })}
      </details>
    </div>
  );
}
