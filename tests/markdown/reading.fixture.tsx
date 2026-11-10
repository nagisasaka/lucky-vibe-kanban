// Real body renderers and Issue panel, with inert callbacks. No backend or live data.
import React, { useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { DragDropContext } from '@hello-pangea/dnd';
import common from '../../packages/web-core/src/i18n/locales/ja/common.json';
import WYSIWYGEditor from '../../packages/web-core/src/shared/components/WYSIWYGEditor';
import { MarkdownPreview } from '../../packages/web-core/src/shared/components/MarkdownPreview';
import { KanbanIssuePanel } from '../../packages/ui/src/components/KanbanIssuePanel';

i18n
  .use(initReactI18next)
  .init({ lng: 'ja', resources: { ja: { common } }, initImmediate: false });
const source = `# Markdown本文の読みやすさ

本文幅を保ち、文字・段落・引用の違いで読み進められるようにします。これは本番データを使わない表示確認用の本文です。

> [!DECISION]
> **プレビューで比較した後、文字サイズを採用しますか？**
>
> 人間が選ぶ必要のある事項だけを示します。本文を自動分類しません。

> **人間の意図**
> 外側の幅と余白を保ち、長い文章を読みやすくしたい。

## 日本語の見出し

通常の段落と **強調**、[安全なリンク](https://example.com)、[危険なリンク](javascript:alert%281%29)、ファイル操作用の \`src/example.ts\` を確認します。

- 一つ目の要件
- 二つ目の要件
    - 入れ子の項目

> [!WARNING]
> 削除すると元に戻せない操作は、実行前に対象を確認してください。

> [!NOTE]
> 通常の補足情報です。

> [!TIP]
> キーボードでも目次から移動できます。

> [!IMPORTANT]
> 保存済みの本文は自動で変更しません。

> [!CAUTION]
> このサンプルは危険な操作を実行しません。

## 日本語の見出し

${'長い本文でも見出し間を移動できます。'.repeat(14)}

\`\`\`ts
const result = "${'long-code-'.repeat(50)}";
\`\`\`

| 対象 | 表示 | 備考 | 補足 |
| --- | --- | --- | --- |
| 本文 | 読みやすい文字 | ${'long-table-'.repeat(22)} | 横スクロール |

### 最後の要件

結論・主要要件・人間の判断待ちは折りたたみません。
`;
const lineBreakSource = `# 改行と引用の比較

SoftOne
SoftTwo

HardOne\x20\x20
HardTwo

> QuoteOne
> QuoteTwo

${['DECISION', 'IMPORTANT', 'NOTE', 'TIP', 'WARNING', 'CAUTION'].map((type) => `> [!${type}]\n> ${type}First\n> ${type}Second\n>\n> ${type}Paragraph`).join('\n\n')}
`;
const attachments = [
  {
    path: '.lvk/attachments/sample.txt',
    proxy_url: 'https://fixture.invalid/sample.txt',
    file_name: 'sample.txt',
    size_bytes: 10,
    format: 'TXT',
    mime_type: 'text/plain',
  },
];
const client = new QueryClient();
const noop = () => {};
function Fixture() {
  const [value, setValue] = useState(source);
  const [title, setTitle] = useState('本文と目次');
  const [key, setKey] = useState('first');
  const [editing, setEditing] = useState(false);
  const [theme, setTheme] = useState<'light' | 'dark'>('light');
  const titleRef = useRef<HTMLTextAreaElement>(null);
  const api = window as any;
  api.source = source;
  api.lineBreakSource = lineBreakSource;
  api.changes ??= [];
  api.changeBody = setValue;
  api.changeTitle = setTitle;
  api.switchDocument = (next = 'second') => setKey(next);
  api.setEditing = setEditing;
  api.setTheme = setTheme;
  api.currentValue = value;
  api.sourceAttachment = '[sample.txt](.lvk/attachments/sample.txt)';
  return (
    <QueryClientProvider client={client}>
      <DragDropContext onDragEnd={noop}>
        <main
          className="flex bg-primary text-normal"
          style={{ height: '100vh' }}
        >
          <section
            id="lexical"
            style={{ width: '50%', minWidth: 0, overflow: 'auto' }}
          >
            <WYSIWYGEditor
              value={value}
              onChange={(v) => {
                api.changes.push(v);
                setValue(v);
              }}
              disabled={!editing}
              readingStyle="document"
              outline
              contentKey={key}
              localAttachments={attachments}
              preserveCopySource
              findMatchingDiffPath={(text) =>
                text === 'src/example.ts' ? text : null
              }
              onCodeClick={(path) => {
                api.fileOpened = path;
              }}
            />
          </section>
          <section
            id="preview"
            style={{ width: '50%', minWidth: 0, overflow: 'auto' }}
          >
            <MarkdownPreview
              content={value}
              theme={theme}
              outline
              contentKey={key}
            />
          </section>
        </main>
        <section
          id="issue"
          className="bg-secondary text-normal"
          style={{ height: 900, width: 390 }}
        >
          <KanbanIssuePanel
            mode="edit"
            issueId={key}
            displayId="LOCAL-368"
            titleInputRef={titleRef}
            formData={{
              title,
              description: value,
              statusId: 'todo',
              priority: 'medium',
              assigneeIds: [],
              tagIds: [],
              createDraftWorkspace: false,
            }}
            statuses={[{ id: 'todo', name: '未着手', color: '#777' }] as any}
            tags={[]}
            onClose={noop}
            onSubmit={noop}
            onFormChange={(field, next) => {
              if (field === 'description') api.issueChanges = next;
              if (field === 'title') setTitle(next);
            }}
            renderDescriptionEditor={(props) => <WYSIWYGEditor {...props} />}
          />
        </section>
        <section id="composer">
          <WYSIWYGEditor value="入力欄のサイズは維持" />
        </section>
      </DragDropContext>
    </QueryClientProvider>
  );
}
createRoot(document.getElementById('root')!).render(<Fixture />);
