import React, { useState } from 'react';
import { createRoot } from 'react-dom/client';
import i18next from 'i18next';
import { initReactI18next } from 'react-i18next';
import { TerminalWindowIcon } from '@phosphor-icons/react';
import {
  ChatAggregatedToolEntries,
  type AggregatedEntry,
} from '../../packages/ui/src/components/ChatAggregatedToolEntries';
import { ChatThinkingMessage } from '../../packages/ui/src/components/ChatThinkingMessage';
import jaTasks from '../../packages/web-core/src/i18n/locales/ja/tasks.json';
import enTasks from '../../packages/web-core/src/i18n/locales/en/tasks.json';
import jaCommon from '../../packages/web-core/src/i18n/locales/ja/common.json';
import enCommon from '../../packages/web-core/src/i18n/locales/en/common.json';

type Example = { id: string; title: string; entries: AggregatedEntry[] };
const command = '  printf "a\\_b\\n"\t\n  pnpm test  ';
const make = (
  id: string,
  status: string,
  content: string,
  outputState: AggregatedEntry['outputState'] = 'complete'
): AggregatedEntry => ({
  summary: command,
  command,
  expansionKey: id,
  status: { status },
  active: status === 'created',
  content,
  outputState,
  ...(status === 'failed'
    ? { exitCode: 1, durationMs: 1250, endedAt: '2026-10-25T09:00:04Z' }
    : {}),
});
export const examples: Example[] = [
  {
    id: 'failure',
    title: '失敗と大量出力',
    entries: [
      make(
        'failure',
        'failed',
        [
          'start',
          ...Array.from({ length: 20 }, (_, index) => `checking ${index}`),
          'Error: expected 2, received 1',
          '  at test.ts:42',
          ...Array.from({ length: 20 }, (_, index) => `cleanup ${index}`),
          'exit 1',
        ].join('\n')
      ),
    ],
  },
  {
    id: 'empty',
    title: '完了・空出力',
    entries: [make('empty', 'success', '')],
  },
  {
    id: 'cancelled',
    title: '明示的キャンセル',
    entries: [make('cancelled', 'cancelled', '', 'unavailable')],
  },
  {
    id: 'unknown',
    title: '終了イベント未受信',
    entries: [
      { ...make('unknown', 'created', '', 'unavailable'), active: false },
    ],
  },
  {
    id: 'running',
    title: '実行中・途中出力',
    entries: [make('running', 'created', 'first\nsecond\n', 'partial')],
  },
  {
    id: 'denied',
    title: '拒否',
    entries: [make('denied', 'denied', '', 'unavailable')],
  },
  {
    id: 'mixed',
    title: '集約に失敗を含む',
    entries: [
      make('ok', 'success', 'ok'),
      make('bad', 'failed', 'Error: regression\nexit 1'),
      make('pending', 'created', '', 'pending'),
    ],
  },
  {
    id: 'json',
    title: '構造化された操作と結果',
    entries: [
      {
        summary: 'Read src/example.ts',
        expansionKey: 'json',
        status: { status: 'success' },
        content: '{"ok": true, "literal": "a\\\\_b"}',
        outputState: 'complete',
        language: 'json',
        input: '{"file_path": "src/example.ts"}',
      },
    ],
  },
];

function ExampleRow({ example }: { example: Example }) {
  const [expanded, setExpanded] = useState(false);
  return (
    <section
      id={example.id}
      className="mb-double rounded-sm border border-border bg-panel p-base"
    >
      <h2 className="mb-base text-sm text-normal">{example.title}</h2>
      <ChatAggregatedToolEntries
        entries={example.entries}
        expanded={expanded}
        onToggle={() => setExpanded((value) => !value)}
        isHovered={false}
        onHoverChange={() => {}}
        icon={
          example.entries[0].command !== undefined
            ? TerminalWindowIcon
            : undefined
        }
      />
    </section>
  );
}

function Harness() {
  const [cases, setCases] = useState(examples);
  const [dark, setDark] = useState(false);
  (window as unknown as { setCases: typeof setCases }).setCases = setCases;
  return (
    <main className={dark ? 'dark' : ''}>
      <div
        className="bg-primary p-base text-normal"
        style={{ height: '100vh', display: 'flex', flexDirection: 'column' }}
      >
        <header className="mb-base flex flex-wrap items-center gap-base">
          <h1 className="text-base">LOCAL-32 実行表示・固定fixture</h1>
          <button
            id="theme"
            aria-pressed={dark}
            onClick={() => {
              document.documentElement.classList.toggle('dark', !dark);
              setDark((value) => !value);
            }}
          >
            ライト／ダーク
          </button>
        </header>
        <p className="mb-base text-xs text-low">
          固定イベントの表示確認用。実provider接続・実コマンド実行は行いません。
        </p>
        <div
          id="history"
          className="min-w-0 overflow-y-auto"
          style={{ flex: 1 }}
        >
          {cases.map((example) => (
            <ExampleRow key={example.id} example={example} />
          ))}
          <section id="notes" className="p-base">
            <ChatThinkingMessage
              content="1. Inspect source\n2. Run tests"
              label="計画"
              provider="Codex"
              active={false}
              renderMarkdown={({ content }) => (
                <pre className="whitespace-pre-wrap">{content}</pre>
              )}
            />
          </section>
        </div>
        <label className="mt-base text-xs">
          入力保持の確認
          <textarea
            id="composer"
            className="w-full rounded-sm border border-border bg-panel p-half text-normal"
          />
        </label>
      </div>
    </main>
  );
}

void i18next
  .use(initReactI18next)
  .init({
    lng: 'ja',
    fallbackLng: 'en',
    resources: {
      ja: { tasks: jaTasks, common: jaCommon },
      en: { tasks: enTasks, common: enCommon },
    },
  })
  .then(() => createRoot(document.getElementById('root')!).render(<Harness />));
