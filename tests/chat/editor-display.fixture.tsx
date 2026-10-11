import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { RichTextPlugin } from "@lexical/react/LexicalRichTextPlugin";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { LexicalErrorBoundary } from "@lexical/react/LexicalErrorBoundary";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";
import { KEY_ENTER_COMMAND, KEY_MODIFIER_COMMAND } from "lexical";
import { MarkdownSyncPlugin } from "../../packages/ui/src/components/MarkdownSyncPlugin";
import { KeyboardCommandsPlugin } from "../../packages/ui/src/components/KeyboardCommandsPlugin";
import { TypeaheadOpenProvider } from "../../packages/ui/src/components/TypeaheadOpenContext";
import { ChatAggregatedToolEntries } from "../../packages/ui/src/components/ChatAggregatedToolEntries";
import { ChatCollapsedThinking } from "../../packages/ui/src/components/ChatCollapsedThinking";
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { useSessionMessageEditor } from "../../packages/web-core/src/features/workspace-chat/model/hooks/useSessionMessageEditor";
import common from "../../packages/web-core/src/i18n/locales/ja/common.json";
import tasks from "../../packages/web-core/src/i18n/locales/ja/tasks.json";

const api = window as any;
const transformers = [];
api.submissions = 0;
void i18n.use(initReactI18next).init({
  lng: "ja",
  resources: { ja: { common, tasks } },
  initImmediate: false,
});
function Capture() {
  const [editor] = useLexicalComposerContext();
  api.enter = (composing: boolean, modifier = false, legacy = false) =>
    editor.dispatchCommand(
      modifier ? KEY_MODIFIER_COMMAND : KEY_ENTER_COMMAND,
      new KeyboardEvent("keydown", {
        key: "Enter",
        isComposing: composing,
        ctrlKey: modifier,
        keyCode: legacy ? 229 : 13,
        cancelable: true,
      }),
    );
  return null;
}
function Editor({
  id,
  value,
  onChange,
  editable = true,
  shortcut = "Enter",
}: any) {
  return (
    <LexicalComposer
      initialConfig={{
        namespace: id,
        onError: (e) => {
          throw e;
        },
      }}
    >
      <MarkdownSyncPlugin
        value={value}
        onChange={onChange}
        editable={editable}
        transformers={transformers}
      />
      <RichTextPlugin
        contentEditable={<ContentEditable id={id} />}
        placeholder={null}
        ErrorBoundary={LexicalErrorBoundary}
      />
      {id === "composer" && (
        <TypeaheadOpenProvider>
          <Capture />
          <KeyboardCommandsPlugin
            sendShortcut={shortcut}
            onCmdEnter={() => api.submissions++}
          />
        </TypeaheadOpenProvider>
      )}
    </LexicalComposer>
  );
}
function PendingDraft() {
  const [snapshot, setSnapshot] = useState({
    isLoading: true,
    scratch: undefined,
  });
  api.scratchSnapshot = snapshot;
  api.setScratchSnapshot = setSnapshot;
  const draft = useSessionMessageEditor({ scratchId: "pending-session" });
  api.pendingDraft = draft;
  return (
    <Editor
      id="pending-composer"
      value={draft.hasInitialValue ? draft.localMessage : ""}
      onChange={draft.setLocalMessage}
    />
  );
}
function Harness() {
  const [value, setValue] = useState("");
  const [log, setLog] = useState("");
  const [draft, setDraft] = useState("");
  const [toolsExpanded, setToolsExpanded] = useState(false);
  const [thinkingExpanded, setThinkingExpanded] = useState(false);
  const [shortcut, setShortcut] = useState("Enter");
  api.setLog = setLog;
  api.setDraft = setDraft;
  api.setValue = setValue;
  api.setShortcut = setShortcut;
  return (
    <>
      <PendingDraft />
      <Editor
        id="composer"
        value={value}
        onChange={setValue}
        shortcut={shortcut}
      />
      <Editor id="log" value={log} editable={false} />
      <Editor id="other-draft" value={draft} onChange={setDraft} />
      <ChatAggregatedToolEntries
        summary="ツール呼び出し完了"
        expanded={toolsExpanded}
        onToggle={() => setToolsExpanded((value) => !value)}
        isHovered={false}
        onHoverChange={() => {}}
        entries={[
          {
            expansionKey: "a",
            summary: "検索",
            command: "rg needle",
            content: "検索結果",
            status: { status: "created" },
          },
          {
            expansionKey: "b",
            summary: "読み込み",
            content: "ファイルの内容",
            status: { status: "success" },
          },
        ]}
      />
      <ChatAggregatedToolEntries
        expanded={false}
        onToggle={() => {}}
        isHovered={false}
        onHoverChange={() => {}}
        summary="単独コマンド"
        entries={[
          {
            expansionKey: "single",
            summary: "単独コマンド",
            content: "単独の出力",
            status: { status: "success" },
          },
        ]}
      />
      <ChatCollapsedThinking
        expanded={thinkingExpanded}
        onToggle={() => setThinkingExpanded((value) => !value)}
        isHovered={false}
        onHoverChange={() => {}}
        entries={[{ expansionKey: "t", content: "確認した内容" }]}
        renderMarkdown={({ content }) => <p>{content}</p>}
      />
    </>
  );
}
createRoot(document.getElementById("root")!).render(<Harness />);
