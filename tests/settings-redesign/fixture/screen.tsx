import { SessionChatFixture } from "./session-chat";
import { RepositoryDialogsFixture } from "./repository-dialogs";
import { ApprovalFixture } from "./approval";
import { IssueArchiveFixture } from "./issue-archive";
import { NotificationsFixture } from "./notifications";
import { ModelSelectorFixture } from "./model-selector";
import { LandingPage } from "@/features/onboarding/ui/LandingPage";
import { NavbarFixture } from "./navbar";
import { ElectricTestPage } from "@/pages/workspaces/ElectricTestPage";
import { ActionLabelsFixture } from "./action-labels";
import { GoalControlsFixture } from "./goal-controls";
import { SchemaFormFixture } from "./schema-form";
import { AgentConfigurationSettingsPanel } from "@/shared/dialogs/settings/settings/AgentConfigurationSettingsPanel";
import { SettingsDirtyProvider } from "@/shared/dialogs/settings/settings/SettingsDirtyContext";
import { BaseCodingAgent } from "../../../shared/types";
import { AgentToolsSettingsSection } from "@/shared/dialogs/settings/settings/AgentToolsSettingsSection";
import { RepositoryFormsFixture } from "./repository-forms";
import { ReleaseNotesDialog } from "@/shared/dialogs/global/ReleaseNotesDialog";
import { DesignNotePanel } from "@/features/design-notes/ui/DesignNotePanel";
import { Fragment, useState } from "react";
import { SettingsMachineUserSystemProvider } from "@/shared/dialogs/settings/settings/SettingsMachineUserSystemProvider";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { HotkeysProvider } from "react-hotkeys-hook";
import { Provider } from "@ebay/nice-modal-react";
import i18n from "@/i18n/config";
import { AppRuntimeProvider } from "@/shared/hooks/useAppRuntime";
import { AuthContext } from "@/shared/hooks/auth/useAuth";
import { AppBarUserPopoverContainer } from "@/shared/components/ui-new/containers/AppBarUserPopoverContainer";
import { SettingsDialog } from "@/shared/dialogs/settings/SettingsDialog";
import type { SettingsSectionType } from "@/shared/dialogs/settings/settings/settingsRegistry";
import { ServerAccessGate } from "../../../packages/local-web/src/app/auth/ServerAccessGate";
import "@/app/styles/new/index.css";

const params = new URLSearchParams(location.search);
void i18n.changeLanguage(params.get("lang") ?? "ja");
Object.defineProperty(window, "__setUiLanguage", {
  value: (language: string) => i18n.changeLanguage(language),
});
if (params.get("theme") === "dark")
  document.documentElement.classList.add("dark");
function Screen() {
  const [opened, setOpened] = useState(false);
  const open = () => {
    setOpened(true);
    if (params.get("view") === "release-notes") {
      void ReleaseNotesDialog.show();
      return;
    }
    void SettingsDialog.show({
      initialSection: (params.get("section") ??
        undefined) as SettingsSectionType,
    });
  };
  if (params.get("view") === "session-chat") return <SessionChatFixture />;
  if (params.get("view") === "repository-dialogs")
    return <RepositoryDialogsFixture />;
  if (params.get("view") === "approval") return <ApprovalFixture />;
  if (params.get("view") === "issue-archive") return <IssueArchiveFixture />;
  if (params.get("view") === "notifications") return <NotificationsFixture />;
  if (params.get("view") === "model-selector") return <ModelSelectorFixture />;
  if (params.get("view") === "onboarding") return <LandingPage />;
  if (params.get("view") === "navbar") return <NavbarFixture />;
  if (params.get("view") === "electric-diagnostics")
    return <ElectricTestPage />;
  if (params.get("view") === "action-labels") return <ActionLabelsFixture />;
  if (params.get("view") === "goal-controls") return <GoalControlsFixture />;
  if (params.get("view") === "schema-form") return <SchemaFormFixture />;
  if (params.get("view") === "design-notes")
    return (
      <div className="h-screen text-normal bg-primary">
        <DesignNotePanel projectId="project-fixture" />
      </div>
    );
  if (params.get("view") === "agent-configuration")
    return (
      <SettingsDirtyProvider>
        <div className="p-6 max-w-3xl text-normal bg-primary">
          <AgentConfigurationSettingsPanel
            executor={BaseCodingAgent.CODEX}
            variant={null}
          />
        </div>
      </SettingsDirtyProvider>
    );
  if (params.get("view") === "agent-tools")
    return (
      <div className="p-double max-w-3xl text-normal bg-primary">
        <AgentToolsSettingsSection />
      </div>
    );
  if (params.get("view") === "repository-forms")
    return <RepositoryFormsFixture />;
  return (
    <div className="p-double text-normal bg-primary min-h-screen">
      <h1 className="text-lg text-high">
        設定UI試作 / Isolated settings prototype
      </h1>
      <p>
        サンプルデータのみ。保存はこのページのメモリ内です。再読込でリセットします。実認証の変更・実行・復旧は無効です。
      </p>
      <p>
        Only sample data. Saves last until reload. Authentication changes and
        recovery are disabled.
      </p>
      {params.get("gear") === "true" ? (
        <AppBarUserPopoverContainer />
      ) : (
        <button className="rounded border p-base mt-double" onClick={open}>
          {opened ? "設定を開き直す" : "設定を開く"}
        </button>
      )}
    </div>
  );
}
const FixtureSystem = params.has("pr")
  ? SettingsMachineUserSystemProvider
  : Fragment;
createRoot(document.getElementById("root")!).render(
  <QueryClientProvider
    client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}
  >
    <AppRuntimeProvider runtime="local">
      <AuthContext.Provider
        value={{ isSignedIn: false, isLoaded: true, userId: null }}
      >
        <HotkeysProvider initiallyActiveScopes={["*"]}>
          <ServerAccessGate>
            <FixtureSystem>
              <Provider>
                <Screen />
              </Provider>
            </FixtureSystem>
          </ServerAccessGate>
        </HotkeysProvider>
      </AuthContext.Provider>
    </AppRuntimeProvider>
  </QueryClientProvider>,
);
