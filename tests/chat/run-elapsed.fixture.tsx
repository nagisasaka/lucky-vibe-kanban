import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { ChatElapsedTime } from "../../packages/ui/src/components/ChatElapsedTime";
import { IssueWorkspaceCard } from "../../packages/ui/src/components/IssueWorkspaceCard";
import { projectCanonicalAgentConversation } from "../../packages/web-core/src/features/agent-runtime/model/canonicalAgentConversation";
import { buildCanonicalAgentSessionTimeline } from "../../packages/web-core/src/features/agent-runtime/model/canonicalAgentSessionTimeline";
import { GoalProgressCard } from "../../packages/web-core/src/features/workspace-chat/ui/GoalProgressCard";

void i18n
  .use(initReactI18next)
  .init({ lng: "en", resources: {}, initImmediate: false });
const api = window as any;
function Harness() {
  const [runs, setRuns] = useState<any[]>([]);
  const [key, setKey] = useState(0);
  api.setRuns = setRuns;
  api.remount = () => setKey((value) => value + 1);
  return (
    <main key={key}>
      {runs.map((run) => {
        const state = run.state;
        const active = ![
          "succeeded",
          "failed",
          "cancelled",
          "crashed",
          "audit_failed",
        ].includes(state.status);
        const projection = projectCanonicalAgentConversation(
          buildCanonicalAgentSessionTimeline("session", [run]),
        );
        return (
          <section key={run.agent_run_id} id={run.agent_run_id}>
            <div className="board">
              <IssueWorkspaceCard
                workspace={{
                  id: run.agent_run_id,
                  localWorkspaceId: null,
                  name: run.agent_run_id,
                  archived: false,
                  filesChanged: 0,
                  linesAdded: 0,
                  linesRemoved: 0,
                  prs: [],
                  owner: null,
                  updatedAt: run.updated_at,
                  isOwnedByCurrentUser: true,
                  isRunning: active,
                  latestProcessStatus: state.status,
                  latestProcessStartedAt: run.created_at,
                  latestProcessCompletedAt: run.completed_at,
                }}
              />
            </div>
            <div className="chat">
              {projection.entries
                .filter((entry) => entry.runTiming)
                .map((entry) => (
                  <ChatElapsedTime key={entry.patchKey} {...entry.runTiming!} />
                ))}
            </div>
            {state.goal && (
              <GoalProgressCard
                goal={state.goal}
                isRunActive={active}
                onPause={() => {}}
                onResume={() => {}}
                onEdit={() => {}}
                onClear={() => {}}
                onInterruptTurn={() => {}}
              />
            )}
          </section>
        );
      })}
      <div id="stale">
        <ChatElapsedTime
          startedAt="2026-10-20T00:00:00Z"
          endedAt="2026-10-20T00:00:12Z"
          active
        />
      </div>
      <div id="unknown">
        <ChatElapsedTime startedAt="2026-10-20T00:00:00Z" active={false} />
      </div>
    </main>
  );
}
createRoot(document.getElementById("root")!).render(<Harness />);
