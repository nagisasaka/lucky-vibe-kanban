import { useEffect, useRef, useState } from 'react';
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

const intents: [ChatIntent, string][] = [
  ['discuss', '相談 / Discuss'],
  ['issue', 'Issue作成 / Create Issue'],
  ['implement', '実装依頼 / Implement'],
  ['change', '追加指示 / Change'],
  ['answer', '質問への回答 / Answer'],
  ['merge', 'マージ依頼 / Merge'],
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
  const navigation = useAppNavigation();
  const { theme } = useTheme();
  const queryClient = useQueryClient();
  const [content, setContent] = useState('');
  const [intent, setIntent] = useState<ChatIntent>('discuss');
  const [sending, setSending] = useState(false);
  const [sendError, setSendError] = useState<string | null>(null);
  const pending = useRef<{
    id: string;
    content: string;
    intent: ChatIntent;
  } | null>(null);
  const queryKey = ['project-chat', projectId];
  const chat = useQuery({
    queryKey,
    queryFn: () =>
      load(projectId, queryClient.getQueryData<ProjectChatView>(queryKey)),
    refetchInterval: 3000,
    refetchIntervalInBackground: false,
  });
  useEffect(() => {
    setContent('');
    setIntent('discuss');
    setSendError(null);
    pending.current = null;
  }, [projectId]);
  const project = chat.data?.project_state as ProjectState | undefined;
  const busy = chat.data?.messages.some((m) => m.status !== 'done');

  async function send() {
    if (!content.trim() || sending) return;
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
      setContent('');
      pending.current = null;
      await queryClient.invalidateQueries({ queryKey });
    } catch {
      setSendError(
        '送信結果を確認できません。同じ内容で再送すると重複を防いで確認します。 / Delivery unconfirmed; retry preserves the request ID.'
      );
    } finally {
      setSending(false);
    }
  }

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
          サーバーで OPENAI_API_KEY と LVK_ASTRA_MODEL
          を設定してください。会話は保存され、設定後に処理されます。
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
      >
        {chat.isLoading && <p>読み込み中… / Loading…</p>}
        {chat.data?.messages.map((message) => (
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
      </div>
      <AsyncAgentQuestions
        key={projectId}
        scope={`project:${projectId}`}
        questions={project?.questions ?? []}
        disabled={false}
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
      {busy && (
        <p role="status" className="text-xs text-low">
          処理中です。続けて相談を送信できます。 / Processing; you can send
          another message.
        </p>
      )}
      <form
        className="space-y-half"
        onSubmit={(event) => {
          event.preventDefault();
          void send();
        }}
      >
        <label className="flex items-center gap-base text-sm">
          依頼の種類 / Request
          <select
            value={intent}
            onChange={(event) => setIntent(event.target.value as ChatIntent)}
            className="rounded border border-border bg-primary p-half"
          >
            {intents.map(([value, label]) => (
              <option key={value} value={value}>
                {label}
              </option>
            ))}
          </select>
        </label>
        <textarea
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
          disabled={sending || !content.trim()}
          className="rounded border border-border px-base py-half text-sm disabled:opacity-50"
        >
          {sending ? '送信中…' : '送信 / Send'}
        </button>
        {sendError && (
          <p role="alert" className="text-sm">
            {sendError}
          </p>
        )}
      </form>
    </section>
  );
}
