# Claude subscription validation

`cli_fixture.py` is an offline protocol stub, not the official Claude CLI. It
never authenticates, calls a model or executes tools. Its self-checks alone do
not prove LVK interoperability:

```bash
python3 -m unittest discover -s tests/claude-subscription -p 'test_*.py'
```

The fixture supports authentication metadata, initial messages, `--resume`,
success/error results, and native interrupt. `CLAUDE_FIXTURE_HOLD` keeps the
process alive; `CLAUDE_FIXTURE_FAIL` returns an error and exits 1. Other requests
complete immediately, exercising terminal-output/process-exit ordering.
`LVK_CLAUDE_FIXTURE_STATE` names a test-owned JSON file with `loggedIn`,
`authMethod`, `apiProvider`, `exitCode` or `malformed` overrides. Optional trace
files record event kinds and session IDs, excluding prompts and environment
values.

## Integrated checks

Check free space/inodes first. Reuse this workspace's target and the canonical
sccache configuration from AGENTS.md; never clear another workspace's cache.
Use Cargo jobs at most 2 and browser workers=1. With dependencies installed:

```bash
# Preserve these profile settings for all Cargo commands in the run.
export CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
cargo test -p executors executors::claude
cargo test -p executors provider_policy
cargo test -p executors executors::codex
cargo test -p local-deployment process_host::tests
pnpm run generate-types
pnpm run generate-types:check
cargo build --bin server --bin agent-process-host
pnpm --dir packages/local-web run build
python3 tests/claude-subscription/run_isolated.py
```

The runner starts real checkout server/host binaries and Vite on separate
loopback ports, with a disposable instance/SQLite DB, `CLAUDE_CONFIG_DIR` and
`CODEX_HOME`. HOME is preserved. Only the isolated process PATH receives a
symlink named `claude`; strict subscription profiles do not use a command
override. No live LVK instance or user login is changed. The local test
instance's onboarding flags are test data, not provider consent.

Chromium exercises negative preflight and runtime admission, initial execution,
follow-up, output, failure, stop/resume, native session reuse, and the Issue →
Workspace provider/preset selection and submission UI. The runner always stops
its instance-owned processes, checks for remaining PIDs/listeners, copies
sanitised fixture traces, then removes its temporary state. Logs and cleanup
results remain under `.build/claude-subscription/e2e-*`; Playwright screenshots
and failure contexts are under `test-results/claude-subscription`.

## Live validation remains separate

The official CLI was not installed for these tests. The user must install it,
review applicable terms, and run `claude auth login` themselves under the same
execution account and configuration directory. See
[Claude setup and limitations](../../docs/agents/claude-code.mdx).

Public auth metadata does not prove a live model request, refreshability or
remaining quota. Managed settings retain precedence over per-process settings.
Confirm the actual account/configuration and observe expiration, refresh, quota
and network behaviour using the official CLI; fixture success is not live
Claude success.
