// Providers unrelated to pets are inert; all sidebar/layout components stay real.
export const useTerminal = () => ({
  getTabsForWorkspace: () => [],
  createTab: (scope: string, cwd: string) => {
    const state = window as unknown as {
      terminalStarts?: { scope: string; cwd: string }[];
    };
    (state.terminalStarts ??= []).push({ scope, cwd });
  },
});
export const useLogsPanel = () => ({
  isTerminalExpanded: false,
  expandTerminal: () => {},
});
export const useWorkspaceWiki = () => ({ repositoryOptions: [] });
