import {
  useReducer,
  useMemo,
  useCallback,
  useRef,
  useEffect,
  ReactNode,
} from 'react';
import type { Terminal } from '@xterm/xterm';
import type { FitAddon } from '@xterm/addon-fit';
import {
  TerminalContext,
  type TerminalTab,
  type TerminalInstance,
} from '@/shared/hooks/useTerminal';
import { openLocalApiWebSocket } from '@/shared/lib/localApiTransport';

interface TerminalConnection {
  ws: WebSocket;
  send: (data: string) => void;
  resize: (cols: number, rows: number) => void;
}

interface TerminalState {
  tabsByWorkspace: Record<string, TerminalTab[]>;
  activeTabByWorkspace: Record<string, string | null>;
}

type TerminalAction =
  | { type: 'CREATE_TAB'; workspaceId: string; cwd: string; tab: TerminalTab }
  | { type: 'CLOSE_TAB'; workspaceId: string; tabId: string }
  | { type: 'SET_ACTIVE_TAB'; workspaceId: string; tabId: string }
  | {
      type: 'UPDATE_TAB_TITLE';
      workspaceId: string;
      tabId: string;
      title: string;
    }
  | { type: 'CLEAR_WORKSPACE_TABS'; workspaceId: string };

function generateTabId(): string {
  return `term-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
}

function encodeBase64(str: string): string {
  const bytes = new TextEncoder().encode(str);
  const binString = Array.from(bytes, (b) => String.fromCodePoint(b)).join('');
  return btoa(binString);
}

function decodeBase64(base64: string): string {
  const binString = atob(base64);
  const bytes = Uint8Array.from(binString, (c) => c.codePointAt(0)!);
  return new TextDecoder().decode(bytes);
}

function terminalReducer(
  state: TerminalState,
  action: TerminalAction
): TerminalState {
  switch (action.type) {
    case 'CREATE_TAB': {
      const { workspaceId, tab: newTab } = action;
      return {
        ...state,
        tabsByWorkspace: {
          ...state.tabsByWorkspace,
          [workspaceId]: [newTab],
        },
        activeTabByWorkspace: {
          ...state.activeTabByWorkspace,
          [workspaceId]: newTab.id,
        },
      };
    }

    case 'CLOSE_TAB': {
      const { workspaceId, tabId } = action;
      const tabs = state.tabsByWorkspace[workspaceId] || [];
      const newTabs = tabs.filter((t) => t.id !== tabId);
      const wasActive = state.activeTabByWorkspace[workspaceId] === tabId;
      let newActiveTab = state.activeTabByWorkspace[workspaceId];

      if (wasActive && newTabs.length > 0) {
        const closedIndex = tabs.findIndex((t) => t.id === tabId);
        const newIndex = Math.min(closedIndex, newTabs.length - 1);
        newActiveTab = newTabs[newIndex]?.id ?? null;
      } else if (newTabs.length === 0) {
        newActiveTab = null;
      }

      return {
        ...state,
        tabsByWorkspace: {
          ...state.tabsByWorkspace,
          [workspaceId]: newTabs,
        },
        activeTabByWorkspace: {
          ...state.activeTabByWorkspace,
          [workspaceId]: newActiveTab,
        },
      };
    }

    case 'SET_ACTIVE_TAB': {
      const { workspaceId, tabId } = action;
      return {
        ...state,
        activeTabByWorkspace: {
          ...state.activeTabByWorkspace,
          [workspaceId]: tabId,
        },
      };
    }

    case 'UPDATE_TAB_TITLE': {
      const { workspaceId, tabId, title } = action;
      const tabs = state.tabsByWorkspace[workspaceId] || [];
      return {
        ...state,
        tabsByWorkspace: {
          ...state.tabsByWorkspace,
          [workspaceId]: tabs.map((t) =>
            t.id === tabId ? { ...t, title } : t
          ),
        },
      };
    }

    case 'CLEAR_WORKSPACE_TABS': {
      const { workspaceId } = action;
      const restTabs = Object.fromEntries(
        Object.entries(state.tabsByWorkspace).filter(
          ([key]) => key !== workspaceId
        )
      );
      const restActive = Object.fromEntries(
        Object.entries(state.activeTabByWorkspace).filter(
          ([key]) => key !== workspaceId
        )
      );
      return {
        tabsByWorkspace: restTabs,
        activeTabByWorkspace: restActive,
      };
    }

    default:
      return state;
  }
}

interface TerminalProviderProps {
  children: ReactNode;
}

export function TerminalProvider({ children }: TerminalProviderProps) {
  const [state, dispatch] = useReducer(terminalReducer, {
    tabsByWorkspace: {},
    activeTabByWorkspace: {},
  });

  // Store terminal instances in a ref to persist across re-renders
  const terminalInstancesRef = useRef<Map<string, TerminalInstance>>(new Map());

  // Store WebSocket connections in a ref to persist across component remounts
  const terminalConnectionsRef = useRef<Map<string, TerminalConnection>>(
    new Map()
  );

  // Store callback refs for each connection to prevent stale closures
  const connectionCallbacksRef = useRef<
    Map<string, { onData: (data: string) => void; onExit?: () => void }>
  >(new Map());

  // Scope ownership is synchronous: repeated opens (including StrictMode) are idempotent.
  const ownedTabsRef = useRef(new Map<string, TerminalTab>());
  const openingConnectionsRef = useRef(new Map<string, object>());

  const closingScopesRef = useRef(new Map<string, Promise<void>>());
  useEffect(
    () => () => {
      openingConnectionsRef.current.clear();
      for (const conn of terminalConnectionsRef.current.values())
        conn.ws.close();
      terminalConnectionsRef.current.clear();
      for (const instance of terminalInstancesRef.current.values())
        instance.terminal.dispose();
      terminalInstancesRef.current.clear();
    },
    []
  );

  const getTabsForWorkspace = useCallback(
    (workspaceId: string): TerminalTab[] => {
      return state.tabsByWorkspace[workspaceId] || [];
    },
    [state.tabsByWorkspace]
  );

  const getActiveTab = useCallback(
    (workspaceId: string): TerminalTab | null => {
      const activeId = state.activeTabByWorkspace[workspaceId];
      if (!activeId) return null;
      const tabs = state.tabsByWorkspace[workspaceId] || [];
      return tabs.find((t) => t.id === activeId) || null;
    },
    [state.tabsByWorkspace, state.activeTabByWorkspace]
  );

  const createTab = useCallback((workspaceId: string, cwd: string) => {
    if (ownedTabsRef.current.has(workspaceId)) return;
    const tab = { id: generateTabId(), title: 'Terminal', workspaceId, cwd };
    ownedTabsRef.current.set(workspaceId, tab);
    dispatch({ type: 'CREATE_TAB', workspaceId, cwd, tab });
  }, []);

  const closeTerminalConnection = useCallback((tabId: string) => {
    openingConnectionsRef.current.delete(tabId);
    const conn = terminalConnectionsRef.current.get(tabId);
    terminalConnectionsRef.current.delete(tabId);
    connectionCallbacksRef.current.delete(tabId);
    if (conn?.ws.readyState === WebSocket.OPEN)
      conn.ws.send(JSON.stringify({ type: 'close' }));
    else conn?.ws.close();
  }, []);

  const closeTab = useCallback(
    (workspaceId: string, tabId: string) => {
      if (ownedTabsRef.current.get(workspaceId)?.id !== tabId) return;
      ownedTabsRef.current.delete(workspaceId);
      const connection = terminalConnectionsRef.current.get(tabId);
      if (connection && connection.ws.readyState !== WebSocket.CLOSED) {
        const settled = new Promise<void>((resolve) => {
          const previous = connection.ws.onclose;
          connection.ws.onclose = (event) => {
            previous?.call(connection.ws, event);
            resolve();
          };
        });
        closingScopesRef.current.set(workspaceId, settled);
        void settled.then(() => {
          if (closingScopesRef.current.get(workspaceId) === settled)
            closingScopesRef.current.delete(workspaceId);
        });
      }
      // Dispose the terminal instance when closing the tab
      const instance = terminalInstancesRef.current.get(tabId);
      if (instance) {
        instance.terminal.dispose();
        terminalInstancesRef.current.delete(tabId);
      }
      // Close the WebSocket connection
      closeTerminalConnection(tabId);
      dispatch({ type: 'CLOSE_TAB', workspaceId, tabId });
    },
    [closeTerminalConnection]
  );

  const setActiveTab = useCallback((workspaceId: string, tabId: string) => {
    dispatch({ type: 'SET_ACTIVE_TAB', workspaceId, tabId });
  }, []);

  const updateTabTitle = useCallback(
    (workspaceId: string, tabId: string, title: string) => {
      dispatch({ type: 'UPDATE_TAB_TITLE', workspaceId, tabId, title });
    },
    []
  );

  const clearWorkspaceTabs = useCallback(
    (workspaceId: string) => {
      const tab = ownedTabsRef.current.get(workspaceId);
      if (tab) closeTab(workspaceId, tab.id);
    },
    [closeTab]
  );

  const registerTerminalInstance = useCallback(
    (tabId: string, terminal: Terminal, fitAddon: FitAddon) => {
      terminalInstancesRef.current.set(tabId, { terminal, fitAddon });
    },
    []
  );

  const getTerminalInstance = useCallback(
    (tabId: string): TerminalInstance | null => {
      return terminalInstancesRef.current.get(tabId) || null;
    },
    []
  );

  const unregisterTerminalInstance = useCallback((tabId: string) => {
    terminalInstancesRef.current.delete(tabId);
  }, []);

  const createTerminalConnection = useCallback(
    (
      tabId: string,
      endpoint: string,
      onData: (data: string) => void,
      onExit?: () => void
    ) => {
      // A lost socket is terminal. It must never create a replacement shell.
      if (
        openingConnectionsRef.current.has(tabId) ||
        terminalConnectionsRef.current.has(tabId)
      ) {
        return {
          send: (data: string) =>
            terminalConnectionsRef.current.get(tabId)?.send(data),
          resize: (cols: number, rows: number) =>
            terminalConnectionsRef.current.get(tabId)?.resize(cols, rows),
        };
      }
      const token = {};
      openingConnectionsRef.current.set(tabId, token);
      connectionCallbacksRef.current.set(tabId, { onData, onExit });
      const finish = () => {
        if (openingConnectionsRef.current.get(tabId) !== token) return;
        openingConnectionsRef.current.delete(tabId);
        const callback = connectionCallbacksRef.current.get(tabId)?.onExit;
        // Let closeTab observe the closing socket before removing it. A user
        // reopening immediately must wait for this transport to settle too.
        callback?.();
        terminalConnectionsRef.current.delete(tabId);
        connectionCallbacksRef.current.delete(tabId);
      };
      void (async () => {
        try {
          const scope = [...ownedTabsRef.current].find(
            ([, tab]) => tab.id === tabId
          )?.[0];
          if (scope) await closingScopesRef.current.get(scope);
          if (openingConnectionsRef.current.get(tabId) !== token) return;
          const ws = await openLocalApiWebSocket(endpoint);
          if (openingConnectionsRef.current.get(tabId) !== token) {
            ws.close();
            return;
          }
          let lastMessage = Date.now();
          const watchdog = window.setInterval(() => {
            if (Date.now() - lastMessage >= 45_000) {
              ws.close();
              finish();
            }
          }, 5000);
          ws.onmessage = (event) => {
            lastMessage = Date.now();
            try {
              const msg = JSON.parse(event.data);
              if (msg.type === 'output' && msg.data)
                connectionCallbacksRef.current
                  .get(tabId)
                  ?.onData(decodeBase64(msg.data));
              else if (msg.type === 'exit' || msg.type === 'error') {
                ws.close();
                finish();
              }
            } catch {
              /* Ignore malformed frames. */
            }
          };
          ws.onclose = () => {
            window.clearInterval(watchdog);
            finish();
          };
          ws.onerror = () => {
            window.clearInterval(watchdog);
            ws.close();
            finish();
          };
          terminalConnectionsRef.current.set(tabId, {
            ws,
            send: (data) => {
              if (ws.readyState === WebSocket.OPEN)
                ws.send(
                  JSON.stringify({ type: 'input', data: encodeBase64(data) })
                );
            },
            resize: (cols, rows) => {
              if (ws.readyState === WebSocket.OPEN)
                ws.send(JSON.stringify({ type: 'resize', cols, rows }));
            },
          });
        } catch {
          finish();
        }
      })();

      // Return functions that use the current connection
      const send = (data: string) => {
        const conn = terminalConnectionsRef.current.get(tabId);
        conn?.send(data);
      };

      const resize = (cols: number, rows: number) => {
        const conn = terminalConnectionsRef.current.get(tabId);
        conn?.resize(cols, rows);
      };

      return { send, resize };
    },
    []
  );

  const getTerminalConnection = useCallback(
    (tabId: string): TerminalConnection | null => {
      return terminalConnectionsRef.current.get(tabId) || null;
    },
    []
  );

  const value = useMemo(
    () => ({
      getTabsForWorkspace,
      getActiveTab,
      createTab,
      closeTab,
      setActiveTab,
      updateTabTitle,
      clearWorkspaceTabs,
      registerTerminalInstance,
      getTerminalInstance,
      unregisterTerminalInstance,
      createTerminalConnection,
      getTerminalConnection,
    }),
    [
      getTabsForWorkspace,
      getActiveTab,
      createTab,
      closeTab,
      setActiveTab,
      updateTabTitle,
      clearWorkspaceTabs,
      registerTerminalInstance,
      getTerminalInstance,
      unregisterTerminalInstance,
      createTerminalConnection,
      getTerminalConnection,
    ]
  );

  return (
    <TerminalContext.Provider value={value}>
      {children}
    </TerminalContext.Provider>
  );
}
