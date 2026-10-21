import React, { useState } from 'react';
import { createRoot } from 'react-dom/client';
import { LexicalComposer } from '@lexical/react/LexicalComposer';
import { RichTextPlugin } from '@lexical/react/LexicalRichTextPlugin';
import { ContentEditable } from '@lexical/react/LexicalContentEditable';
import { LexicalErrorBoundary } from '@lexical/react/LexicalErrorBoundary';
import { CodeNode, CodeHighlightNode } from '@lexical/code';
import { CODE } from '@lexical/markdown';
import { ChatCode } from '../../packages/ui/src/components/ChatCode';
import { CodeHighlightPlugin } from '../../packages/ui/src/components/CodeHighlightPlugin';
import { MarkdownSyncPlugin } from '../../packages/ui/src/components/MarkdownSyncPlugin';
import { CODE_HIGHLIGHT_CLASSES } from '../../packages/ui/src/lib/code-highlight-theme';
import { ChatAggregatedToolEntries } from '../../packages/ui/src/components/ChatAggregatedToolEntries';

const api = window as any;
const transformers = [CODE];
function Harness() {
  const [code, setCode] = useState('');
  const [language, setLanguage] = useState<string | undefined>('bash');
  const [markdown, setMarkdown] = useState('');
  api.setCode = setCode;
  api.setLanguage = setLanguage;
  api.setMarkdown = setMarkdown;
  return (
    <>
      <input id="composer" />
      <pre id="code">
        <ChatCode code={code} language={language} />
      </pre>
      <ChatAggregatedToolEntries
        summary="command"
        entries={[
          {
            expansionKey: 'command',
            summary: 'command',
            command: code,
            content: 'plain output',
          },
        ]}
        expanded={false}
        isHovered={false}
        onToggle={() => {}}
        onHoverChange={() => {}}
      />
      <LexicalComposer
        initialConfig={{
          namespace: 'code',
          editable: false,
          nodes: [CodeNode, CodeHighlightNode],
          theme: { codeHighlight: CODE_HIGHLIGHT_CLASSES },
          onError: (e) => {
            throw e;
          },
        }}
      >
        <RichTextPlugin
          contentEditable={<ContentEditable id="markdown" />}
          placeholder={null}
          ErrorBoundary={LexicalErrorBoundary}
        />
        <MarkdownSyncPlugin
          value={markdown}
          editable={false}
          transformers={transformers}
        />
        <CodeHighlightPlugin readOnly />
      </LexicalComposer>
    </>
  );
}
createRoot(document.getElementById('root')!).render(<Harness />);
