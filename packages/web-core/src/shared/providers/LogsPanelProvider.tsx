import {
  useState,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  type ReactNode,
} from 'react';
import { useHostId } from '@/shared/providers/HostIdProvider';
import { useLogStream } from '@/shared/hooks/useLogStream';
import type { LogsPanelContent } from '@/shared/types/actions';
import {
  useWorkspacePanelState,
  RIGHT_MAIN_PANEL_MODES,
} from '@/shared/stores/useUiPreferencesStore';
import { useWorkspaceContext } from '@/shared/hooks/useWorkspaceContext';
import {
  LogsPanelActionsContext,
  LogsPanelContext,
} from '@/shared/hooks/useLogsPanel';

interface LogsPanelProviderProps {
  children: ReactNode;
}

export function LogsPanelProvider({ children }: LogsPanelProviderProps) {
  const { workspaceId, isCreateMode } = useWorkspaceContext();
  const { rightMainPanelMode, setRightMainPanelMode } = useWorkspacePanelState(
    isCreateMode ? undefined : workspaceId
  );
  const rightMainPanelModeRef = useRef(rightMainPanelMode);
  rightMainPanelModeRef.current = rightMainPanelMode;
  const hostId = useHostId();
  const scope = JSON.stringify([hostId, workspaceId, isCreateMode]);
  const [selection, setSelection] = useState<{
    scope: string;
    content: LogsPanelContent | null;
  }>({ scope, content: null });
  const logsPanelContent = selection.scope === scope ? selection.content : null;
  const setLogsPanelContent = useCallback(
    (content: LogsPanelContent | null) => setSelection({ scope, content }),
    [scope]
  );
  useEffect(() => {
    setSelection((previous) =>
      previous.scope === scope ? previous : { scope, content: null }
    );
  }, [scope]);
  const processId =
    rightMainPanelMode === RIGHT_MAIN_PANEL_MODES.LOGS &&
    logsPanelContent?.type === 'process'
      ? logsPanelContent.processId
      : '';
  const { logs: processLogs, error: processLogsError } =
    useLogStream(processId);
  const [logSearchQuery, setLogSearchQuery] = useState('');
  const [logMatchIndices, setLogMatchIndices] = useState<number[]>([]);
  const [logCurrentMatchIdx, setLogCurrentMatchIdx] = useState(0);

  const isTerminalExpanded = logsPanelContent?.type === 'terminal';

  const logContentId =
    logsPanelContent?.type === 'process'
      ? logsPanelContent.processId
      : logsPanelContent?.type === 'tool'
        ? logsPanelContent.toolName
        : null;

  useEffect(() => {
    setLogSearchQuery('');
    setLogCurrentMatchIdx(0);
    setLogMatchIndices([]);
  }, [logContentId, scope]);

  useEffect(() => {
    setLogCurrentMatchIdx(0);
  }, [logSearchQuery]);

  // Collapse terminal when switching away from Logs panel mode
  useEffect(() => {
    if (
      rightMainPanelMode !== RIGHT_MAIN_PANEL_MODES.LOGS &&
      isTerminalExpanded
    ) {
      setLogsPanelContent(null);
    }
  }, [rightMainPanelMode, isTerminalExpanded, setLogsPanelContent]);

  const handleLogPrevMatch = useCallback(() => {
    if (logMatchIndices.length === 0) return;
    setLogCurrentMatchIdx((prev) =>
      prev > 0 ? prev - 1 : logMatchIndices.length - 1
    );
  }, [logMatchIndices.length]);

  const handleLogNextMatch = useCallback(() => {
    if (logMatchIndices.length === 0) return;
    setLogCurrentMatchIdx((prev) =>
      prev < logMatchIndices.length - 1 ? prev + 1 : 0
    );
  }, [logMatchIndices.length]);

  const viewProcessInPanel = useCallback(
    (processId: string) => {
      if (rightMainPanelModeRef.current !== RIGHT_MAIN_PANEL_MODES.LOGS) {
        setRightMainPanelMode(RIGHT_MAIN_PANEL_MODES.LOGS);
      }
      setLogsPanelContent({ type: 'process', processId });
    },
    [setRightMainPanelMode, setLogsPanelContent]
  );

  const viewToolContentInPanel = useCallback(
    (toolName: string, content: string, command?: string) => {
      if (rightMainPanelModeRef.current !== RIGHT_MAIN_PANEL_MODES.LOGS) {
        setRightMainPanelMode(RIGHT_MAIN_PANEL_MODES.LOGS);
      }
      setLogsPanelContent({ type: 'tool', toolName, content, command });
    },
    [setRightMainPanelMode, setLogsPanelContent]
  );

  const expandTerminal = useCallback(() => {
    if (rightMainPanelModeRef.current !== RIGHT_MAIN_PANEL_MODES.LOGS) {
      setRightMainPanelMode(RIGHT_MAIN_PANEL_MODES.LOGS);
    }
    setLogsPanelContent({ type: 'terminal' });
  }, [setRightMainPanelMode, setLogsPanelContent]);

  const collapseTerminal = useCallback(() => {
    setLogsPanelContent(null);
  }, [setLogsPanelContent]);

  const actionsValue = useMemo(
    () => ({
      viewProcessInPanel,
      viewToolContentInPanel,
      expandTerminal,
      collapseTerminal,
    }),
    [
      viewProcessInPanel,
      viewToolContentInPanel,
      expandTerminal,
      collapseTerminal,
    ]
  );

  const value = useMemo(
    () => ({
      logsPanelContent,
      processLogs,
      processLogsError,
      logSearchQuery,
      logMatchIndices,
      logCurrentMatchIdx,
      setLogSearchQuery,
      setLogMatchIndices,
      handleLogPrevMatch,
      handleLogNextMatch,
      viewProcessInPanel,
      viewToolContentInPanel,
      expandTerminal,
      collapseTerminal,
      isTerminalExpanded,
    }),
    [
      logsPanelContent,
      processLogs,
      processLogsError,
      logSearchQuery,
      logMatchIndices,
      logCurrentMatchIdx,
      handleLogPrevMatch,
      handleLogNextMatch,
      viewProcessInPanel,
      viewToolContentInPanel,
      expandTerminal,
      collapseTerminal,
      isTerminalExpanded,
    ]
  );

  return (
    <LogsPanelActionsContext.Provider value={actionsValue}>
      <LogsPanelContext.Provider value={value}>
        {children}
      </LogsPanelContext.Provider>
    </LogsPanelActionsContext.Provider>
  );
}
