# REVIEW mode regression checks

REVIEW is a Codex execution mode on the ordinary CODE conversation path. The
checks keep execution mode separate from sandbox/approval policy and do not use
PLAN approval, Goal continuation, automatic Issue comments or a new report format.

Run the focused frontend checks:

```bash
node --test tests/review-mode/selector.test.mjs tests/chat/executor-config.test.mjs
pnpm --dir packages/web-core exec vitest run src/shared/lib/executorOverrides.test.ts src/features/workspace-chat/model/goalValidation.test.ts src/features/workspace-chat/model/resumeGoal.test.ts
```

The selector fixture uses the real mode selector and executor-config hook, with
offline discovery and localStorage persistence. Chromium checks 1440px and 390px,
selection, remount, reload and submitted configuration, including CODE/PLAN/GOAL.

For the full application path, build the workspace's server and process host,
then run the isolated fixture:

```bash
CARGO_BUILD_JOBS=4 node scripts/dev-cargo.mjs build --bin server --bin agent-process-host
python3 tests/review-mode/run_isolated.py
```

Choose build parallelism from current memory/CPU pressure. The runner starts this
checkout's server, process host and Vite with a disposable instance/SQLite DB,
CODEX_HOME and workspace directory. It never uses the shared development DB or
artifacts. Only the provider is replaced. Chromium changes Code to Review,
observes API scratch persistence, reloads, submits a real follow-up and reads the
saved configuration and normal terminal report. Sanitised provider observations
verify REVIEW instructions on resume, workspace-write sandbox, default
collaboration mode, exactly two normal turns (initial CODE plus REVIEW) and no
Goal activation. Cleanup checks owned processes and ports before deleting the
temporary instance. Small logs remain in `.build/review-mode/` for diagnosis.

The Rust regressions cover start/resume instruction and permission parity,
ordinary completion without PLAN/Goal/commit continuation, source-finalisation
suppression, stdio schema/configuration handoff and nullable/error responses,
remote launch/read and immutable Session configuration. MCP approval protocol
fixtures run the same approval, denial, cancellation and malformed-request cases
in CODE and REVIEW.

These fixtures do not prove real-model obedience, real external OAuth/MCP
interoperability, native sandbox enforcement or improved external approvals.
The ability to keep investigation files uncommitted is separate from permission
to adopt, merge, publish or deploy product changes.
