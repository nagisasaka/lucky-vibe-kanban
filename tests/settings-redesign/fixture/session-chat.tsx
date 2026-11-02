import { AsyncAgentQuestions } from "@/features/agent-runtime/ui/AsyncAgentQuestions";
import { useState } from "react";
import {
  SessionChatBox,
  type SessionRuntimeActionPolicy,
} from "@vibe/ui/components/SessionChatBox";
export function SessionChatFixture() {
  const [mode, setMode] = useState(
    new URLSearchParams(location.search).get("mode") ?? "existing",
  );
  const [value, setValue] = useState("Raw draft 原文");
  const [events, setEvents] = useState<unknown[]>([]);
  const record = (action: string) => () =>
    setEvents((old) => [...old, { action, value }]);
  const actions = [
    "send_initial",
    "send_follow_up",
    "queue_follow_up",
    "cancel_queue",
    "stop",
    "approve",
    "request_changes",
    "answer_question",
    "retry",
    "resume",
  ];
  const policy = Object.fromEntries(
    actions.map((action) => [
      action,
      { action, allowed: false, reason: "runtime_busy" },
    ]),
  ) as SessionRuntimeActionPolicy;
  return (
    <main className="min-h-screen bg-primary text-normal p-base">
      <div>
        {[
          "existing",
          "running",
          "blocked",
          "async",
          "async-accept",
          "async-disabled",
        ].map((name) => (
          <button key={name} onClick={() => setMode(name)}>
            {name}
          </button>
        ))}
      </div>
      {mode.startsWith("async") ? (
        <AsyncAgentQuestions
          questions={[
            {
              id: "raw-question-1",
              title: "Raw question 原文",
              options: ["Raw choice 選択肢"],
            },
            {
              id: "raw-question-2",
              title: "Raw delivery question",
              options: ["Raw blocked choice"],
              delivery_status: "raw-status",
            },
          ]}
          scope="i18n-question-fixture"
          disabled={mode === "async-disabled"}
          onAnswer={async (question, answer) => {
            setEvents((old) => [...old, { id: question.id, answer }]);
            if (mode !== "async-accept")
              throw new Error("Raw provider secret: forbidden UI body");
          }}
        />
      ) : (
        <section data-testid="session-chat">
          <SessionChatBox
            status={mode === "running" ? "running" : "idle"}
            editor={{ value, onChange: setValue }}
            renderEditor={(props) => (
              <textarea
                data-testid="chat-editor"
                value={props.value}
                onChange={(event) => props.onChange(event.target.value)}
                placeholder={props.placeholder}
                disabled={props.disabled}
                className="w-full bg-primary text-normal"
                onKeyDown={(event) => {
                  if (event.ctrlKey && event.key === "Enter") {
                    event.preventDefault();
                    props.onCmdEnter();
                  }
                }}
              />
            )}
            actions={{
              onSend: record("send"),
              onQueue: record("queue"),
              onSteer: record("steer"),
              onCancelQueue: record("cancel-queue"),
              onStop: record("stop"),
              onPasteFiles: () => {},
            }}
            session={{
              sessions: [
                {
                  id: "raw-session",
                  name: "Raw session 原文",
                  created_at: "2026-11-01T00:00:00Z",
                },
              ],
              selectedSessionId: "raw-session",
              onSelectSession: () => {},
            }}
            stats={{
              filesChanged: 1234,
              linesAdded: 2345,
              linesRemoved: 1234,
              diffStatsStatus: "ready",
            }}
            actionPolicy={mode === "blocked" ? policy : undefined}
            onPrCommentClick={record("pr-comments")}
          />
        </section>
      )}
      <output data-testid="session-chat-events" className="break-all">
        {JSON.stringify(events)}
      </output>
    </main>
  );
}
