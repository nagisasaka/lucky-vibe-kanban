import React, { useState } from 'react';
import { createRoot } from 'react-dom/client';
import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import { ChatUserMessage } from '../../packages/ui/src/components/ChatUserMessage';
import { splitPromptContext } from '../../packages/ui/src/lib/splitPromptContext';
import { CardContextSection } from '../../packages/web-core/src/features/pipeline/ui/CardContextSection';
import { defaultCardContext } from '../../packages/web-core/src/features/pipeline/model/cardContext';
import common from '../../packages/web-core/src/i18n/locales/ja/common.json';
import tasks from '../../packages/web-core/src/i18n/locales/ja/tasks.json';

void i18n.use(initReactI18next).init({
  lng: 'ja',
  resources: { ja: { common, tasks } },
  initImmediate: false,
});
const api = window as any;
api.split = splitPromptContext;
api.initial = defaultCardContext(
  'ユーザーの元入力\n\n```text\n例示の本文\n```',
  []
);
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
      <section id="settings">
        <CardContextSection
          description={description}
          pipelines={[]}
          onRetry={() => {}}
          onDescriptionChange={setDescription}
        />
      </section>
    </>
  );
}
createRoot(document.getElementById('root')!).render(<Harness />);
