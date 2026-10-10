import { ListMagnifyingGlassIcon } from '@phosphor-icons/react';
import { useTranslation } from 'react-i18next';
import { cn } from '../lib/cn';
import { ActivityText } from './ActivityText';
import { ChatElapsedTime } from './ChatElapsedTime';
import { ToolStatusDot, type ToolStatusLike } from './ToolStatusDot';

export interface AggregatedEntry {
  summary: string;
  status?: ToolStatusLike;
  active?: boolean;
  expansionKey: string;
  content?: string;
  command?: string;
  startedAt?: string | null;
  endedAt?: string | null;
}

interface ChatAggregatedToolEntriesProps {
  entries: AggregatedEntry[];
  summary?: string;
  detail?: string;
  label?: string;
  unit?: string;
  icon?: React.ElementType;
  className?: string;
  startedAt?: string | null;
  endedAt?: string | null;
}

function isEntryActive(entry: AggregatedEntry) {
  return entry.active ?? isActiveStatus(entry.status);
}

function getAggregateLifecycleStatus(entries: AggregatedEntry[]) {
  const activeEntry = entries.find(isEntryActive);
  return activeEntry?.status ?? { status: 'success' };
}

function getEntryDetails(entry: AggregatedEntry) {
  const content = entry.content ?? '';
  if (
    !content.trim() &&
    (!entry.command?.trim() || entry.command.trim() === entry.summary)
  )
    return null;

  return {
    command: entry.command?.trim() || null,
    content,
  };
}

function ToolEntryDetails({ entry }: { entry: AggregatedEntry }) {
  const details = getEntryDetails(entry);
  if (!details) return null;

  return (
    <div className="mt-half overflow-hidden rounded-sm border border-border/70 bg-panel/70">
      {details.command && details.command !== entry.summary && (
        <pre className="overflow-x-auto border-b border-border/70 px-base py-half font-mono text-xs leading-relaxed text-normal whitespace-pre-wrap break-words">
          {details.command}
        </pre>
      )}
      <pre className="overflow-x-auto px-base py-half font-mono text-xs leading-relaxed text-low whitespace-pre-wrap break-words">
        {details.content}
      </pre>
    </div>
  );
}

function getFallbackSummary({
  entries,
  label,
  unit,
  fallbackSummary,
}: {
  entries: AggregatedEntry[];
  label?: string;
  unit?: string;
  fallbackSummary: string;
}) {
  if (!label || !unit) return fallbackSummary;
  return `${label} ${entries.length} ${unit}`;
}

function isActiveStatus(status?: ToolStatusLike) {
  return status?.status === 'created' || status?.status === 'pending_approval';
}

export function ChatAggregatedToolEntries({
  entries,
  summary,
  detail,
  label,
  unit,
  icon: Icon = ListMagnifyingGlassIcon,
  className,
  startedAt,
  endedAt,
}: ChatAggregatedToolEntriesProps) {
  const { t } = useTranslation('tasks');

  if (entries.length === 0) return null;

  const isRunning = entries.some(isEntryActive);
  const aggregateStatus = getAggregateLifecycleStatus(entries);
  const headerSummary =
    summary ??
    getFallbackSummary({
      entries,
      label,
      unit,
      fallbackSummary: t('conversation.aggregated.operationCount', {
        count: entries.length,
      }),
    });

  return (
    <div className={cn('flex min-w-0 flex-col gap-half', className)}>
      <div
        className="flex min-w-0 items-center gap-base text-sm text-low"
        data-scroll-anchor-target=""
      >
        <span className="relative shrink-0 pt-0.5">
          <Icon className="size-icon-base" />
          <ToolStatusDot
            status={aggregateStatus}
            active={isRunning}
            className="absolute -bottom-0.5 -left-0.5"
          />
        </span>
        <ActivityText active={isRunning} className="min-w-0 flex-1">
          {headerSummary}
        </ActivityText>
        {detail && <span className="text-xs text-low">{detail}</span>}
        <ChatElapsedTime
          startedAt={startedAt}
          endedAt={endedAt}
          active={isRunning}
        />
      </div>
      <div className="ml-6 flex min-w-0 flex-col gap-base">
        {entries.map((entry) => (
          <div key={entry.expansionKey} className="min-w-0">
            {(entries.length > 1 || entry.summary !== headerSummary) && (
              <div className="flex min-w-0 items-start gap-base text-sm text-low">
                {entry.status && (
                  <ToolStatusDot
                    status={entry.status}
                    active={isEntryActive(entry)}
                  />
                )}
                <span className="min-w-0 flex-1 whitespace-pre-wrap break-words">
                  {entry.summary}
                </span>
                <ChatElapsedTime
                  startedAt={entry.startedAt}
                  endedAt={entry.endedAt}
                  active={isEntryActive(entry)}
                />
              </div>
            )}
            <ToolEntryDetails entry={entry} />
          </div>
        ))}
      </div>
    </div>
  );
}
