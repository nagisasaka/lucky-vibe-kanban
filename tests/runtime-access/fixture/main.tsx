// Production settings component/API client with isolated HTTP fixtures in Playwright.
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import common from "../../../packages/web-core/src/i18n/locales/ja/common.json";
import settings from "../../../packages/web-core/src/i18n/locales/ja/settings.json";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RuntimeAccessSettingsSection } from "../../../packages/web-core/src/shared/dialogs/settings/settings/RuntimeAccessSettingsSection";
import { SettingsDirtyProvider } from "../../../packages/web-core/src/shared/dialogs/settings/settings/SettingsDirtyContext";
import "../../../packages/web-core/src/app/styles/new/index.css";
i18n.use(initReactI18next).init({
  lng: "ja",
  resources: { ja: { settings, common } },
  defaultNS: "settings",
});
createRoot(document.getElementById("root")!).render(
  <QueryClientProvider
    client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
  >
    <SettingsDirtyProvider>
      <main className="p-double max-w-3xl">
        <RuntimeAccessSettingsSection />
      </main>
    </SettingsDirtyProvider>
  </QueryClientProvider>,
);
