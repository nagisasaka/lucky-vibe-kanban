import { GeneralSettingsSection } from "../../packages/web-core/src/shared/dialogs/settings/settings/GeneralSettingsSection";
import { SettingsDirtyProvider } from "../../packages/web-core/src/shared/dialogs/settings/settings/SettingsDirtyContext";
import common from "../../packages/web-core/src/i18n/locales/ja/common.json";
import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import NiceModal from "@ebay/nice-modal-react";
import { HotkeysProvider } from "react-hotkeys-hook";
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import settings from "../../packages/web-core/src/i18n/locales/ja/settings.json";
import { ExternalEditorSettings } from "../../packages/web-core/src/shared/components/ExternalEditorSettings";
import { EditorSelectionDialog } from "../../packages/web-core/src/shared/dialogs/command-bar/EditorSelectionDialog";
import { UserSystemContext } from "../../packages/web-core/src/shared/hooks/useUserSystem";
import { useUserSystemController } from "../../packages/web-core/src/shared/hooks/useUserSystemController";
import type { Config, UserSystemInfo } from "../../shared/types";

const api = window as any;
api.saved = {
  editor: { entries: null },
  git_branch_prefix: "keep",
  theme: "SYSTEM",
  language: "JA",
  executor_profile: { executor: "CLAUDE_CODE", variant: null },
  notifications: {
    sound_enabled: false,
    push_enabled: false,
    sound_file: "COW_MOOING",
  },
};
api.saveCount = 0;
api.requests = [];
api.resolve = async (kind: string, id: string, request: any) => {
  api.requests.push({ kind, id, request });
  await new Promise((resolve) => setTimeout(resolve, 80));
  if (id === "missing" || api.failWorkspace)
    throw new Error(
      "Editor directory is unavailable; no worktree will be created",
    );
  return {
    entries: api.saved.editor.entries.map((entry: any) => {
      const unavailable =
        entry.uri_template.includes("<lvk_file_path>") && !request.file_path;
      return {
        name: entry.name,
        url: unavailable
          ? null
          : entry.uri_template
              .replaceAll("<lvk_workspace_dir>", "/workspace/repo")
              .replaceAll("<lvk_file_path>", "/workspace/repo/file.txt"),
        unavailable_reason: unavailable
          ? "This template requires a file; select a file to use it"
          : null,
      };
    }),
  };
};
void i18n.use(initReactI18next).init({
  lng: "ja",
  resources: { ja: { settings, common } },
  initImmediate: false,
});
const load = async () =>
  ({ config: structuredClone(api.saved) }) as UserSystemInfo;
const save = async (config: Config) => {
  if (api.rejectSave) throw new Error("Invalid URI template (fixture)");
  api.saved = structuredClone(config);
  api.saveCount++;
  return config;
};
const queryKey = ["user-system", "fixture"];

function Fixture() {
  const { value } = useUserSystemController({ queryKey, load, save });
  const [message, setMessage] = useState("");
  const [general, setGeneral] = useState(false);
  if (!value.config) return <p>Loading</p>;
  return (
    <UserSystemContext.Provider value={value}>
      <NiceModal.Provider>
        <main className="mx-auto max-w-3xl space-y-base bg-primary p-base text-normal">
          <h1>外部エディター連携</h1>
          {general ? (
            <SettingsDirtyProvider>
              <GeneralSettingsSection />
            </SettingsDirtyProvider>
          ) : (
            <ExternalEditorSettings
              value={value.config.editor}
              onChange={(editor) => value.updateConfig({ editor })}
            />
          )}
          <button onClick={() => setGeneral(!general)}>General settings</button>
          <button
            onClick={async () => {
              await value.updateAndSaveConfig(value.config!);
              setMessage("Saved");
            }}
          >
            Save
          </button>
          <button onClick={() => value.reloadSystem()}>Reload</button>
          <button
            onClick={() =>
              EditorSelectionDialog.show({ selectedAttemptId: "workspace" })
            }
          >
            Workspace menu
          </button>
          <button
            onClick={() =>
              EditorSelectionDialog.show({
                selectedAttemptId: "workspace",
                repoId: "repo",
                filePath: "file.txt",
              })
            }
          >
            File menu
          </button>
          <button
            onClick={() =>
              EditorSelectionDialog.show({
                selectedAttemptId: "workspace",
                repoId: "repo",
              })
            }
          >
            Worktree repo menu
          </button>
          <button
            onClick={() => EditorSelectionDialog.show({ repoId: "repo" })}
          >
            Original repo menu
          </button>
          <button
            onClick={() =>
              EditorSelectionDialog.show({ selectedAttemptId: "missing" })
            }
          >
            Missing menu
          </button>
          <p role="status">{message}</p>
        </main>
      </NiceModal.Provider>
    </UserSystemContext.Provider>
  );
}
createRoot(document.getElementById("root")!).render(
  <QueryClientProvider client={new QueryClient()}>
    <HotkeysProvider>
      <Fixture />
    </HotkeysProvider>
  </QueryClientProvider>,
);
