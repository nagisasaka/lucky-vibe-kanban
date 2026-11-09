// Production presentation components with inert data and callbacks; no backend or provider execution.
import React, { useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { DragDropContext } from '@hello-pangea/dnd';
import common from '../../packages/web-core/src/i18n/locales/ja/common.json';
import { KanbanIssuePanel } from '../../packages/ui/src/components/KanbanIssuePanel';
import { IssueCommentsSection } from '../../packages/ui/src/components/IssueCommentsSection';
import { IssueRelationshipsSection } from '../../packages/ui/src/components/IssueRelationshipsSection';
import { IssueSubIssuesSection } from '../../packages/ui/src/components/IssueSubIssuesSection';
import {
  IssueTaskAttemptsSection,
  IssueAttemptOption,
} from '../../packages/ui/src/components/IssueTaskAttemptsSection';
import { ChatMarkdown } from '../../packages/ui/src/components/ChatMarkdown';
import {
  ChatBoxBase,
  VisualVariant,
} from '../../packages/ui/src/components/ChatBoxBase';
import WYSIWYGEditor from '../../packages/web-core/src/shared/components/WYSIWYGEditor';
i18n
  .use(initReactI18next)
  .init({ lng: 'ja', resources: { ja: { common } }, initImmediate: false });
const noop = () => {};
const record = (action: string) => {
  (window as any).lastAction = action;
};
const description =
  '## 読みやすい Issue 詳細\n\n左右分割では、実際のペイン幅に合わせて本文と余白を整えます。既存のセクション構成を活かし、主タイトル・本文・操作の違いを明確にします。\n\n'.repeat(
    3
  ) +
  '\n\n```ts\nconst longResult = ' +
  '"sample-value", '.repeat(35) +
  ';\n```\n\n| 対象 | 狭いペイン | 広いペイン | 備考 |\n| --- | --- | --- | --- |\n| Issue 詳細 | 適切な余白 | 読める幅 | ' +
  'long-content-'.repeat(12) +
  ' |';
const editor = (props: any) => <WYSIWYGEditor {...props} />;
function Fixture() {
  const [createMode, setCreateMode] = useState(false);
  (window as any).setCreateMode = setCreateMode;
  const ref = useRef<HTMLTextAreaElement>(null);
  const [data, setData] = useState({
    title: 'Issue 詳細のデザインと左右分割の本文幅・余白を見直す',
    description,
    statusId: 'todo',
    priority: 'high',
    assigneeIds: [],
    tagIds: [],
    createDraftWorkspace: false,
  });
  const [input, setInput] = useState('');
  return (
    <QueryClientProvider client={new QueryClient()}>
      <DragDropContext onDragEnd={noop}>
        <main className="flex h-screen bg-primary text-normal">
          <section id="issue" style={{ width: '50%', minWidth: 0 }}>
            <KanbanIssuePanel
              mode={createMode ? 'create' : 'edit'}
              displayId="LOCAL-339"
              issueId="339"
              formData={data as any}
              onFormChange={(k, v) => setData((d) => ({ ...d, [k]: v }))}
              statuses={[{ id: 'todo', name: '未着手', color: '#777' }] as any}
              tags={[]}
              onClose={noop}
              onSubmit={noop}
              titleInputRef={ref}
              renderDescriptionEditor={editor}
              renderWorkspacesSection={() => (
                <>
                  <IssueTaskAttemptsSection
                    attempts={[]}
                    onCreateWorkflowAttempt={() => record('workflow')}
                    onCreateSingleAgentAttempt={() => record('single')}
                    arenaOption={
                      <IssueAttemptOption
                        title="アリーナを開始"
                        description="複数エージェントの案を比較して選ぶ"
                        onClick={() => record('arena')}
                      />
                    }
                  />
                </>
              )}
              renderRelationshipsSection={() => (
                <IssueRelationshipsSection
                  relationships={[
                    {
                      relationshipId: '1',
                      relatedIssueId: '343',
                      relatedIssueDisplayId: 'LOCAL-343',
                      displayType: 'related',
                    },
                    {
                      relationshipId: '2',
                      relatedIssueId: '300',
                      relatedIssueDisplayId: 'LOCAL-300',
                      displayType: 'blocked_by',
                    },
                  ]}
                  onRelationshipClick={noop}
                  onRemoveRelationship={noop}
                />
              )}
              renderSubIssuesSection={() => (
                <IssueSubIssuesSection
                  parentIssueId="339"
                  subIssues={[1, 2].map((n) => ({
                    id: `sub-${n}`,
                    simpleId: `LOCAL-${340 + n}`,
                    title:
                      n === 1
                        ? 'コメント著者と境界を見分けやすくする'
                        : '長い本文とコードを分割表示で確認する',
                    priority: null,
                    statusColor: '#777',
                    assignees: [],
                    createdAt: '2026-11-06T10:00:00Z',
                    parentIssueSortOrder: n,
                  }))}
                  onSubIssueClick={noop}
                />
              )}
              renderCommentsSection={() => (
                <IssueCommentsSection
                  comments={['田中', '佐藤', '田中'].map((authorName, n) => ({
                    id: String(n),
                    authorId: null,
                    authorName,
                    message:
                      n === 1
                        ? '狭いペインでも、三つの試行方法を同じ階層で選べるようにしたいです。'
                        : '本文の行間と余白があると、長い仕様も読み進めやすくなります。\n\n次のコメントとの境界も確認します。',
                    createdAt: '2026-11-06T10:00:00Z',
                    canModify: true,
                  }))}
                  commentInput={input}
                  onCommentInputChange={setInput}
                  onSubmitComment={noop}
                  editingCommentId={null}
                  editingValue=""
                  onEditingValueChange={noop}
                  onStartEdit={noop}
                  onSaveEdit={noop}
                  onCancelEdit={noop}
                  onDeleteComment={noop}
                  reactionsByCommentId={new Map()}
                  onToggleReaction={noop}
                  onReply={() => record('reply')}
                  renderEditor={editor}
                />
              )}
            />
          </section>
          <section
            id="workspace"
            className="kanban-workspace-pane flex flex-col border-l"
            style={{ width: '50%', minWidth: 0 }}
          >
            <header className="border-b px-base py-half">
              Workspace / 会話
            </header>
            <div className="flex flex-1 min-h-0 overflow-hidden justify-center">
              <div
                className="w-[var(--kanban-chat-width,48rem)] max-w-full h-full"
                id="conversation"
              >
                <ChatMarkdown
                  content={
                    '## 実装方針\n\nペイン幅に応じた自然な既定レイアウトを検討します。\n\n' +
                    description
                  }
                  renderContent={({ content, className }) => (
                    <WYSIWYGEditor
                      disabled
                      value={content}
                      className={className}
                    />
                  )}
                />
              </div>
            </div>
            <div className="flex justify-center @container" id="composer">
              <ChatBoxBase
                visualVariant={VisualVariant.NORMAL}
                editor={
                  <textarea
                    placeholder="メッセージを入力"
                    className="w-full bg-transparent"
                  />
                }
                footerRight={<button>送信</button>}
              />
            </div>
          </section>
        </main>
      </DragDropContext>
    </QueryClientProvider>
  );
}
createRoot(document.getElementById('root')!).render(<Fixture />);
