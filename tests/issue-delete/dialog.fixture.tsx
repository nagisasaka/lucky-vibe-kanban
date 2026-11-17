import { createRoot } from "react-dom/client";
import { createInstance } from "i18next";
import { I18nextProvider } from "react-i18next";
import { HotkeysProvider } from "react-hotkeys-hook";
import NiceModal from "@ebay/nice-modal-react";
import { IssueDeleteDialog } from "../../packages/web-core/src/shared/dialogs/kanban/IssueDeleteDialog";
import en from "../../packages/web-core/src/i18n/locales/en/tasks.json";
import ja from "../../packages/web-core/src/i18n/locales/ja/tasks.json";
import commonEn from "../../packages/web-core/src/i18n/locales/en/common.json";
import commonJa from "../../packages/web-core/src/i18n/locales/ja/common.json";
const i18n = createInstance();
i18n.init({
  lng: new URLSearchParams(location.search).get("lang") || "en",
  fallbackLng: "en",
  resources: {
    en: { tasks: en, common: commonEn },
    ja: { tasks: ja, common: commonJa },
  },
  initImmediate: false,
});
createRoot(document.getElementById("root")!).render(
  <I18nextProvider i18n={i18n}>
    <HotkeysProvider>
      <NiceModal.Provider>
        <button
          id="open"
          onClick={() =>
            void IssueDeleteDialog.show({
              projectId: "project",
              issueId: "issue",
              onDeleteIssue: () => {
                (window as unknown as { offDeletes: number }).offDeletes =
                  ((window as unknown as { offDeletes: number }).offDeletes ||
                    0) + 1;
              },
            })
          }
        >
          Open
        </button>
      </NiceModal.Provider>
    </HotkeysProvider>
  </I18nextProvider>,
);
