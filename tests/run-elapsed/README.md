# Run elapsed regression

The board and each chat run footer use the same wall-clock interval: canonical
AgentRun `created_at` to the current attempt's persisted `finished_at`. Pending,
input/approval waits and retries remain inside that run's interval. Separate
runs are never added together. Goal's provider-reported `time_used_seconds`
keeps its existing accounting and remains a separate display.

Run from the repository root:

```sh
pnpm --dir packages/web-core exec vitest run src/features/agent-runtime/model src/features/workspace-chat/model/goalElapsed.test.ts src/features/workspace-chat/ui/GoalProgressCard.test.tsx src/shared/hooks/useWorkspaces.test.ts --maxWorkers=1
PLAYWRIGHT_BROWSERS_PATH=/opt/lvk-browsers node --test tests/chat/run-elapsed.test.mjs
pnpm --dir packages/local-web run build
cargo build --locked --bin server --bin agent-process-host
PLAYWRIGHT_BROWSERS_PATH=/opt/lvk-browsers node tests/run-elapsed/isolated.mjs
```

The component browser regression supplies canonical fixtures and a controlled
clock. It covers normal/Goal, running, waits, cancelling, all terminal statuses,
replay/remount, multiple runs, missing terminal evidence, and a stale active
flag with a recorded end. It is not a provider or backend integration test.

The isolated test starts this checkout's built server on an OS-assigned
loopback port with a new test-owned `LVK_INSTANCE_DIR`. It creates Project/Issue/Workspace/Session and a disposable Git repository
through the real API, inserts synthetic canonical provider records into its own
SQLite, and opens the built app in fresh Chromium pages. It observes running
board/chat clocks, the frozen previous run, completion at 42 seconds, continued
stability, and reload. API, SQLite, WebSocket, routing and UI rendering are real;
provider execution is synthetic. It does not launch an external model, use the
live dev database, or validate real provider timing accuracy.

Screenshots, observations and server logs are written to
`.build/run-elapsed-validation/`. The script always stops and waits for its
server and browser, then removes its temporary instance DB/data/runtime.
