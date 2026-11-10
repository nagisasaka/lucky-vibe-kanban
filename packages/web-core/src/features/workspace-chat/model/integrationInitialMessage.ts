import type { IntegrationRun } from 'shared/types';

type SavedInput = {
  agent_run_id: string;
  input: { message_id: string; content: string };
};
export function savedIntegrationInput(run: IntegrationRun): SavedInput | null {
  const dispatch = run.payload.worker_dispatch;
  if (!dispatch || typeof dispatch !== 'object' || Array.isArray(dispatch))
    return null;
  const value = dispatch as unknown as {
    type?: string;
    data?: { request?: SavedInput };
  };
  const request = value.type === 'create' ? value.data?.request : null;
  return request &&
    typeof request.agent_run_id === 'string' &&
    typeof request.input?.message_id === 'string' &&
    typeof request.input.content === 'string'
    ? request
    : null;
}

export function isIntegrationInitialMessage(
  run: IntegrationRun,
  agentRunId: string | undefined,
  messageId: string | undefined,
  content: string
): boolean {
  const saved = savedIntegrationInput(run);
  return (
    !!saved &&
    agentRunId === saved.agent_run_id &&
    messageId === saved.input.message_id &&
    content === saved.input.content
  );
}
