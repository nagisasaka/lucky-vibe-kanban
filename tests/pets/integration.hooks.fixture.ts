// Providers unrelated to pets are inert; all sidebar/layout components stay real.
export const useTerminal = () => ({ getTabsForWorkspace: () => [] });
export const useLogsPanel = () => ({
  isTerminalExpanded: false,
  expandTerminal: () => {},
});
export const useWorkspaceWiki = () => ({ repositoryOptions: [] });
