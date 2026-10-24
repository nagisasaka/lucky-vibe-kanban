import React from "react";
import { createRoot } from "react-dom/client";
import { LexicalComposer } from "@lexical/react/LexicalComposer";
import { RichTextPlugin } from "@lexical/react/LexicalRichTextPlugin";
import { ContentEditable } from "@lexical/react/LexicalContentEditable";
import { LexicalErrorBoundary } from "@lexical/react/LexicalErrorBoundary";
import { TRANSFORMERS } from "@lexical/markdown";
import { HeadingNode, QuoteNode } from "@lexical/rich-text";
import { ListNode, ListItemNode } from "@lexical/list";
import { CodeNode } from "@lexical/code";
import { LinkNode } from "@lexical/link";
import { PasteMarkdownPlugin } from "../../packages/ui/src/components/PasteMarkdownPlugin";

createRoot(document.getElementById("root")!).render(
  <LexicalComposer
    initialConfig={{
      namespace: "browser-paste",
      nodes: [
        HeadingNode,
        QuoteNode,
        ListNode,
        ListItemNode,
        CodeNode,
        LinkNode,
      ],
      onError: (error) => {
        throw error;
      },
    }}
  >
    <RichTextPlugin
      contentEditable={<ContentEditable aria-label="Editor" />}
      placeholder={null}
      ErrorBoundary={LexicalErrorBoundary}
    />
    <PasteMarkdownPlugin transformers={TRANSFORMERS} />
  </LexicalComposer>,
);
