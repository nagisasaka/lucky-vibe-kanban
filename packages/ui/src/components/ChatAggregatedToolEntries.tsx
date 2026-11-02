import { CaretDownIcon, ListMagnifyingGlassIcon } from '@phosphor-icons/react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cn } from '../lib/cn';
import { ChatCode } from './ChatCode';
import { ActivityText } from './ActivityText';
import { ChatElapsedTime } from './ChatElapsedTime';
import { ToolStatusDot, type ToolStatusLike } from './ToolStatusDot';
import {
  aggregateToolStatus,
  isToolActive,
  toolDisplayStatus,
} from '../lib/tool-lifecycle';
import {
  toolOutputPreview,
  toolCommandHeading,
} from '../lib/tool-output-preview';

export interface AggregatedEntry {
  summary: string;
  status?: ToolStatusLike;
  active?: boolean;
  expansionKey: string;
  content?: string;
  command?: string;
  startedAt?: string | null;
  endedAt?: string | null;
  durationMs?: number | null;
  outputState?: 'pending' | 'partial' | 'complete' | 'unavailable';
  exitCode?: number | null;
  cwd?: string | null;
  language?: string;
  input?: string;
}

interface ChatAggregatedToolEntriesProps {
  entries: AggregatedEntry[];
  expanded: boolean;
  isHovered: boolean;
  onToggle: () => void;
  onHoverChange: (hovered: boolean) => void;
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
  return isToolActive(entry);
}

function getEntryDetails(entry: AggregatedEntry) {
  const content = entry.content ?? '';
  if (!content && entry.command === undefined && !entry.outputState)
    return null;

  return {
    command: entry.command ?? null,
    content,
  };
}

function CopySource({ value, label }: { value: string; label: string }) {
  const { t } = useTranslation('tasks');
  const [result, setResult] = useState<'copied' | 'copyFailed' | null>(null);
  useEffect(() => setResult(null), [value]);
  return (
    <span className="inline-flex items-center gap-half">
      <button
        type="button"
        className="rounded-sm text-xs underline focus-visible:outline-brand"
        onClick={async () => {
          try {
            await navigator.clipboard.writeText(value);
            setResult('copied');
          } catch {
            setResult('copyFailed');
          }
        }}
      >
        {label}
      </button>
      {result && (
        <span role="status" className="text-xs">
          {t(`execution.${result}`)}
        </span>
      )}
    </span>
  );
}

function ToolEntryPreview({
  entry,
  includeHeading = false,
}: {
  entry: AggregatedEntry;
  includeHeading?: boolean;
}) {
  const { t } = useTranslation('tasks');
  if (entry.command === undefined && !entry.outputState) return null;
  const preview = toolOutputPreview(
    entry.content ?? '',
    entry.status?.status === 'failed'
  );
  const state =
    entry.outputState ?? (isEntryActive(entry) ? 'pending' : 'unavailable');
  return (
    <div className="ml-6 mt-half min-w-0 text-xs text-low">
      {includeHeading && (
        <div className="truncate font-ibm-plex-mono">
          {entry.command !== undefined
            ? toolCommandHeading(entry.command)
            : entry.summary}
        </div>
      )}
      {preview.text ? (
        <pre className="overflow-hidden whitespace-pre-wrap break-words font-ibm-plex-mono leading-relaxed">
          {preview.text}
        </pre>
      ) : (
        <span>
          {t(
            state === 'complete'
              ? 'execution.noOutput'
              : state === 'pending'
                ? 'execution.waitingOutput'
                : 'execution.outputUnavailable'
          )}
        </span>
      )}
      {(preview.clipped || state === 'partial') && (
        <span className="block">
          {t(state === 'partial' ? 'execution.partial' : 'execution.preview')}
        </span>
      )}
    </div>
  );
}

function EntryStatus({ entry }: { entry: AggregatedEntry }) {
  const { t } = useTranslation('tasks');
  const status = toolDisplayStatus(entry);
  return (
    <span
      className={cn(
        'shrink-0 text-xs',
        ['failed', 'denied', 'timed_out'].includes(status)
          ? 'text-error'
          : 'text-low'
      )}
    >
      {t(`execution.status.${status}`)}
      {entry.exitCode != null && entry.exitCode !== 0
        ? ` · ${t('conversation.script.exitCode', { code: entry.exitCode })}`
        : ''}
    </span>
  );
}

function ToolEntryDetails({ entry }: { entry: AggregatedEntry }) {
  const { t, i18n } = useTranslation('tasks');
  const locale = i18n.resolvedLanguage || i18n.language;
  const details = getEntryDetails(entry);
  if (!details) return null;

  return (
    <div className="mx-base mb-half overflow-hidden rounded-sm border border-border/70 bg-panel/70">
      <div className="flex flex-wrap items-center gap-base px-base py-half text-xs text-low">
        <span>{t('execution.retainedDetails')}</span>
        {details.command !== null && (
          <CopySource
            value={details.command}
            label={t('execution.copyCommand')}
          />
        )}
        {details.content.length > 0 && (
          <CopySource
            value={details.content}
            label={t('execution.copyOutput')}
          />
        )}
      </div>
      {entry.cwd && (
        <div className="break-words px-base py-half font-ibm-plex-mono text-xs text-low">
          {t('execution.workingDirectory', { path: entry.cwd })}
        </div>
      )}
      {entry.input && (
        <details
          className="px-base py-half text-xs text-low"
          open={entry.command === undefined}
        >
          <summary className="cursor-pointer rounded-sm focus-visible:outline-brand">
            {t('execution.input')}
          </summary>
          <pre className="max-h-56 overflow-auto whitespace-pre-wrap break-words font-ibm-plex-mono">
            <ChatCode code={entry.input} language="json" />
          </pre>
        </details>
      )}
      {details.command !== null && (
        <pre className="overflow-x-auto border-b border-border/70 px-base py-half font-mono text-xs leading-relaxed text-normal whitespace-pre-wrap break-words">
          <ChatCode code={details.command} language="bash" />
        </pre>
      )}
      <pre className="max-h-56 overflow-auto px-base py-half font-mono text-xs leading-relaxed text-low whitespace-pre-wrap break-words">
        <ChatCode code={details.content} language={entry.language} />
      </pre>
      {entry.outputState === 'complete' && details.content.length === 0 && (
        <div className="px-base py-half text-xs text-low">
          {t('execution.noOutput')}
        </div>
      )}
      {entry.outputState !== 'complete' && (
        <div className="px-base py-half text-xs text-low">
          {t(
            entry.outputState === 'partial'
              ? 'execution.partial'
              : entry.outputState === 'pending'
                ? 'execution.waitingOutput'
                : 'execution.outputUnavailable'
          )}
        </div>
      )}
      <div className="flex flex-wrap gap-base px-base py-half text-xs text-low">
        {entry.exitCode != null && (
          <span>
            {t('conversation.script.exitCode', { code: entry.exitCode })}
          </span>
        )}
        {entry.endedAt && (
          <time dateTime={entry.endedAt}>
            {t('execution.completedAt', {
              time: new Date(entry.endedAt).toLocaleString(locale),
            })}
          </time>
        )}
        {entry.durationMs != null && (
          <span>
            {t('execution.providerDuration', {
              seconds: new Intl.NumberFormat(locale, {
                maximumFractionDigits: 20,
              }).format(entry.durationMs / 1000),
            })}
          </span>
        )}
      </div>
    </div>
  );
}

function getFallbackSummary({
  label,
  unit,
  fallbackSummary,
  formattedCount,
}: {
  label?: string;
  unit?: string;
  fallbackSummary: string;
  formattedCount: string;
}) {
  if (!label || !unit) return fallbackSummary;
  return `${label} ${formattedCount} ${unit}`;
}

export function ChatAggregatedToolEntries({
  entries,
  expanded,
  isHovered,
  onToggle,
  onHoverChange,
  summary,
  detail,
  label,
  unit,
  icon: Icon = ListMagnifyingGlassIcon,
  className,
  startedAt,
  endedAt,
}: ChatAggregatedToolEntriesProps) {
  const { t, i18n } = useTranslation('tasks');
  const number = new Intl.NumberFormat(i18n.resolvedLanguage || i18n.language);
  const [expandedEntryKey, setExpandedEntryKey] = useState<string | null>(null);

  if (entries.length === 0) return null;

  const isRunning = entries.some(isEntryActive);
  const aggregateStatus = aggregateToolStatus(entries);
  const headerSummary =
    entries.length === 1
      ? entries[0].summary
      : (summary ??
        getFallbackSummary({
          formattedCount: number.format(entries.length),
          label,
          unit,
          fallbackSummary: t('conversation.aggregated.operationCount', {
            count: entries.length,
            formattedCount: number.format(entries.length),
          }),
        }));

  if (entries.length === 1) {
    const entry = entries[0];
    const hasDetails = Boolean(getEntryDetails(entry));
    const entryExpanded = expanded;
    return (
      <div className={cn('flex flex-col', className)}>
        <button
          type="button"
          className={cn(
            'flex min-w-0 items-center gap-base text-left text-sm text-low transition-colors',
            hasDetails && 'cursor-pointer hover:text-normal'
          )}
          onClick={onToggle}
          disabled={!hasDetails}
          aria-expanded={hasDetails ? entryExpanded : undefined}
        >
          <span className="relative shrink-0 pt-0.5">
            <Icon className="size-icon-base" />
            {entry.status && (
              <ToolStatusDot
                status={entry.status}
                active={isEntryActive(entry)}
                className="absolute -bottom-0.5 -left-0.5"
              />
            )}
          </span>
          <ActivityText active={isRunning} className="min-w-0 flex-1 truncate">
            {entry.command !== undefined && entry.command === headerSummary ? (
              <ChatCode
                code={toolCommandHeading(entry.command)}
                language="bash"
              />
            ) : (
              headerSummary
            )}
          </ActivityText>
          <EntryStatus entry={entry} />
          {detail && (
            <span className="hidden shrink-0 text-xs text-low sm:inline">
              {detail}
            </span>
          )}
          <ChatElapsedTime
            startedAt={entry.startedAt ?? startedAt}
            endedAt={entry.endedAt ?? endedAt}
            durationMs={entry.durationMs}
            active={isEntryActive(entry)}
          />
          {hasDetails && (
            <CaretDownIcon
              className={cn(
                'size-icon-sm shrink-0 transition-transform duration-150',
                !entryExpanded && '-rotate-90'
              )}
            />
          )}
        </button>
        {!entryExpanded && <ToolEntryPreview entry={entry} />}
        {entryExpanded && <ToolEntryDetails entry={entry} />}
      </div>
    );
  }

  return (
    <div className={cn('flex flex-col', className)}>
      <button
        type="button"
        className="group -mx-half flex min-w-0 items-center gap-base rounded-sm px-half py-0.5 text-left text-sm text-low transition-colors hover:bg-muted/30 hover:text-normal"
        onClick={onToggle}
        onMouseEnter={() => onHoverChange(true)}
        onMouseLeave={() => onHoverChange(false)}
        aria-expanded={expanded}
        data-scroll-anchor-target=""
      >
        <span className="relative shrink-0 pt-0.5">
          {isHovered ? (
            <CaretDownIcon
              className={cn(
                'size-icon-base transition-transform duration-150',
                !expanded && '-rotate-90'
              )}
            />
          ) : (
            <Icon className="size-icon-base" />
          )}
          {aggregateStatus && (
            <ToolStatusDot
              status={aggregateStatus}
              active={isRunning}
              className="absolute -bottom-0.5 -left-0.5"
            />
          )}
        </span>

        <ActivityText active={isRunning} className="min-w-0 flex-1 truncate">
          {headerSummary}
        </ActivityText>
        <span className="max-w-[50%] shrink whitespace-normal text-right text-xs text-low">
          {Array.from(new Set(entries.map(toolDisplayStatus)))
            .map((status) =>
              t('execution.statusCount', {
                status: t(`execution.status.${status}`),
                formattedCount: number.format(
                  entries.filter((entry) => toolDisplayStatus(entry) === status)
                    .length
                ),
              })
            )
            .join(' · ')}
        </span>
        {detail && (
          <span className="hidden shrink-0 text-xs text-low sm:inline">
            {detail}
          </span>
        )}
        <ChatElapsedTime
          startedAt={startedAt}
          endedAt={endedAt}
          active={isRunning}
        />
      </button>

      {!expanded &&
        entries.map((entry) => (
          <ToolEntryPreview
            key={entry.expansionKey}
            entry={entry}
            includeHeading
          />
        ))}

      {expanded && (
        <div className="ml-6 mt-1 max-h-72 overflow-y-auto rounded-md border border-border bg-muted/10 p-half">
          {entries.map((entry) => {
            const hasDetails = Boolean(getEntryDetails(entry));
            const entryExpanded = expandedEntryKey === entry.expansionKey;

            return (
              <div key={entry.expansionKey}>
                <button
                  type="button"
                  className={cn(
                    'group/item flex w-full min-w-0 items-center gap-base rounded-sm px-base py-half text-left text-sm text-low transition-colors',
                    hasDetails &&
                      'cursor-pointer hover:bg-muted/30 hover:text-normal'
                  )}
                  onClick={() =>
                    setExpandedEntryKey(
                      entryExpanded ? null : entry.expansionKey
                    )
                  }
                  disabled={!hasDetails}
                  aria-expanded={hasDetails ? entryExpanded : undefined}
                >
                  <span className="relative shrink-0 pt-0.5">
                    <Icon className="size-icon-base" />
                    {entry.status && (
                      <ToolStatusDot
                        status={entry.status}
                        active={isEntryActive(entry)}
                        className="absolute -bottom-0.5 -left-0.5"
                      />
                    )}
                  </span>
                  <span className="flex min-w-0 flex-1 items-center gap-base">
                    <span
                      className={cn(
                        'min-w-0 flex-1 truncate',
                        entry.command && 'font-mono text-xs'
                      )}
                    >
                      {entry.command !== undefined &&
                      entry.command === entry.summary ? (
                        <ChatCode
                          code={toolCommandHeading(entry.command)}
                          language="bash"
                        />
                      ) : (
                        entry.summary
                      )}
                    </span>
                    <ChatElapsedTime
                      startedAt={entry.startedAt}
                      endedAt={entry.endedAt}
                      durationMs={entry.durationMs}
                      active={isEntryActive(entry)}
                    />
                    {hasDetails && (
                      <CaretDownIcon
                        className={cn(
                          'size-icon-sm shrink-0 transition-transform duration-150',
                          !entryExpanded && '-rotate-90'
                        )}
                      />
                    )}
                  </span>
                  <EntryStatus entry={entry} />
                </button>
                {!entryExpanded && <ToolEntryPreview entry={entry} />}
                {entryExpanded && <ToolEntryDetails entry={entry} />}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
