import { useQuery, useQueryClient } from "@tanstack/react-query";
// Only surrounding app context is mocked; the Integration UI and API client are real.
export const useProjectContext = () => ({
  projectId: "project",
  issues: [{ id: "card", simple_id: "LVK-1", title: "Example change" }],
});
export const useAppNavigation = () => ({ goToWorkspace: () => {} });
export const useUserSystem = () => {
  const client = useQueryClient();
  const { data: config } = useQuery({
    queryKey: ["config"],
    queryFn: async () => (await (await fetch("/api/info")).json()).data.config,
  });
  return {
    profiles: {},
    config,
    updateAndSaveConfig: async (updates: object) => {
      client.setQueryData(["config"], { ...config, ...updates });
      const response = await fetch("/api/config", {
        method: "PUT",
        body: JSON.stringify({ ...config, ...updates }),
      });
      if (!response.ok) return false;
      client.setQueryData(["config"], (await response.json()).data);
      return true;
    },
  };
};
export const useExecutorConfig = () => ({
  executorConfig: { executor: "CODEX", execution_mode: "code" },
  effectiveExecutor: "CODEX",
  executorOptions: ["CODEX"],
});
export const useAgentProviderOptions = () => ({
  options: [{ executor: "CODEX", enabled: true }],
});
export const ModelSelectorContainer = () => null;
export const SettingsDialog = { show: async () => {} };
export const repoApi = {
  getBranches: async () => [{ name: "main", is_remote: false }],
};
export async function handleApiResponse(response: Response) {
  const value = await response.json();
  if (!response.ok) throw new Error(value.message);
  return value.data;
}
