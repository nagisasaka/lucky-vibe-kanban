// Deterministic Codex app-server stand-in. Only the external provider is mocked;
// the adapter, process host, audit, DB, WebSocket and browser are production code.
import { createInterface } from "node:readline";
import {
  watch,
  existsSync,
  readFileSync,
  appendFileSync,
  readlinkSync,
} from "node:fs";
import { join } from "node:path";
import { randomUUID } from "node:crypto";

const emit = (value) => process.stdout.write(`${JSON.stringify(value)}\n`);
let cwd;
let threadId = randomUUID();
let turnId;
let watcher;
const turn = (status) => ({
  id: turnId,
  items: [],
  status,
  error:
    status === "failed"
      ? {
          message: "fixture failure",
          codexErrorInfo: null,
          additionalDetails: null,
        }
      : null,
});
const finish = (status) => {
  if (!turnId) return;
  appendFileSync(
    join(cwd, "observations.jsonl"),
    `${JSON.stringify({
      stage: "provider_terminal",
      parentExecutable: readlinkSync(`/proc/${process.ppid}/exe`),
      status,
      at: new Date().toISOString(),
    })}\n`,
  );
  emit({
    method: "item/completed",
    params: {
      threadId,
      turnId,
      completedAtMs: Date.now(),
      item: {
        type: "agentMessage",
        id: randomUUID(),
        text: "LATENCY_FIXTURE_DONE",
        phase: "final_answer",
      },
    },
  });
  emit({ method: "turn/completed", params: { threadId, turn: turn(status) } });
  turnId = null;
  watcher?.close();
};
for await (const line of createInterface({ input: process.stdin })) {
  const req = JSON.parse(line);
  const reply = (result) => emit({ id: req.id, result });
  switch (req.method) {
    case "initialize":
      reply({
        userAgent: "codex/0.144.1",
        codexHome: process.cwd(),
        platformFamily: "unix",
        platformOs: "linux",
      });
      break;
    case "initialized":
      break;
    case "thread/start":
    case "thread/resume": {
      cwd = req.params.cwd;
      threadId = req.params.threadId ?? threadId;
      reply({
        thread: {
          id: threadId,
          sessionId: threadId,
          preview: "",
          ephemeral: false,
          modelProvider: "openai",
          createdAt: 1,
          updatedAt: 1,
          status: { type: "idle" },
          path: null,
          cwd,
          cliVersion: "0.144.1",
          source: "cli",
          agentNickname: null,
          agentRole: null,
          gitInfo: null,
          name: null,
          turns: [],
        },
        model: "fixture",
        modelProvider: "openai",
        serviceTier: null,
        cwd,
        approvalPolicy: "never",
        approvalsReviewer: "user",
        sandbox: { type: "dangerFullAccess" },
        reasoningEffort: null,
      });
      break;
    }
    case "turn/start": {
      turnId = randomUUID();
      reply({ turn: turn("inProgress") });
      emit({
        method: "turn/started",
        params: { threadId, turn: turn("inProgress") },
      });
      const release = join(cwd, "release");
      const check = () => {
        if (existsSync(release)) finish(readFileSync(release, "utf8").trim());
      };
      watcher = watch(cwd, check);
      check();
      break;
    }
    case "turn/interrupt":
      reply({});
      finish("interrupted");
      break;
    case "model/list":
      reply({ data: [], nextCursor: null });
      break;
    case "skills/list":
      reply({ data: [] });
      break;
    case "config/read":
      reply({ config: {}, origins: {}, layers: [] });
      break;
    case "account/read":
      reply({ account: null, requiresOpenaiAuth: false });
      break;
    case "thread/inject_items":
    case "thread/injectItems":
      reply({});
      break;
    default:
      if (req.id !== undefined)
        emit({
          id: req.id,
          error: {
            code: -32601,
            message: `Unsupported fixture method: ${req.method}`,
          },
        });
  }
}
