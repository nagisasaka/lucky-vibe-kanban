# Terminal-to-next-action regression

TODO: `a530ff5c-1fd4-4d1b-bc71-e3efa071ae9a`.

Measured before/after evidence: [RESULTS.md](RESULTS.md).

The Codex control handle retains `AppServerClient`, whose `LogWriter` owns the
replacement stdout pipe. Holding that handle while waiting for stdout EOF makes
the process host wait for the audit drain deadline even after provider cleanup.
Release the unused control handle **after** existing process cleanup and **before**
draining output. Buffered output, audit close, durable terminal publication,
backend admission guards, and the five-second drain deadline remain in place.

The production path is:

1. Codex JSON-RPC completion / process exit (`executors/.../codex/jsonrpc.rs`).
2. Process cleanup, output drain, Native Audit close and host terminal journal
   (`local-deployment/src/process_host.rs`).
3. Atomic canonical projection and terminal broadcast
   (`local-deployment/src/agent_run_port.rs`, `finish_host_projection` acknowledges
   the host after projection).
4. AgentRun WebSocket and `useAgentRunCanonicalStream` feed
   `AgentRunSessionProvider`; `SessionChatBoxContainer` derives active controls.
5. The next request still passes session/workspace ownership and active-run guards.

## Reproduce

From the repository root, with the repository toolchain and dependencies installed:

```sh
cargo test -p local-deployment process_host::tests -- --nocapture
pnpm --dir packages/local-web run build
cargo build --bin server --bin agent-process-host
python3 tests/terminal-latency/run.py
```

The runner uses the current workspace binaries and built frontend, a fresh
`LVK_INSTANCE_DIR`, separate fixture folders and ephemeral loopback listeners.
It never targets an existing app or modifies scheduler settings. Chromium defaults
to the container's installed binary; override with `LVK_E2E_CHROMIUM` if needed.
The runner verifies instance processes have stopped before removing its temporary
DB/runtime. Diagnostic logs/journals remain under
`test-results/terminal-latency-instance/` (Git ignored).

Only the external Codex app-server is mocked by `codex-fixture.mjs`; no model,
authentication service or external tools run. The actual Codex adapter, process
host, process cleanup, audit files, SQLite backend, HTTP/WebSocket transport,
React application and Chromium are used. This is not real Codex service validation.

The browser checks completion, failure, cancellation, automatic WebSocket reconnection (real frames forwarded
with one injected close), page reload after completion, rejection of follow-up while active, and successful next-run admission
through the Send button. Provider terminal emission, browser receipt of terminal
WebSocket data and the DOM's first enabled Send state are timestamped. The reload case
includes navigation/loading and is not a steady-connection latency sample. The
`canonicalAt` field is the test's HTTP observation time, not DB commit time; host
journal event timestamps are retained separately. No product sleeps are changed.

The Rust regression uses a real OS pipe retained by a control handle and a real
NativeAuditWriter. It verifies EOF arrives without reaching the drain deadline,
the buffered tail is audited before terminal, and process exit without provider completion evidence
remains `crashed` (exit 0) or `failed` (nonzero). Existing host tests cover bounded cancellation and replay /
reconnection ordering.
