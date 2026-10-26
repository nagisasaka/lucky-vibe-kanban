import React from "react";
import { KanbanCardContent } from "@vibe/ui/components/KanbanCardContent";
import { createRoot } from "react-dom/client";
import NiceModal from "@ebay/nice-modal-react";
import { HotkeysProvider } from "react-hotkeys-hook";
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import projects from "../../packages/web-core/src/i18n/locales/ja/projects.json";
import common from "../../packages/web-core/src/i18n/locales/ja/common.json";
import { CreateRemoteProjectDialog } from "../../packages/web-core/src/shared/dialogs/org/CreateRemoteProjectDialog";

void i18n.use(initReactI18next).init({
  lng: "ja",
  resources: { ja: { projects, common } },
  initImmediate: false,
});
createRoot(document.getElementById("root")!).render(
  <HotkeysProvider>
    <NiceModal.Provider>
      <KanbanCardContent
        displayId="LVK-161"
        title="New key issue"
        priority={null}
        tags={[]}
        assignees={[]}
      />
      <button
        onClick={() =>
          void CreateRemoteProjectDialog.show({ organizationId: "org" })
        }
      >
        Open
      </button>
    </NiceModal.Provider>
  </HotkeysProvider>,
);
