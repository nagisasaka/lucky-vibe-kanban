import { memo, useCallback, useEffect, useRef, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import type {
  ChatIntent,
  ProjectChatView,
  PendingAgentQuestion,
} from 'shared/types';
import { handleApiResponse } from '@/shared/lib/api';
import { makeLocalApiRequest } from '@/shared/lib/localApiTransport';
import { useAppNavigation } from '@/shared/hooks/useAppNavigation';
import { agentRunsApi } from '@/shared/lib/agentRunApi';
import { AsyncAgentQuestions } from '@/features/agent-runtime/ui/AsyncAgentQuestions';
import { getResolvedTheme, useTheme } from '@/shared/hooks/useTheme';
import { MarkdownPreview } from '@/shared/components/MarkdownPreview';

type ChatMode = Extract<ChatIntent, 'discuss' | 'operate'>;
const intents: [ChatMode, string][] = [
  ['discuss', '相談／RO'],
  ['operate', '操作可 / Operations allowed'],
];

async function load(
  projectId: string,
  previous?: ProjectChatView
): Promise<ProjectChatView> {
  const messages = new Map((previous?.messages ?? []).map((m) => [m.id, m]));
  const pending = previous?.messages.filter((m) => m.status !== 'done') ?? [];
  let after = pending.length
    ? Number(pending[0].sequence) - 1
    : Number(previous?.messages.at(-1)?.sequence ?? 0);
  for (;;) {
    const page = await handleApiResponse<ProjectChatView>(
      await makeLocalApiRequest(
        `/api/projects/${projectId}/chat?after=${after}`
      )
    );
    page.messages.forEach((message) => messages.set(message.id, message));
    if (page.messages.length < 200)
      return {
        ...page,
        messages: [...messages.values()].sort(
          (a, b) => Number(a.sequence) - Number(b.sequence)
        ),
      };
    after = Number(page.messages[page.messages.length - 1].sequence);
  }
}

type ProjectState = {
  questions?: PendingAgentQuestion[];
  issues?: { id: string; title: string; simple_id: string }[];
  workspaces?: { id: string; issue_id: string; name: string | null }[];
  runs?: { agent_run_id: string; workspace_id: string; status: string }[];
};

export function ProjectChat({ projectId }: { projectId: string }) {
  return <ProjectChatContent key={projectId} projectId={projectId} />;
}

function ProjectChatContent({ projectId }: { projectId: string }) {
  const [intent, setIntent] = useState<ChatMode>('discuss');
  const navigation = useAppNavigation();
  const queryClient = useQueryClient();
  const queryKey = ['project-chat', projectId];
  const chat = useQuery({
    queryKey,
    queryFn: () =>
      load(projectId, queryClient.getQueryData<ProjectChatView>(queryKey)),
    refetchInterval: 3000,
    refetchIntervalInBackground: false,
  });
  const project = chat.data?.project_state as ProjectState | undefined;
  const history = useRef<HTMLDivElement>(null);
  const historyContent = useRef<HTMLDivElement>(null);
  const follow = useRef(true);
  const [away, setAway] = useState(false);
  const toLatest = useCallback(() => {
    follow.current = true;
    setAway(false);
    if (history.current)
      history.current.scrollTop = history.current.scrollHeight;
  }, []);
  const onSent = useCallback(() => {
    toLatest();
    void queryClient.invalidateQueries({
      queryKey: ['project-chat', projectId],
    });
  }, [toLatest, queryClient, projectId]);
  useEffect(() => {
    const element = history.current;
    const contents = historyContent.current;
    if (!element || !contents) return;
    const observer = new ResizeObserver(() => {
      if (follow.current) element.scrollTop = element.scrollHeight;
    });
    observer.observe(contents);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    if (follow.current) toLatest();
  }, [chat.data?.messages, toLatest]);

  return (
    <section
      aria-label="Astra project chat"
      className="flex min-h-0 flex-1 flex-col gap-base px-double pb-double"
    >
      <p className="text-sm text-low">
        Astra — プロジェクトの相談・実装窓口 / Project conversation
      </p>
      {chat.data && !chat.data.configured && (
        <p role="status" className="text-sm text-low">
          サーバーのCodexでChatGPTにログインし、LVK_ASTRA_MODEL
          を設定してください。APIキーは不要です。会話は保存され、認証後に処理されます。
        </p>
      )}
      {chat.isError && (
        <p role="alert">
          会話を取得できません。再接続時に再取得します。 / Reconnecting…
        </p>
      )}
      <div
        className="min-h-0 flex-1 overflow-y-auto space-y-double"
        aria-label="Conversation history"
        ref={history}
        onScroll={(event) => {
          const element = event.currentTarget;
          follow.current =
            element.scrollHeight - element.scrollTop - element.clientHeight <
            64;
          setAway(!follow.current);
        }}
      >
        {chat.isLoading && <p>読み込み中… / Loading…</p>}
        <div ref={historyContent} className="space-y-double">
          <ChatHistory
            messages={chat.data?.messages}
            project={project}
            projectId={projectId}
          />
        </div>
      </div>
      {away && (
        <button type="button" onClick={toLatest} className="text-sm underline">
          最新へ ↓
        </button>
      )}
      <AsyncAgentQuestions
        key={projectId}
        scope={`project:${projectId}`}
        questions={project?.questions ?? []}
        disabled={intent !== 'operate'}
        onAnswer={async (question, answer) => {
          const target = project?.questions?.find((q) => q.id === question.id);
          if (!target) throw new Error('Question is no longer pending');
          await agentRunsApi.answerQuestion(
            target.session_id,
            target.id,
            answer
          );
          await queryClient.invalidateQueries({ queryKey });
        }}
      />
      <details className="text-sm">
        <summary className="cursor-pointer">
          Issue・Workspace・承認・差分 / Open existing work
        </summary>
        <div className="max-h-40 overflow-y-auto space-y-half">
          {project?.issues?.map((issue) => (
            <div key={issue.id}>
              <button
                type="button"
                className="underline"
                onClick={() => navigation.goToProjectIssue(projectId, issue.id)}
              >
                {issue.simple_id} {issue.title}
              </button>
              {project.workspaces
                ?.filter((w) => w.issue_id === issue.id)
                .map((workspace) => (
                  <button
                    key={workspace.id}
                    type="button"
                    className="ml-base underline"
                    onClick={() =>
                      navigation.goToProjectIssueWorkspace(
                        projectId,
                        issue.id,
                        workspace.id
                      )
                    }
                  >
                    {workspace.name ?? workspace.id} ·{' '}
                    {project.runs?.find((r) => r.workspace_id === workspace.id)
                      ?.status ?? '未確認 / Unverified'}
                  </button>
                ))}
            </div>
          ))}
        </div>
      </details>
      <ChatActivity
        messages={chat.data?.messages}
        configured={chat.data?.configured}
        disconnected={chat.isError}
      />
      <ChatComposer
        projectId={projectId}
        onSent={onSent}
        intent={intent}
        setIntent={setIntent}
      />
    </section>
  );
}

const ChatHistory = memo(function ChatHistory({
  messages,
  project,
  projectId,
}: {
  messages: ProjectChatView['messages'] | undefined;
  project: ProjectState | undefined;
  projectId: string;
}) {
  const navigation = useAppNavigation();
  const { theme } = useTheme();
  return (
    <>
      {' '}
      {messages?.map((message) => (
        <article
          key={message.id}
          className="rounded border border-border p-base"
        >
          <div className="mb-half text-xs text-low">
            {message.role === 'assistant'
              ? 'Astra'
              : message.role === 'user'
                ? 'You'
                : 'LVK'}{' '}
            · {message.created_at}{' '}
            {message.status !== 'done' && `· ${message.status}`}
            {message.role === 'user' &&
              ` · ${message.intent === 'operate' ? '操作可' : message.intent === 'discuss' ? '相談／RO' : `旧権限 / Legacy: ${message.intent}`}`}
          </div>
          {message.role === 'tool' || message.role === 'event' ? (
            <details>
              <summary className="cursor-pointer text-sm">
                {message.role === 'tool'
                  ? '操作結果 / Operation receipt'
                  : '作業の更新 / Workspace events'}
              </summary>
              <pre className="overflow-x-auto whitespace-pre-wrap break-words text-xs">
                {message.content}
              </pre>
            </details>
          ) : (
            <MarkdownPreview
              content={message.content}
              theme={getResolvedTheme(theme)}
              disableImages
            />
          )}
          {(message.role === 'tool' || message.role === 'event') && (
            <div className="flex flex-wrap gap-base text-xs">
              {project?.issues
                ?.filter((issue) =>
                  message.content
                    .replaceAll('-', '')
                    .includes(issue.id.replaceAll('-', ''))
                )
                .map((issue) => (
                  <button
                    key={issue.id}
                    type="button"
                    className="underline"
                    onClick={() =>
                      navigation.goToProjectIssue(projectId, issue.id)
                    }
                  >
                    {issue.simple_id}
                  </button>
                ))}
              {project?.workspaces
                ?.filter(
                  (workspace) =>
                    message.content
                      .replaceAll('-', '')
                      .includes(workspace.id.replaceAll('-', '')) ||
                    project.runs?.some(
                      (run) =>
                        run.workspace_id === workspace.id &&
                        message.content.includes(run.agent_run_id)
                    )
                )
                .map((workspace) => (
                  <button
                    key={workspace.id}
                    type="button"
                    className="underline"
                    onClick={() =>
                      navigation.goToProjectIssueWorkspace(
                        projectId,
                        workspace.issue_id,
                        workspace.id
                      )
                    }
                  >
                    {workspace.name ?? workspace.id}
                  </button>
                ))}
            </div>
          )}
          {message.error && (
            <p role="status" className="text-sm text-low">
              {message.error}
            </p>
          )}
        </article>
      ))}
    </>
  );
});

function ChatComposer({
  projectId,
  onSent,
  intent,
  setIntent,
}: {
  projectId: string;
  onSent: () => void;
  intent: ChatMode;
  setIntent: (mode: ChatMode) => void;
}) {
  const [content, setContent] = useState('');
  const [sending, setSending] = useState(false);
  const [sendError, setSendError] = useState<string | null>(null);
  const pending = useRef<{
    id: string;
    content: string;
    intent: ChatIntent;
  } | null>(null);
  const tooLong = new TextEncoder().encode(content).length > 32000;
  async function send() {
    if (!content.trim() || sending || tooLong) return;
    if (
      !pending.current ||
      pending.current.content !== content ||
      pending.current.intent !== intent
    ) {
      pending.current = { id: crypto.randomUUID(), content, intent };
    }
    const request = pending.current;
    setSending(true);
    setSendError(null);
    try {
      await handleApiResponse<string>(
        await makeLocalApiRequest(`/api/projects/${projectId}/chat`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            request_id: request.id,
            content: request.content,
            intent: request.intent,
          }),
        })
      );
      setContent((current) => (current === request.content ? '' : current));
      pending.current = null;
      onSent();
    } catch {
      setSendError(
        '送信結果を確認できません。再送で元の内容・モード・IDの受付を確認してください。 / Delivery unconfirmed; retry preserves the request ID.'
      );
    } finally {
      setSending(false);
    }
  }

  return (
    <form
      className="space-y-half"
      onSubmit={(event) => {
        event.preventDefault();
        void send();
      }}
    >
      <label className="flex items-center gap-base text-sm">
        モード / Mode
        <select
          value={intent}
          onChange={(event) => setIntent(event.target.value as ChatMode)}
          disabled={sending || sendError !== null}
          aria-describedby="astra-mode-description"
          className="rounded border border-border bg-primary p-half"
        >
          {intents.map(([value, label]) => (
            <option key={value} value={value}>
              {label}
            </option>
          ))}
        </select>
      </label>
      <p id="astra-mode-description" className="text-sm text-low">
        {intent === 'discuss'
          ? '読み取りと相談のみ。変更は実行しません。操作を依頼するには「操作可」に切り替えて送信してください。 / Read-only.'
          : '今回の明示的な依頼に応じて操作します。相談だけでは変更しません。 / Acts only on your direct request.'}
      </p>
      <textarea
        disabled={sending || sendError !== null}
        aria-label="Message to Astra"
        value={content}
        onChange={(event) => setContent(event.target.value)}
        maxLength={32000}
        rows={3}
        className="w-full resize-y rounded border border-border bg-primary p-base text-sm"
        placeholder="相談を続ける、実装を依頼する、作業の状況を確認する…"
      />
      <button
        type="submit"
        disabled={sending || !content.trim() || tooLong}
        className="rounded border border-border px-base py-half text-sm disabled:opacity-50"
      >
        {sending ? '送信中…' : '送信 / Send'}
      </button>
      {tooLong && (
        <p role="alert">
          メッセージは32,000バイト以内にしてください。 / Maximum 32,000 bytes.
        </p>
      )}
      {sendError && (
        <p role="alert" className="text-sm">
          {sendError}
        </p>
      )}
    </form>
  );
}

function ChatActivity({
  messages,
  configured,
  disconnected,
}: {
  messages: ProjectChatView['messages'] | undefined;
  configured: boolean | undefined;
  disconnected: boolean;
}) {
  const active = messages?.find((message) => message.status !== 'done');
  const [now, setNow] = useState(Date.now);
  const activeId = active?.id;
  useEffect(() => {
    if (!activeId) return;
    setNow(Date.now());
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [activeId]);
  if (!active) return null;
  const raw = active.created_at;
  const since = Date.parse(
    /[zZ]|[+-]\d{2}:\d{2}$/.test(raw) ? raw : `${raw.replace(' ', 'T')}Z`
  );
  const seconds = Number.isFinite(since)
    ? Math.max(0, Math.floor((now - since) / 1000))
    : null;
  const label = disconnected
    ? '接続を確認中（最新の実行状態は未確認）'
    : !configured
      ? 'モデル設定・認証待ち'
      : active.status === 'running'
        ? 'Astra が処理中…'
        : active.error
          ? '接続エラー・再試行待ち'
          : '順番待ち…';
  return (
    <div className="text-sm text-low">
      <p role="status" className="flex items-center gap-half">
        {!disconnected && configured && active.status === 'running' && (
          <span
            aria-hidden="true"
            className="inline-block size-3 animate-spin rounded-full border-2 border-current border-t-transparent motion-reduce:animate-none"
          />
        )}
        {label}
      </p>
      {seconds !== null && (
        <p>
          受付から {Math.floor(seconds / 60)}分{seconds % 60}秒
        </p>
      )}
      <p>続けて相談を送信できます。</p>
    </div>
  );
}
