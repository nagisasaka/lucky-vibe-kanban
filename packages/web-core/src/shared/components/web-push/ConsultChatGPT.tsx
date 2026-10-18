import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';

export type ConsultationReference = {
  projectId?: string;
  issueId?: string;
  workspaceId: string;
  sessionId: string;
  agentRunId: string;
  questionId?: string | null;
  approvalId?: string | null;
};

export function consultationPrompt(
  reference: ConsultationReference,
  origin: string
): string {
  const url = new URL(
    reference.projectId && reference.issueId
      ? `/projects/${encodeURIComponent(reference.projectId)}/issues/${encodeURIComponent(reference.issueId)}/workspaces/${encodeURIComponent(reference.workspaceId)}`
      : `/workspaces/${encodeURIComponent(reference.workspaceId)}`,
    origin
  );
  url.searchParams.set('session_id', reference.sessionId);
  url.searchParams.set('agent_run_id', reference.agentRunId);
  return [
    'LVK MCPで次の対象を参照し、質問または実行結果について相談に乗ってください。',
    reference.projectId ? `Project ID: ${reference.projectId}` : '',
    reference.issueId ? `Issue ID: ${reference.issueId}` : '',
    `Workspace ID: ${reference.workspaceId}`,
    `Session ID: ${reference.sessionId}`,
    `AgentRun ID: ${reference.agentRunId}`,
    reference.questionId
      ? `Question / Input ID: ${JSON.stringify(reference.questionId)} (kind: blocking_specification)`
      : '',
    reference.approvalId
      ? `Approval ID: ${JSON.stringify(reference.approvalId)} (kind: approval)`
      : '',
    `LVK URL: ${url.href}`,
    '取得できない場合はその旨を伝えてください。明示的に依頼するまで回答・実行承認・マージはしないでください。実行終了、テスト結果、Integration、公開は別々に確認してください。',
  ]
    .filter(Boolean)
    .join('\n');
}

/** Deliberate copy + ordinary ChatGPT URL; no unofficial auto-send deep link. */
export function ConsultChatGPT(reference: ConsultationReference) {
  const [expanded, setExpanded] = useState(false);
  const [copied, setCopied] = useState(false);
  const {
    data: context,
    isPending,
    isError,
  } = useQuery({
    queryKey: ['web-push-context', reference.workspaceId],
    queryFn: async (): Promise<{
      project_id: string;
      issue_id: string;
    } | null> => {
      const response = await fetch(
        `/api/web-push/context/${encodeURIComponent(reference.workspaceId)}`
      );
      if (!response.ok) throw new Error('LVK参照を取得できません');
      return response.json();
    },
  });
  const prompt = consultationPrompt(
    {
      ...reference,
      projectId: context?.project_id,
      issueId: context?.issue_id,
    },
    window.location.origin
  );
  const copy = async () => {
    setExpanded(true);
    try {
      await navigator.clipboard.writeText(prompt);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  };
  return (
    <div className="space-y-half text-sm">
      <button
        type="button"
        className="rounded border bg-secondary text-high px-base py-half"
        disabled={isPending}
        onClick={() => void copy()}
      >
        ChatGPTに相談 — 依頼文をコピー
      </button>
      {isError && (
        <p className="text-low">
          Projectの参照を取得できませんでした。相談文のWorkspace
          IDから確認してください。
        </p>
      )}
      {expanded && (
        <>
          <p className="text-low">
            {copied
              ? 'コピーしました。ChatGPTを開いて貼り付けてください。'
              : '下の依頼文を選択してコピーしてください。'}{' '}
            LVK MCPの接続が必要です。
          </p>
          <a
            className="text-high underline"
            href="https://chatgpt.com/"
            target="_blank"
            rel="noopener noreferrer"
          >
            ChatGPTを開く
          </a>
          <textarea
            aria-label="ChatGPTへの相談文"
            className="w-full rounded border bg-primary p-base"
            rows={5}
            readOnly
            value={prompt}
            onFocus={(e) => e.currentTarget.select()}
          />
        </>
      )}
    </div>
  );
}
