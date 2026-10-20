# Formal Integration dispatch recovery

## Current source: host-owned merge engine (2026-10-20)

New Integrations use Git for every ordinary merge. AI is created only when
`merge-tree` reports a conflict. The full conflict instruction and immutable
Create identity are saved before dispatch; there is no Integration-wide
Orchestration node, reused AgentRun, or Retry/Steer step hand-off. Each conflict
job gets a fresh AgentRun in the same Integration Workspace/Session. It stages
resolutions; the host verifies the merge identity/index, creates the merge commit,
records its OID before moving the private branch, and requests candidate-bound
human review. Approval continues the Git loop without another agent.

Git operations preserve parents and selected source OIDs. Already included
sources need no extra commit. A durable candidate intent allows an interrupted
installation to be recognised without a second merge. Unexpected dirty files,
HEAD, MERGE_HEAD, or an unsettled writer stop progress; they are never reset.
Known conflict metadata cleanup after a successful branch CAS is idempotent.
Publication still uses the existing exact-candidate intent/CAS and recovery path.
Starting projection persistence is also a provider-launch prerequisite: a failed
write does not launch a provider or mark an unexecuted attempt as executed.

### Validation configuration

Unless validation was explicitly skipped at admission, the frozen target commit
must contain `.lvk/integration-validation.json`, for example:

```json
{"commands":["pnpm run check","pnpm run lint","pnpm run backend:test"]}
```

Commands run from the repository root and are required. Missing/malformed config
rejects admission. The configuration is read from the exact target Git object,
not the current worktree, source branches, AI response, or historical test logs.
Repository prerequisites belong in this list too: LVK's checked-in configuration
builds the real frontend before its normal check/lint/backend-test gates.
The entire plan is frozen at admission and runs once on the final candidate;
prefix steps record merge/review evidence, not a claim that each prefix was tested.
Existing exact-command/commit execution evidence and explicit validation gates
remain in force. Test failure does not start an AI repair loop. An explicit
`skip_validation=true` reads no plan, executes no checks, and reports unverified.

### Existing retained Integration and deployment boundary

**Do not execute the old cutover scripts or Resume the retained four-source run
as a way to adopt this implementation.** Those scripts pin historical hashes.
Use only the newly prepared operator bundle below; historical scripts remain obsolete.

Old payloads default to `mechanical=false`. Their audit remains readable and
publication recovery remains available, but this engine refuses their ordinary
Resume with `LEGACY_INTEGRATION`. It does not reconstruct projections, advance
step indices, reuse historical Create/Steer commands, or silently migrate them.
Explicit Cancel remains available through the existing safe writer settlement.
There is no second legacy execution engine to maintain.

For `c68ce019-9349-4679-976f-326120191c08`, preserve candidate
`8d0eda6b75cce0d0930a17c46d43c5d162f58e7b`, the three accepted steps and scoped
human approval, frozen main/source commits and `skip_validation=true`. A separate
reviewed checkpoint migration must verify those facts plus exited writers and
settled historical commands before permitting mechanical step 4. The limited adoption endpoint described below implements this check. It has
been tested on an isolated copy, but **has not been executed on the live instance**.
Never flip the payload flag by direct SQLite editing. The old two-source `0771cd56...` must not be resumed.

The explicit `POST /api/integrations/{id}/adopt-checkpoint` action accepts
`project_id`, a stable `request_id`, `candidate` and required `dry_run`. It supports
only the retained run, exact candidate and frozen payload SHA256
`d29ed8880dc32be3eba3e8727af1d4217ae314bf07d864734666b4d7c7e85e6c`.
It checks the original merge parents/source refs, clean private checkout,
review scope and resolution diff, skipped validation and exited providers.
Unknown or pending command delivery refuses adoption. The exact historical
failed Steer remains failed; no command or Agent projection is replayed/repaired.

Dry-run changes no database rows. Execution settles a still-cancelling old product
through its existing completion API, archives the complete original payload/error/
Agent identity, and uses a payload compare-and-swap to adopt the checkpoint. The
same request is idempotent, including interruption after product settlement.
Adoption stays **blocked**: separate formal Resume is required and recomputes step 4.
The old product's terminal audit outcome is not reused as new Integration success.
For the observed startup settlement (parent cancelled, node succeeded), adoption
requires saved events109/110 and the succeeded historical child link, then preserves
those terminal records without rewriting them or appending another completion.

The regression suite exercises real temporary Git repositories and isolated
SQLite: clean cumulative merges, duplicate installation/completion, interrupted
conflict cleanup, unresolved/dirty/moved state, durable approval/replay/staleness,
frozen validation and skip policy, conflict Create identity, and legacy refusal.
These boundary tests do not claim live provider/browser or live cutover coverage.

### Source validation

The original engine commit `47a6fb40` passed these checks (2026-10-20):

- `cargo test --locked --offline -p server --lib routes::integrations`: 25 tests.
- `cargo test --locked --offline -p local-deployment --lib agent_run_port::tests`:
  20 tests, including failed Starting persistence and idempotent redelivery.
- `cargo test --locked --offline -p db --lib models::integration`: 17 tests.
- `cargo clippy --locked --offline -p server -p local-deployment -p db --all-targets -- -D warnings`.
- `cargo run --locked --offline --bin generate_types` and the same command with
  `-- --check`.
- `pnpm run web-core:check`, `pnpm run local-web:check`, and
  `pnpm --dir packages/web-core exec vitest run src/features/kanban/ui/IntegrationGateControls.test.tsx --maxWorkers=1`
  (4 tests).
- Scoped ESLint/Prettier for `IntegrationPanel.tsx`, changed Rust file formatting,
  and `git diff --check`.

Rust checks ran sequentially with two build jobs and the existing workspace
target/cache. That original check did not exercise a complete provider/host scenario.
The additional isolated scenarios below cover that gap; browser testing remains separate. SQLx metadata regeneration/check was
unavailable because `sqlx-cli` is not installed; this change introduces no schema
migration or compile-time query macro change. The normal workspace `target/`,
installed dependencies and retained real frontend `dist/` remain in place; the
currently serving debug binary depends on that `dist/`.

## Historical evidence (not executable instructions)

The full investigation and old operator commands remain in Git at
`39ffe48e50b854dc56deb1d422e4c816d3e1e395`. Retired tracked cutover scripts
remain retrievable at `47a6fb40bf93f0a3ef64531beefe10a98e41314c`.
Their instance-specific hashes are obsolete. Existing external prepared files,
backups and execution logs must remain untouched.

- `45bd9581`: targeted Create acceptance fixed the mistaken assumption that
  delivering an unrelated outbox item meant this AgentRun had started.
  Orchestration event read-to-write contention was reproduced with SQLite code 5;
  the scoped transaction uses BEGIN IMMEDIATE. The historical lock holder is unknown.
- `33587794`: Create acceptance preceded Codex thread readiness; an immediate
  Steer failed with “Codex has no active thread”. The host's subsequent Cancel
  explained the cancelled child. This was distinct from successful integration.
- `0a5cef88`: retry startup reused an input event ID across attempts, causing
  projection degradation despite a successful provider result. Per-attempt event
  identity and narrowly evidenced reconstruction were added. Product-completion
  transaction contention was separately reproduced and fixed.
- At 14:54 UTC the step-3 approval for `8d0eda6b` was saved. A later Starting
  write failed with code 5, but the old provider launch continued with its original
  step-1 input. The approved checkpoint was not lost; the repeated instruction
  came from the reused Agent attempt. No historical lock holder was established.
- `47a6fb40`: host-owned merges removed the per-step Retry/Steer hand-off. Conflict
  jobs get a fresh complete instruction; Starting persistence gates launch.

Keep common Workflow delivery/SQLite fixes and their regression tests. Legacy
projection reconstruction is still called by generic Retry and is not dead code.
Retirement requires a separate decision about historical runs. No audit facts,
source histories, external backups or canonical OpenWiki were removed by this
cleanup.

## Prepared operator handoff

Bundle: `/var/tmp/local9-mechanical-5dbe-prepared/`. This is a private, one-time
operator deliverable outside the repository; its exact source commit, server
SHA256/size and script hashes are recorded in `build.json`. Do not reuse it after
source, binary, launcher, frontend, Git refs or the frozen payload changes.

After this AgentRun and all other jobs have ended, use an independent SSH TTY as
`appuser` in the same container, with no new submissions during maintenance:

```bash
python3 /var/tmp/local9-mechanical-5dbe-prepared/cutover.py --execute
```

The program verifies the serving ELF and ownership/namespace, frozen refs/payload,
launchers/assets and idle state; captures the current startup environment privately;
saves the old ELF/startup files; stops only the verified PID with SIGTERM; takes a
stopped WAL-aware SQLite backup; atomically replaces the ELF; and invokes the same
`start.sh`. Startup checks require process identity, listener ownership, HTTP health
and the new startup log. No force-kill, automatic DB restore, adoption or Resume.
These checks do not lock out new user submissions: the maintenance window is required.

Durable outcome is in `/var/tmp/local9-cutover-mechanical-5dbe/`:

- `SUCCESS_NO_RESUME` / exit 0: updated and healthy, Integration still frozen.
- `ABORT_OR_STOP_UNCONFIRMED_OPERATOR_CHECK` / exit 2: inspect before any action;
  never assume the old process stopped.
- `ROLLED_BACK_NO_DB_RESTORE` / exit 3: executable restored and old health checked.
- `MANUAL_RECOVERY_REQUIRED_NO_FORCE_KILL` / exit 4: preserve all evidence; confirm
  PID/listener ownership before recovering. The verified old executable is
  `server.old`; startup files and private launch environment are alongside it.

The recovery directory refuses repeated execution. Do not delete it to rearm the
script. Automatic rollback covers startup/health failure in this cutover; it does
not undo a later adoption or publication. No DB/schema migration is added by this
source. Manual rollback requires confirmed idle/stopped state, the verified old
ELF and the saved environment; never overwrite an unknown running executable or
restore the DB over newer work.

After successful cutover, perform a fresh read-only MCP check. Checkpoint adoption
is then a separate operator action, followed by an explicit UI Resume:

```bash
python3 /var/tmp/local9-mechanical-5dbe-prepared/adopt.py --check
python3 /var/tmp/local9-mechanical-5dbe-prepared/adopt.py --execute
```

Both use the same saved request identity. `--check` only tests eligibility;
`--execute` adopts but does not Resume. Neither command was run against live data
in this preparation. Keep `main` at the frozen base until formal publication.

The opt-in Rust scenario tests require a fresh isolated `LVK_INSTANCE_DIR` whose
basename starts with `local9-scenario-`; run each in a separate process. The normal
scenario uses real Git/SQLite and the production host entrypoints, with deterministic
provider events by default or real Codex when `LVK_INTEGRATION_REAL_PROVIDER=1`.
The retained-checkpoint scenario additionally requires `LVK_LEGACY_FIXTURE`, a private
scoped snapshot, and clones Git objects into independent refs. It never boots the
live database. Native host tokens/PIDs in the test fixture are replaced; no live
provider can be signalled. Test results and build evidence are retained in
`target/local9-preparation/` and summarised in the operator bundle.

### Verification for this preparation

- Integration unit/regression selection: 25 passed, 2 explicitly isolated scenarios ignored by default.
- Both isolated scenarios were then run explicitly and passed: real Codex conflict resolution → human approval → next source → publication; retained checkpoint rejection checks → interrupted adoption recovery → idempotent adoption → formal Resume → source 4 publication.
- The 14 standalone cutover mock-service tests passed, including executable rollback, stale PID, changed pins/payload, busy jobs and repeated execution refusal.
- Server all-target Clippy with `-D warnings` passed; changed Rust files were formatted.

These scenarios exercise production host entrypoints with real Git and isolated
SQLite, not browser/MCP end-to-end automation. The real Codex scenario performs
actual provider execution. No live cutover, checkpoint adoption or Resume was run.
The operator bundle's build manifest records the later build and read-only live
preflight result.

### Operator preflight abort and new preparation (16:42 UTC)

The first mechanical updater aborted during preflight, before binary backup or
server stop. Preserve `/var/tmp/local9-cutover-mechanical-c179/` and the old
`/var/tmp/local9-mechanical-c179-prepared/`; do not delete state to rearm it.
The generic historical RuntimeError did not record the exact failed guard.

A residual Codex app-server (PID 1077248, started 15:17:43 UTC) independently
violated the all-process idle guard. Its own trace contained only initialization,
no thread identity or turn start; it had no registered Agent host or children.
Its pipes remained connected to the old server. After explicit operator
authorisation and identity/evidence rechecks, this one process exited on SIGTERM.
The server PID/ELF, frozen refs and Integration payload remained unchanged.
Discovery cancellation is a possible origin, not a proven root cause of the leak.

The new `d02` bundle uses a separate one-time recovery directory and the same
verified source21d26f6a/server artifact. It reports fixed guard reasons (table/count
or process PID/name) without dumping environment or remote exception bodies.
Running Codex/build/host processes remain forbidden; exited zombies do not count
as executing processes, while durable DB/host settlement checks still apply.
It neither kills residual processes nor bypasses them automatically.
The current diagnostic AgentRun must finish before operator execution.

The new bundle passed 17 isolated tests (14 service scenarios plus 3 process/guard
checks) and AST/compile validation. Live read-only preflight reached the database
activity check and reported `Active work: table=agent_runs count=1`, identifying
this diagnostic Run. No `d02` recovery state or live cutover was created.

### Startup reconciliation and retained terminal audit

The `d02` operator cutover reached the new server on 2026-10-20 at 16:52:33 UTC.
Normal startup reconciliation accepted the old succeeded child, recorded node
Succeeded (event109), and settled the cancelling parent as Cancelled (event110).
The old updater wrongly required a cancelling parent after startup and rolled back
the executable. Its stopped backup and logs remain at
`/var/tmp/local9-cutover-mechanical-d02/`; do not restore that DB or rerun old bundles.

The current `5dbe` bundle pins the now-terminal historical parent/node and their
evidence, rejects every nonterminal orchestration, and preserves the Integration
payload/base/sources unchanged. The adoption endpoint separately verifies that
same settlement before retaining the old terminal audit. This is not acceptance
of the old child as a completed Integration or permission to bypass review.

The isolated retained-checkpoint scenario now imports the real historical Agent
link and invokes `OrchestrationService::reconcile_startup` twice before adoption.
It verifies the exact transition, rejects missing settlement evidence, adopts,
explicitly resumes source4, publishes only in the independent Git clone, and
checks that historical terminal states/events remain unchanged.
The prior fixture omitted the link and did not exercise startup; the prior mock
service tests alone did not establish real startup compatibility.

Verification for this correction: the explicit startup/adoption scenario passed;
25 scoped Integration regressions passed (2 opt-in scenarios ignored in that
selection); server all-target Clippy with `-D warnings`, scoped rustfmt/diff checks,
and 19 isolated updater tests passed. The ordinary real-Codex conflict scenario
was not repeated because this correction changes only historical adoption.
The bundle manifest records the subsequent exact-source build and live read-only
preflight. No real cutover/adoption/Resume is executed by this preparation.
