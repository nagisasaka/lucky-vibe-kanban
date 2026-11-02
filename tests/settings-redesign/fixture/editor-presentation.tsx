import { LexicalComposer } from "../../../packages/web-core/node_modules/@lexical/react/LexicalComposer";
import { RichTextPlugin } from "../../../packages/web-core/node_modules/@lexical/react/LexicalRichTextPlugin";
import { ContentEditable } from "../../../packages/web-core/node_modules/@lexical/react/LexicalContentEditable";
import { LexicalErrorBoundary } from "../../../packages/web-core/node_modules/@lexical/react/LexicalErrorBoundary";
import { HistoryPlugin } from "../../../packages/web-core/node_modules/@lexical/react/LexicalHistoryPlugin";
import { ListPlugin } from "../../../packages/web-core/node_modules/@lexical/react/LexicalListPlugin";
import {
  ListNode,
  ListItemNode,
} from "../../../packages/web-core/node_modules/@lexical/list";
import {
  SlashCommandTypeaheadPlugin,
  type SlashCommandDescriptionLike,
} from "@vibe/ui/components/SlashCommandTypeaheadPlugin";
import { StaticToolbarPlugin } from "@vibe/ui/components/StaticToolbarPlugin";
import { ToolbarPlugin } from "@vibe/ui/components/ToolbarPlugin";
import { TypeaheadOpenProvider } from "@vibe/ui/components/TypeaheadOpenContext";

const commands: SlashCommandDescriptionLike[] = [
  {
    name: "raw-native",
    description: "Provider 原文 native",
    support_level: "native",
  },
  {
    name: "raw-skill",
    description: "Provider 原文 skill",
    support_level: "skill",
  },
];
export function EditorPresentationFixture() {
  return (
    <div className="p-base bg-primary text-normal">
      <LexicalComposer
        initialConfig={{
          namespace: "i18n-presentation",
          nodes: [ListNode, ListItemNode],
          onError: (error) => {
            throw error;
          },
        }}
      >
        <HistoryPlugin />
        <ListPlugin />
        <RichTextPlugin
          contentEditable={
            <ContentEditable
              data-testid="editor"
              className="min-h-32 border border-border p-base mt-double"
            />
          }
          placeholder={null}
          ErrorBoundary={LexicalErrorBoundary}
        />
        <StaticToolbarPlugin />
        <ToolbarPlugin />
        <TypeaheadOpenProvider>
          <SlashCommandTypeaheadPlugin
            enabled
            commands={commands}
            isInitialized
          />
        </TypeaheadOpenProvider>
      </LexicalComposer>
    </div>
  );
}
