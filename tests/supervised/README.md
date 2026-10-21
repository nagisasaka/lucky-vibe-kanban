# Supervised approval regression

This fixture replaces only the Codex app-server process. It describes command
and file requests but executes neither. The isolated browser test runs the
workspace's real server, process host, provider adapter, JSON-RPC client,
canonical event store, AgentRun control API, WebSocket stream and existing UI.
It does not contact a model or prove native sandbox enforcement.

Run heavyweight steps sequentially. Check the resource-coordination snapshot
before using shared infrastructure; this runner uses a fresh temporary instance,
SQLite DB and three dynamically selected loopback ports. It never uses live
instance state. It explicitly pins `VIBE_KANBAN_AGENT_PROCESS_HOST` to this checkout
and discards inherited LVK/VK/process-host overrides. It preserves HOME and uses
a disposable CODEX_HOME.

```bash
# Use the repository's sccache configuration when available.
CARGO_BUILD_JOBS=2 cargo test -p executors executors::codex
CARGO_BUILD_JOBS=2 cargo test -p local-deployment process_host
CARGO_BUILD_JOBS=2 cargo test -p services services::approvals
pnpm install --frozen-lockfile
CARGO_BUILD_JOBS=2 cargo build --bin server --bin agent-process-host
python3 tests/supervised/run_isolated.py
```

Chromium must be installed for the repository's Playwright version. The runner
starts Vite from this checkout and uses the real backend without API mocks.
Logs and the fixture's exact request/response trace go to `.build/supervised-approval/e2e-*`.
Its `finally` block stops its server, browser ancestry, process hosts and fixture
children, checks ports/processes, and removes temporary instance data. A failed
check is not a pass. The temporary workspace/profile is never the user's profile.

The Python-backed Rust protocol fixture runs on Unix; Windows native execution is
not covered here. Rust protocol cases include accept, reject, denial feedback, cancellation,
missing/failing service, explicit full access and the old Noop injection for both request
types. Host tests reuse the real approval bridge for pending, early/late/duplicate
answers and cancellation; bridge tests cover timeout and cancellation races.

The browser checks pending command/diff visibility, reload while waiting,
accept/deny/stop, two simultaneous requests and full access. It also checks
canonical native references/checksums and the original native response IDs.
See [Change Manifest](../../docs/development/supervised-approval-change-manifest.md)
for actual runs, initial failures and verification limits.
