import { createRoot } from "react-dom/client";
import { useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import i18n from "../../packages/web-core/src/i18n/config";
import { EnvironmentPanel } from "../../packages/web-core/src/shared/components/EnvironmentPanel";
import { SharedStorageSettings } from "../../packages/web-core/src/shared/dialogs/settings/settings/SharedStorageSettings";
import type { MachineClient } from "../../packages/web-core/src/shared/lib/machineClient";
import type { Repo } from "../../shared/types";

void i18n.changeLanguage("ja");
const client = {
  getSharedStorage: (id: string) => request(id),
  clearSharedStorage: (id: string, kind: string) => request(id, kind),
} as unknown as MachineClient;
async function request(id: string, kind?: string) {
  const response = await fetch(
    `/api/repos/${id}/shared-storage${kind ? `/${kind}` : ""}`,
    { method: kind ? "DELETE" : "GET" },
  );
  if (!response.ok) throw new Error("fixture request failed");
  return (await response.json()).data;
}
function App() {
  const [settings, setSettings] = useState(false);
  const [id, setId] = useState("one");
  return (
    <main className="min-h-screen bg-primary text-normal p-base">
      <button onClick={() => setSettings(!settings)}>設定を切替</button>
      <div className="max-w-2xl space-y-base">
        <div className="w-72 max-w-full">
          <EnvironmentPanel repoIds={["one", "one", "two"]} />
        </div>
        {settings && (
          <>
            <button onClick={() => setId(id === "one" ? "two" : "one")}>
              Repositoryを切替
            </button>
            <SharedStorageSettings
              key={id}
              repo={
                {
                  id,
                  display_name: `Repository ${id}`,
                  path: `/fixture/${id}`,
                } as Repo
              }
              client={client}
            />
          </>
        )}
      </div>
    </main>
  );
}
createRoot(document.getElementById("root")!).render(
  <QueryClientProvider client={new QueryClient()}>
    <App />
  </QueryClientProvider>,
);
