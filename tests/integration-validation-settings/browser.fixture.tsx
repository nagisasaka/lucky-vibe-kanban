import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { IntegrationValidationSettings } from "../../packages/web-core/src/shared/dialogs/settings/settings/IntegrationValidationSettings";
import {
  SettingsDirtyProvider,
  useSettingsDirty,
} from "../../packages/web-core/src/shared/dialogs/settings/settings/SettingsDirtyContext";
import type { MachineClient } from "../../packages/web-core/src/shared/lib/machineClient";
import en from "../../packages/web-core/src/i18n/locales/en/settings.json";
import ja from "../../packages/web-core/src/i18n/locales/ja/settings.json";

void i18n
  .use(initReactI18next)
  .init({
    lng: new URLSearchParams(location.search).get("lang") || "en",
    resources: { en: { settings: en }, ja: { settings: ja } },
    interpolation: { escapeValue: false },
  });
async function request(init?: RequestInit) {
  const response = await fetch("/fixture/settings", init);
  if (!response.ok) throw new Error(await response.text());
  return response.json();
}
const client = {
  target: { id: "local" },
  queryScopeKey: ["machine", "local"],
  getIntegrationValidation: () => request(),
  saveIntegrationValidation: (_: string, data: unknown) =>
    request({ method: "PUT", body: JSON.stringify(data) }),
} as unknown as MachineClient;
function Dirty() {
  const { isDirty } = useSettingsDirty();
  return <span data-testid="dirty">{String(isDirty)}</span>;
}
createRoot(document.getElementById("root")!).render(
  <QueryClientProvider
    client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
  >
    <SettingsDirtyProvider>
      <IntegrationValidationSettings
        repoId="fixture"
        repoPath="/fixture/repository"
        client={client}
      />
      <Dirty />
    </SettingsDirtyProvider>
  </QueryClientProvider>,
);
