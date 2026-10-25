import { useCallback, useLayoutEffect, useRef, useState } from 'react';
import { TerminalWindowIcon } from '@phosphor-icons/react';
import type { ToolStatus } from 'shared/types';
import { ChatToolSummary } from '@vibe/ui/components/ChatToolSummary';
import { ChatAggregatedToolEntries } from '@vibe/ui/components/ChatAggregatedToolEntries';
import { usePersistedExpanded } from '@/shared/stores/useUiPreferencesStore';
import { useLogsPanelActions } from '@/shared/hooks/useLogsPanel';
import type { PatchTypeWithKey } from '@/shared/hooks/useConversationHistory/types';

export function ToolSummaryEntry({
  summary,
  expansionKey,
  status,
  active,
  content,
  toolName,
  command,
  actionType,
  startedAt,
  endedAt,
  durationMs,
  toolOutput,
}: {
  summary: string;
  expansionKey: string;
  status: ToolStatus;
  active: boolean;
  content: string;
  toolName: string;
  command: string | undefined;
  actionType: string;
  startedAt: string | null;
  endedAt: string | null;
  durationMs?: number | null;
  toolOutput?: PatchTypeWithKey['toolOutput'];
}) {
  const [expanded, toggle] = usePersistedExpanded(
    `tool:${expansionKey}`,
    false
  );
  const { viewToolContentInPanel } = useLogsPanelActions();
  const textRef = useRef<HTMLSpanElement>(null);
  const [isTruncated, setIsTruncated] = useState(false);

  useLayoutEffect(() => {
    const el = textRef.current;
    if (el && !expanded) {
      setIsTruncated(el.scrollWidth > el.clientWidth);
    }
  }, [summary, expanded]);

  // Any tool with output can open the logs panel
  const hasOutput = content && content.trim().length > 0;

  const handleViewContent = useCallback(() => {
    viewToolContentInPanel(toolName, content, command);
  }, [viewToolContentInPanel, toolName, content, command]);

  if (command !== undefined || toolOutput) {
    return (
      <ChatAggregatedToolEntries
        entries={[
          {
            summary,
            expansionKey,
            status,
            active,
            command,
            content,
            startedAt,
            endedAt,
            durationMs,
            outputState: toolOutput?.state,
            exitCode: toolOutput?.exitCode,
            cwd: toolOutput?.cwd,
            language: toolOutput?.language ?? undefined,
            input: toolOutput?.input ?? undefined,
          },
        ]}
        summary={summary}
        startedAt={startedAt}
        endedAt={endedAt}
        icon={command !== undefined ? TerminalWindowIcon : undefined}
        expanded={expanded}
        onToggle={toggle}
        isHovered={false}
        onHoverChange={() => {}}
      />
    );
  }

  return (
    <ChatToolSummary
      ref={textRef}
      summary={summary}
      expanded={expanded}
      onToggle={toggle}
      status={status}
      active={active}
      onViewContent={hasOutput ? handleViewContent : undefined}
      toolName={toolName}
      isTruncated={isTruncated}
      actionType={actionType}
      startedAt={startedAt}
      endedAt={endedAt}
      durationMs={durationMs}
    />
  );
}
