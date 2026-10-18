import { useState } from "react";
import {
  AgentRunSessionProvider,
  useAgentRunSession,
} from "../../../packages/web-core/src/features/agent-runtime/model/AgentRunSessionContext";
function Status() {
  const session = useAgentRunSession();
  return (
    <>
      <div data-testid="loading">{String(session.isLoading)}</div>
      <div data-testid="history">{String(session.isLoadingHistory)}</div>
      <div data-testid="loaded">
        {session.timeline?.runs.filter((run) => run.timeline?.state).length ??
          0}
      </div>
      <div data-testid="session">{session.timeline?.sessionId}</div>
    </>
  );
}
export function History() {
  const [session, setSession] = useState("long");
  return (
    <>
      <button onClick={() => setSession("other")}>Other session</button>
      <AgentRunSessionProvider sessionId={session}>
        <Status />
      </AgentRunSessionProvider>
    </>
  );
}
