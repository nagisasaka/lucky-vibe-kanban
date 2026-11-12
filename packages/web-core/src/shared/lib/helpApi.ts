import type {
  AgentRunHistoryPage,
  AgentRunPortSnapshot,
  AgentRunSummary,
  Session,
} from 'shared/types';
import { handleApiResponse } from './api';
import { makeLocalApiRequest } from './localApiTransport';

export interface HelpSource {
  path: string;
  build_revision: string;
  commit: string;
  history_scope: string;
}
export interface HelpConversation {
  workspace_id: string;
  session_id: string;
  runs: AgentRunSummary[];
  history: AgentRunHistoryPage | null;
  questions: {
    agent_run_id: string;
    prompt: string;
    source_commit?: string | null;
    build_revision?: string | null;
  }[];
  source: HelpSource;
}
interface AskHelpResponse {
  workspace_id: string;
  session_id: string;
  run: AgentRunPortSnapshot;
  source: HelpSource;
}

async function request<T>(
  path: string,
  method = 'GET',
  body?: unknown
): Promise<T> {
  return handleApiResponse<T>(
    await makeLocalApiRequest(`/api/help/conversations${path}`, {
      method,
      headers: { 'Content-Type': 'application/json' },
      ...(body ? { body: JSON.stringify(body) } : {}),
    })
  );
}

export const helpApi = {
  list: () => request<Session[]>(''),
  get: (sessionId: string) => request<HelpConversation>(`/${sessionId}`),
  ask: (prompt: string, sessionId?: string) =>
    request<AskHelpResponse>('', 'POST', { prompt, session_id: sessionId }),
  cancel: (sessionId: string) => request<void>(`/${sessionId}/cancel`, 'POST'),
  delete: (sessionId: string) => request<void>(`/${sessionId}`, 'DELETE'),
};
