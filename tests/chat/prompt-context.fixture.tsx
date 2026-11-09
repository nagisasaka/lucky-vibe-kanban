import React, { useState } from 'react';
import { createRoot } from 'react-dom/client';
import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import { ChatUserMessage } from '../../packages/ui/src/components/ChatUserMessage';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import WYSIWYGEditor from '../../packages/web-core/src/shared/components/WYSIWYGEditor';
import { KanbanIssuePanel } from '../../packages/ui/src/components/KanbanIssuePanel';
import common from '../../packages/web-core/src/i18n/locales/ja/common.json';
import tasks from '../../packages/web-core/src/i18n/locales/ja/tasks.json';

void i18n.use(initReactI18next).init({
  lng: 'ja',
  resources: { ja: { common, tasks } },
  initImmediate: false,
});
const api = window as any;
api.initial =
  'ユーザーの元入力\n<!-- lvk:card-context:start -->\n独自の要求\n<!-- vk:pipeline:start -->\n保存済み独自段階\n<!-- vk:pipeline:end -->\n<!-- lvk:card-context:end -->';
function Harness() {
  const [content, setContent] = useState(api.initial);
  const [description, setDescription] = useState(api.initial);
  api.setContent = setContent;
  api.description = description;
  return (
    <>
      <section id="message">
        <ChatUserMessage
          content={content}
          renderMarkdown={({ content }) => (
            <pre className="whitespace-pre-wrap break-words">{content}</pre>
          )}
          onEdit={() => {
            api.edited = content;
          }}
        />
      </section>
      <section id="issue">
        <KanbanIssuePanel
          mode="edit"
          issueId="stored-card"
          displayId="LOCAL-test"
          formData={{
            title: 'Task',
            description,
            statusId: 'todo',
            priority: 'medium',
            assigneeIds: [],
            tagIds: [],
            createDraftWorkspace: false,
          }}
          onFormChange={(key, value) => {
            if (key === 'description') setDescription(value);
          }}
          statuses={[]}
          tags={[]}
          onClose={() => {}}
          onSubmit={() => {}}
          renderDescriptionEditor={(props) => <WYSIWYGEditor {...props} />}
        />
      </section>
    </>
  );
}
createRoot(document.getElementById('root')!).render(
  <QueryClientProvider client={new QueryClient()}>
    <Harness />
  </QueryClientProvider>
);
