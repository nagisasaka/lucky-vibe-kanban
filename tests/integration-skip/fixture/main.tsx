import { useTodoViewStore } from "../../../packages/web-core/src/features/todos/model/todoViewStore";
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import common from "../../../packages/web-core/src/i18n/locales/en/common.json";
i18n
  .use(initReactI18next)
  .init({ lng: "en", resources: { en: { common } }, defaultNS: "common" });
import { History } from "./History";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { HotkeysProvider } from "react-hotkeys-hook";
import { IntegrationPanel } from "../../../packages/web-core/src/features/kanban/ui/IntegrationPanel";
import "../../../packages/web-core/src/app/styles/new/index.css";
createRoot(document.getElementById("root")!).render(
  <QueryClientProvider
    client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
  >
    <HotkeysProvider>
      {location.search.includes("history") ? (
        <History />
      ) : (
        <>
          <button
            onClick={() =>
              useTodoViewStore.getState().setIntegrationProject("project")
            }
          >
            Integrate / Auto Merge
          </button>
          <IntegrationPanel focusedRunId={null} onClearFocus={() => {}} />
        </>
      )}
    </HotkeysProvider>
  </QueryClientProvider>,
);
