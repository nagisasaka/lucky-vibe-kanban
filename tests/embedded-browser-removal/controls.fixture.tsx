import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { DevServerControlsContainer } from "../../packages/web-core/src/pages/workspaces/DevServerControlsContainer";
import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { RichTextPlugin } from "@lexical/react/LexicalRichTextPlugin";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { LexicalErrorBoundary } from "@lexical/react/LexicalErrorBoundary";
import { MarkdownSyncPlugin } from "../../packages/ui/src/components/MarkdownSyncPlugin";
import {
  ComponentInfoNode,
  COMPONENT_INFO_TRANSFORMER,
} from "../../packages/ui/src/components/component-info-node";
import { restoreRightMainPanelMode } from "../../packages/web-core/src/shared/stores/useUiPreferencesStore";
import { MOBILE_TABS } from "../../packages/ui/src/components/Navbar";
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import tasks from "../../packages/web-core/src/i18n/locales/en/tasks.json";
const api = window as any;
api.running = false;
api.selected = "one";
api.calls = [];
api.mobileTabs = MOBILE_TABS;
api.restoredPanel = restoreRightMainPanelMode("preview");
api.fixtureLogs = {
  one: [
    { content: "\u001b[32mLocal: http://local" },
    { content: "host:5173/\u001b[0m\n" },
  ],
  two: [{ content: "https://preview.example.test:18443/app\n" }],
};
void i18n
  .use(initReactI18next)
  .init({ lng: "en", resources: { en: { tasks } }, initImmediate: false });
const markdown =
  "```vk-component\n" +
  JSON.stringify({
    framework: "react",
    component: "OldButton",
    file: "src/Button.tsx",
    line: 4,
    htmlPreview: "<button>Old</button>",
  }) +
  "\n```";
function App() {
  const [, refresh] = useState(0);
  api.refresh = () => refresh((n) => n + 1);
  return (
    <>
      <DevServerControlsContainer />
      <div id="history">
        <LexicalComposer
          initialConfig={{
            namespace: "history",
            nodes: [ComponentInfoNode],
            onError: (error) => {
              throw error;
            },
          }}
        >
          <MarkdownSyncPlugin
            value={markdown}
            onChange={() => {}}
            editable={false}
            transformers={[COMPONENT_INFO_TRANSFORMER]}
          />
          <RichTextPlugin
            contentEditable={<ContentEditable />}
            placeholder={null}
            ErrorBoundary={LexicalErrorBoundary}
          />
        </LexicalComposer>
      </div>
    </>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
