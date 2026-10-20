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
No new server artifact/cutover is prepared by this change.

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
settled historical commands before permitting mechanical step 4. That migration
is **not implemented or executed here**. Never flip the payload flag by direct
SQLite editing. The old two-source `0771cd56...` must not be resumed.

Operator follow-up: review source/tests and legacy handling, decide and implement
the explicit retained-checkpoint migration if recovery is desired, then build a
new immutable artifact and prepare a newly pinned SSH cutover. No deployment,
service restart, live DB change, Integration action or main publication is part
of this source implementation. A deployment of this source alone does not
restore the retained run.

The regression suite exercises real temporary Git repositories and isolated
SQLite: clean cumulative merges, duplicate installation/completion, interrupted
conflict cleanup, unresolved/dirty/moved state, durable approval/replay/staleness,
frozen validation and skip policy, conflict Create identity, and legacy refusal.
These boundary tests do not claim live provider/browser or live cutover coverage.

### Source validation

The following checks passed for this implementation (2026-10-20):

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
target/cache. No full live Integration, provider execution, frontend browser
test, deployment build or cutover was run. SQLx metadata regeneration/check was
unavailable because `sqlx-cli` is not installed; this change introduces no schema
migration or compile-time query macro change. The normal workspace `target/`,
installed dependencies and retained real frontend `dist/` remain in place; the
currently serving debug binary depends on that `dist/`.

## Historical dispatch recovery notes

All cutover commands and Resume advice below describe earlier builds and are
historical. They are not instructions for this implementation.

A Create command being queued, or a global outbox delivery returning success,
does not prove that its AgentRun exists. The dispatcher must settle the specific
Create, verify its durable run/node/agent link, and query that AgentRun before
returning `Started`. A cancelled parent's discarded Create is not acceptance.
Concurrent delivery waits on the same command; replay preserves the original
idempotency key and AgentRun identity.

## LOCAL-9 recovery boundary

The retained four-source Integration is
`c68ce019-9349-4679-976f-326120191c08`. Its frozen policy is
`payload.skip_validation=true`. The old two-source Integration
`0771cd56-3d0a-45e2-98fc-4c467734f7d8` must not also be resumed.
This source fix does not deploy a server, resume either run, or publish Git.

Before an authorised operator resumes the four-source run:

1. Review and validate this fix, then deploy a server artifact containing it
   through the approved deployment procedure. Preserve the current database and
   retained Integration Workspace. Do not edit SQLite or manually merge sources.
2. Keep the repository's target `main` at the run's frozen base
   `b385a2d2d1a8683fbdcb8d931356d34f4264169c` during this bootstrap. Building a
   fixed server from a reviewed source snapshot is separate from publishing
   that snapshot to this repository's target branch. Merging the fix to `main`
   first would invalidate the retained run's target precondition. If the target
   has already advanced, stop and obtain an explicit recovery decision.
3. Read the retained run again. Require unpublished state, no publication
   intent, unchanged selections, `skip_validation=true`, and settled writers.
   Inspect delivery/AgentRun evidence rather than assuming an absent link means
   no process exists. A `delivering` command requires reconciliation.
4. Open its Integration Workspace and use the existing **Resume** action
   (`POST /api/integrations/{id}/intervene` with the actual project ID, a fresh
   request UUID and optional instructions). Reuse that request UUID if the
   response is lost. Do not create a replacement Integration. Resume reopens the
   existing orchestration and reuses the saved Create identity when no Agent
   was accepted; the corrected dispatcher checks its own delivery.
5. Check that the saved AgentRun exists and is linked to this run. Observe the
   formal source steps, any required conflict review, and final publication.
   Skipping validation does not bypass ancestry, clean candidate, review,
   target consistency or publication protections. Report the result as
   unverified; never describe skipped tests as passed.

The frozen source commits, in order, are:

- LOCAL-3: `7922bf778fa5837762f056bcf7322b5191d4d0ab`
- LOCAL-4: `4a11c7a304891f1cfe95f9daecd4a5e319de51d0`
- LOCAL-6: `b8f954948d615e66866bba85b5deb6882e71d55e`
- LOCAL-7: `40487d16a091b0f67eb24ecd011e507a5f42b8db`

A restart alone is not a code fix and does not explicitly resume a blocked
Integration. Publication recovery (`/recover`) is a different operation from
pre-publication Resume. If cancellation is chosen instead, use the formal
Cancel action; a planned identity can only be discarded when durable evidence
shows an unaccepted command under a terminal parent, with no in-flight delivery.

## Resuming implementation after an agent interruption

Keep this worktree's diff and commit history. Inspect the previous AgentRun's
terminal error and confirm whether its build/test processes have settled before
reusing `target/`. Re-run only interrupted or invalidated checks; a command that
started without a recorded completion is not a pass. Never infer deployment,
Integration completion or publication from a source commit or test result.

The earlier implementation run `95c64c33-571f-4cdd-9eab-b450b825aa20`
ended with a recorded Codex `provider error`, not a recorded test failure.
The production SQLite lock holder in the earlier Integration is unknown;
a deterministic isolated contention test establishes the read-to-write upgrade
hazard without claiming to identify that historical connection.

## SQLite change and limits

`OrchestrationEventRecord::append_and_project` now begins with
`BEGIN IMMEDIATE`. It obtains the writer reservation before reading the event
and projection, so a competing writer can be waited for using the existing busy
timeout instead of failing a deferred read-to-write upgrade. The isolated
DELETE-journal, four-connection regression test first reproduces SQLite code 5
and then checks that `CommandQueued` waits for a held writer and persists once.
The previous implementation failed this regression; the corrected one passes.

This is deliberately limited to the demonstrated event-persistence path.
Global journal mode, connection pool size, timeout settings and unrelated
transactions are unchanged. Persistent storage contention can still exhaust
the timeout and must remain a reported failure, not a waived validation result.

## Validation record

The following checks ran in the LOCAL-9 worktree, using isolated test databases
and mock AgentRun ports. They did not start production agents or resume the
retained Integration.

| Check | Result |
| --- | --- |
| `cargo test -p db --lib command_event_waits_for_writer -- --nocapture` before the locking fix | Expected failure: SQLite code 5 in `CommandQueued` persistence |
| `cargo test -p db --lib models::orchestration::tests` after the fix | 10 passed |
| `cargo test -p services -p server --lib dispatch_tests` | 7 passed; server target compiled, no server tests match this filter |
| `cargo test -p services -p server --lib routes::integrations::runtime::tests` | 19 passed |
| `cargo clippy -p db -p services -p server --all-targets -- -D warnings` | Passed; all selected targets type-checked without warnings |
| Scoped `rustfmt --check` and `git diff --check` | Passed |
| `pnpm run prepare-db:check` | Could not run migrations: `cargo-sqlx` is not installed. Temporary database cleaned by the script. No compile-time SQL query macros or schema were changed. |

The regression coverage includes unrelated/obsolete pending commands, concurrent
delivery, replay after acceptance before link persistence, cancelled parents,
missing AgentRun state, explicit Integration resume with the same identity and
skip policy, and cancellation of an unaccepted planned identity. Source tests do
not establish that a deployed server contains the fix or that publication has
succeeded.

## Development instance cutover handoff — 2026-10-20

Historical first-hotfix handoff. After the 12:53 Resume failure, use the second-failure
section below to update binary and payload preconditions; do not execute these
old assertions unchanged.

**Preparation only: none of the stop, backup, install, start or rollback commands
below were executed. Run the cutover from an independent operator SSH session,
never from this server's MCP or an AgentRun it hosts.** Finish this AgentRun and
all other active agents, workflows, resource operations and builds first. Review
resource coordination again at cutover time; the preparation snapshot was empty.
Do not resume either Integration as part of this procedure.

### Inspected deployment and candidate

The dev instance is `/var/tmp/lvk-instances/dev` (appuser, mode 0700).
PID 270619, owned by appuser, started on 2026-10-18 13:57:43 UTC, has PPID 1
and PID=PGID=SID. Both its executable directory and cwd are the retained checkout
`/var/tmp/vibe-kanban/worktrees/1f9a-chatgpt/lucky-vibe-kanban`.
This is a Git worktree sharing `/repos/lucky-vibe-kanban/.git`, **not an independent
repository copy**. Never reset, checkout or build in that deployment checkout.
Its current source HEAD is `49c0482165fec154e9cf605937c724cf8e13f8a2`;
`deployed-build.json` instead records `45b6a4fb5d783e19a0b818694bd703737f687db4`.
Neither proves the executable's source. The observed running executable SHA256 is
`b6ecc36d59dfda72eef9c9d07894f8c6a6339ebd59f60bed176009cc1d144521`.

`start.sh` requires appuser, obtains `start.lock`, refuses occupied port 4020,
then uses `nohup setsid python3 run.py`. `run.py` writes `server.pid`, changes to
the retained checkout and execs `target/debug/server`. It preserves inherited
environment plus its explicit public-mode settings, binds 127.0.0.1:4020 and sets
`VIBE_KANBAN_AGENT_PROCESS_HOST` to that checkout's `target/debug/agent-process-host`.
The source installs a SIGTERM handler and graceful HTTP shutdown. There is no
active Supervisor process; the present `control.sh`/Supervisor configuration is
not the stop/start path for this PID. Do not use `control.sh restart`.

Active log: `/var/tmp/lvk-instances/dev/logs/server.log`; `server.log` at instance
root is a different historical file. Settings and SQLite are `data/config.json`
and `data/db.v2.sqlite`, mode 0600. Preserve the whole private `data/` tree,
`runtime/`, existing environment and run scripts. Do not print environment or
credential files. The existing launch does not require sudo; SSH must enter the
same filesystem/process namespace as appuser. The external host/container SSH
entry command was not discoverable here and is an operator prerequisite.

Candidate source is exactly `45bd958184f3513e10efa1ac5a9793d5109aae59`.
Candidate path:
`/var/tmp/lvk-instances/dev/runtime/worktrees/d546-local-3-4-integr/lucky-vibe-kanban/target/debug/server`.
Build command, from that workspace root:

```bash
RUSTC_WRAPPER=/usr/local/bin/sccache \
SCCACHE_DIR=/var/tmp/lvk-instances/dev/runtime/shared/lucky-vibe-kanban-40384b04-3346-4f93-a96e-8b42696b0aaa/cache/sccache \
CARGO_BUILD_JOBS=2 cargo build --locked --offline -p server --bin server
```

The existing sccache daemon reports `/home/appuser/.cache/sccache`, differing
from the requested directory; it was not stopped or reconfigured. The own
workspace target was reused, with 439 GiB free disk and 13 GiB available RAM
before building. No concurrent Cargo build was observed.

This is a **binary-only replacement with a retained external asset dependency**.
Debug rust-embed resolves frontend files from the candidate's compile-time
workspace `packages/local-web/dist`, not the deployment cwd. Retain that workspace
and its dist after the switch; do not rebuild/clean it while serving. Existing
real frontend assets were copied there (992 files, 77 MiB), not regenerated from
missing node_modules. Their manifest digest (sorted relative path/SHA256 pairs,
Python JSON encoding) is
`944540d7eb8081ac35cc53c06a9a86bf3f6f9bf53e470273ee17286efffbe599`.
Tracked `packages/`, `shared/`, executor/agent-host sources and DB migrations have
no differences between LOCAL-9 and either recorded deployed commit or current
deployment checkout HEAD, and those deployment paths are clean. This supports
reusing the old agent-process-host and UI; their original build provenance is
not independently certified. No migration change is introduced by LOCAL-9.
Do not replace the agent-process-host, frontend in the live checkout or run.py.

### Operator SSH cutover

Use Bash with the same established startup environment. If its equivalence is
unknown, resolve it before stopping. Do not source arbitrary `.env` files.
First inspect the current PID/executable again and confirm no other user of this
binary or checkout and no automatic service manager. Stop if reality differs.
The following variables are used in all blocks, including rollback:

```bash
set -euo pipefail
umask 077
test "$(id -un)" = appuser
instance=/var/tmp/lvk-instances/dev
source_root=/var/tmp/lvk-instances/dev/runtime/worktrees/d546-local-3-4-integr/lucky-vibe-kanban
live=/var/tmp/vibe-kanban/worktrees/1f9a-chatgpt/lucky-vibe-kanban/target/debug/server
candidate="$source_root/target/debug/server"
backup="$instance/backup-local9-$(date -u +%Y%m%dT%H%M%SZ)"
export instance backup
test "$(sha256sum "$candidate" | cut -d' ' -f1)" = 3e37ca967d7faf0cdf0737568a8243b838b84b163fc18d68dd6152efde53c4d5
test "$(sha256sum "$live" | cut -d' ' -f1)" = b6ecc36d59dfda72eef9c9d07894f8c6a6339ebd59f60bed176009cc1d144521
test "$(git -C "$source_root" rev-parse refs/heads/main)" = b385a2d2d1a8683fbdcb8d931356d34f4264169c
pid=$(cat "$instance/server.pid")
test "$(readlink -f "/proc/$pid/exe")" = "$live"
ps -p "$pid" -o pid,ppid,pgid,sid,user,lstart,comm
mkdir -m 700 "$backup"
cp -p "$live" "$backup/server.old"
cp -p "$instance/start.sh" "$instance/run.py" "$instance/deployed-build.json" "$backup/"
```

Record and compare the Integration payload using this read-only check before
stop and again after startup (also in rollback). This verifies the entire frozen
payload, including selections and validation policy, not just the status:

```bash
python3 - <<'PY'
import hashlib, json, sqlite3, uuid
c = sqlite3.connect('file:/var/tmp/lvk-instances/dev/data/db.v2.sqlite?mode=ro', uri=True)
r = c.execute('SELECT status,payload,updated_at FROM integration_runs WHERE id=?',
              (uuid.UUID('c68ce019-9349-4679-976f-326120191c08').bytes,)).fetchone()
assert r and r[0] == 'blocked'
p = json.loads(r[1])
assert p['skip_validation'] is True and p['published'] is False
assert not p['publication_intent'] and p['result_commit'] is None
assert p['base_commit'] == 'b385a2d2d1a8683fbdcb8d931356d34f4264169c'
assert hashlib.sha256(r[1].encode()).hexdigest() == 'e6ad3a8ecdc52e66368a560042f6c08a50ece913a30f89908b51d08c885a9191'
print('Frozen Integration unchanged:', r[0], r[2])
PY
```

After operator confirmation that all writers/agents are idle, graceful stop:

```bash
kill -TERM "$pid"
for attempt in $(seq 1 60); do
  test ! -e "/proc/$pid" && break
  sleep 1
done
test ! -e "/proc/$pid"  # If this fails: STOP; do not force-kill or switch.
python3 - <<'PY'
import socket
with socket.socket() as s:
    assert s.connect_ex(('127.0.0.1', 4020)) != 0, 'Listener still present'
PY
```

Back up the stopped private data tree, then use SQLite backup API to obtain a
consistent independent database, including any committed WAL pages. Require all
other DB writers stopped; do not copy only the live `.sqlite` file:

```bash
cp -a "$instance/data" "$backup/data"
python3 - <<'PY'
import os, sqlite3
src = sqlite3.connect('file:' + os.environ['instance'] + '/data/db.v2.sqlite?mode=ro', uri=True)
dst = sqlite3.connect(os.environ['backup'] + '/db.v2.consistent.sqlite')
src.backup(dst)
assert dst.execute('PRAGMA integrity_check').fetchone()[0] == 'ok'
dst.close(); src.close()
PY
# Atomic rename on the live binary's filesystem; never truncate its old inode.
test ! -e "$live.local9-new"
install -m 755 "$candidate" "$live.local9-new"
test "$(sha256sum "$candidate" | cut -d' ' -f1)" = "$(sha256sum "$live.local9-new" | cut -d' ' -f1)"
mv -T "$live.local9-new" "$live"
"$instance/start.sh"
```

Health and identity checks (startup request alone is not success):

```bash
curl --fail --silent --show-error --retry 10 --retry-connrefused --retry-delay 2 \
  -H 'Host: dev.luckyvibekanban.com' http://127.0.0.1:4020/api/health
curl --fail --silent --show-error https://dev.luckyvibekanban.com/api/health
newpid=$(cat "$instance/server.pid")
test "$(readlink -f "/proc/$newpid/exe")" = "$live"
test "$(sha256sum "/proc/$newpid/exe" | cut -d' ' -f1)" = "$(sha256sum "$candidate" | cut -d' ' -f1)"
test "$(git -C "$source_root" rev-parse refs/heads/main)" = b385a2d2d1a8683fbdcb8d931356d34f4264169c
```

Repeat the frozen-payload check above. Using the existing authenticated LVK MCP
connection, call **only** `get_integration(project_id="2e066b19-db27-4b8a-943e-ea097aa601c3",
 integration_id="c68ce019-9349-4679-976f-326120191c08", section="summary")`.
Require blocked, four sources, unpublished, result_commit=null, validation.total=0.
Open the authenticated UI and verify JS/CSS load; inspect new startup log entries
locally without sharing secrets. HTTP health alone does not test Agent dispatch.
Only after operator acceptance may a separate authorised action formally Resume
the retained four-source Integration. Leave `0771cd56-...` untouched.

### Immediate rollback

If startup, UI, MCP or identity checks fail, admit no new work. Stop the new PID
with the same verified-executable, SIGTERM and 60-second exit/listener checks.
If it does not exit, stop the procedure and investigate; never overwrite or
restore data beneath a running process. Then:

```bash
test ! -e "$live.local9-rollback"
install -m 755 "$backup/server.old" "$live.local9-rollback"
mv -T "$live.local9-rollback" "$live"
"$instance/start.sh"
```

Repeat HTTP, old-binary hash, main and frozen Integration checks. A binary
rollback normally preserves the current DB because this fix adds no migrations.
Do **not** automatically restore the DB: startup may have recorded legitimate
activity. If data rollback is independently required, keep the server and all
writers stopped, preserve the failed data directory for diagnosis, and obtain an
explicit operator decision before restoring the private data snapshot/consistent
SQLite backup without stale `-wal`/`-shm` sidecars. `runtime/` and worktrees must
remain intact. Do not resume/create Integrations to test rollback.

### Completed preparation evidence

- Candidate SHA256: `3e37ca967d7faf0cdf0737568a8243b838b84b163fc18d68dd6152efde53c4d5`;
  size 1,353,357,576 bytes; ELF debug executable, current mode 0700.
- Offline build succeeded in 2m06s. `ldd` resolved all dependencies: loader
  `/lib64/ld-linux-x86-64.so.2`, `libc.so.6`, `libgcc_s.so.1`, `libm.so.6` from
  `/lib/x86_64-linux-gnu/`. Keep this host ABI; this is not a portable release.
- `cargo test --locked --offline -p server --lib production_fallback_keeps_api_misses_out_of_spa`:
  1 passed. Real copied assets were hash-compared with the deployment assets.
  Cargo replayed an old build-script dummy-dist warning; the actual index and
  992-file asset set were verified and the real-asset test passed.
- Existing live HTTP health returned OK with `Host: dev.luckyvibekanban.com`;
  plain loopback Host is rejected. Authenticated MCP `get_integration` succeeded.
  These are pre-cutover live checks, **not candidate process smoke tests**.
- No candidate server process was launched and no live-data startup was tested.
  This binary has no safe `--help`/`--version` command: unknown arguments fall
  through to ordinary startup. Do not use them as smoke tests.
- Earlier dispatch, locking and cancellation regression results are recorded
  above. They were not unnecessarily repeated for this documentation-only work.
- Build/test logs and artifact metadata are retained under the own workspace's
  `target/local9-handoff/`. The build artifact remains `target/debug/server`;
  verify its pinned hash before use. Retain the 77 MiB copied dist and normal
  Cargo target, not additional duplicate binaries.
- Cutover/rollback commands are reviewed instructions, not executed evidence.
  If preparation is interrupted, first check remaining Cargo processes and the
  recorded artifact digest; reuse the existing target and logs. Do not infer
  successful deployment from a completed build or documentation commit.

## Second failure after first hotfix deployment — 2026-10-20 12:53 UTC

The preceding cutover section is historical preparation for **45bd9581**. The
operator has now deployed that binary (PID 960511 when inspected; SHA256
`3e37ca967d7faf0cdf0737568a8243b838b84b163fc18d68dd6152efde53c4d5`).
It contains the Create fix but **not the subsequent instruction-readiness fix**.
Do not reuse the old pre-Resume payload digest as the next cutover precondition.

The second failure is a demonstrated startup race, followed by product-driven
cancellation. It is not another missing AgentRun or SQLite lock failure.
Read-only evidence from `data/db.v2.sqlite`, `logs/server.log:296` and the eight
native audit frames for attempt `7b7cf75b-8209-f85f-1792-7823676fb7a0`:

| UTC time | Recorded observation |
| --- | --- |
| 12:53:17.337 | AgentRun `65afecfc-fac6-6e71-fd42-2050835d8cdd` Starting |
| 12:53:17.413 | Running, projected from process-host registration |
| 12:53:17.434 | Specific Create outbox delivered |
| 12:53:17.441–.482 | Steer `d74d0df1-6ddd-45dd-b806-010bad8c73d7` persisted then failed, one delivery attempt |
| 12:53:17.487 | Server logged `agent run port is unavailable: I/O error: Codex has no active thread` |
| 12:53:17.524 | Orchestration Cancel outbox created |
| 12:53:17.540 | Native initialize response (request 1), after Steer already failed |
| 12:53:17.542 | Canonical Cancelling |
| 12:53:17.564 | `I/O error: server was shutdown while waiting for account/read response` |
| 12:53:26.568 | Canonical Cancelled; audit subsequently complete, eight frames |
| 12:53:26.605 | Integration blocked, stop stage integrating/category technical_recovery |

`LocalAgentRunPort` projects Running when the host registers its child.
`DeferredCodexControl` exposes the connected client before its async initialize,
account/read, thread/start and turn/start sequence finishes. The exact no-thread
(and analogous no-turn) error in `AppServerClient::send_direct_control` is raised
locally **before** the Steer RPC is sent. `resume_intervention` previously treated
Running as ready. Its dispatch error propagated to `hold`, which found the
workspace busy and called `cancel`. Thus the account/read shutdown error is
subsequent to cancellation, not evidence of an earlier worker crash. No preceding
provider authentication failure or stale-host failure was recorded. Stderr at
12:53:17.513 does report bubblewrap absent from PATH, but explicitly selects the
bundled fallback; initialize and config/read subsequently respond. This warning
is not the source of the pre-send no-thread error. This does
not certify that provider startup will otherwise succeed on the next attempt.

The observed agent-process-host executable remains the deployment checkout's
`target/debug/agent-process-host`. The failed attempt registry is exited; recorded host PID 962508 and child PID
962510 no longer exist. There was no provider-session observation in
the failed run's six canonical events; never attempt to Steer its cancelled
attempt. The existing formal Resume code selects Retry/Restart when no provider
session is saved, or Retry/Resume when one exists, then delivers the saved
instruction to the new attempt under the same AgentRun identity.

### Narrow fix and evidence limits

`deliver_intervention_instruction` checks canonical state, then uses the existing
durable command service. Pending/Starting/Cancelling wait; terminal states never
receive Steer. Only the two exact adapter pre-send errors (`Codex has no active
thread` / `Codex has no active turn`, with the existing I/O prefix) return Waiting.
The normal one-second Integration monitor retries the same saved command ID and
idempotency key. Failed delivery receipts remain visible until that same command
succeeds; delivered receipts prevent resend. No arbitrary transport error is
retried because delivery could be ambiguous. No global worker, permission,
provider protocol, schema or timeout changes are made. An indefinitely stuck
provider remains visible and requires operator diagnosis; startup waiting does
not automatically cancel it. Port-level terminal fencing still handles a state
change between query and send.

Regression tests use fresh persisted Create identities and a mock port with
Running but no native thread/turn, then exercise readiness, same-command retry,
delivered replay, waiting/terminal fences, and a non-retryable transport failure.
They do not launch a real provider or prove end-to-end Integration publication.

### Follow-up operator deployment and recovery

1. Review the new LOCAL-9 commit and its test results. Build a **new** server
   artifact from that immutable revision using the offline build command above,
   record its new digest and verify the external dist dependency. The old
   `target/local9-handoff/artifact.json` and digest describe 45bd9581 only; this
   investigation did not build or deploy a second candidate server executable.
2. Keep main at `b385a2d2d1a8683fbdcb8d931356d34f4264169c`. Before any cutover,
   finish agents/builds and inspect resource coordination, PID, executable and
   live state again from independent SSH. Current frozen payload SHA256 is
   `8158b60f43a4412d64b2f01e2696de717778dec2f1d3c4aaeda7167b5e88022e`;
   status blocked, skip_validation=true, unpublished, no publication intent,
   result_commit=null, zero validations. Require that payload and original four
   sources to remain unchanged during cutover.
3. Adapt the checked cutover procedure above: expected old binary hash is now
   `3e37ca967d7faf0cdf0737568a8243b838b84b163fc18d68dd6152efde53c4d5`, and expected
   candidate hash must come from the new build. Use a fresh private backup path;
   save this currently serving binary for rollback. Use graceful stop, stopped
   WAL-aware backup, atomic rename and the existing start.sh. Do not execute
   these steps via the server's MCP. This source change requires no agent-host
   replacement, migration, source-ref movement or config change.
4. Verify HTTP, UI, authenticated read-only MCP, main and new frozen payload
   digest. Rollback, if needed, restores the just-backed-up 45bd9581 binary using
   the same stopped-server procedure, not the pre-first-hotfix binary. Do not
   automatically restore DB or resume work during rollback.
5. Only after operator acceptance, use a **new explicit formal Resume request**
   on `c68ce019-9349-4679-976f-326120191c08`. The cancelled attempt must be
   restarted by the supported Retry path, never directly steered. Do not replay
   the old failed Steer alone, create another Integration or resume `0771cd56`.
   Observe new attempt identity, provider startup, instruction receipt and
   progression while preserving skip_validation=true.

Second-failure validation in this workspace:

- `cargo test --offline --locked -p server --lib routes::integrations::runtime::tests`: 21 passed (including two new readiness regressions).
- `cargo test --offline --locked -p services --lib dispatch_tests`: 7 passed (including concurrent Create, replay, cancellation and formal resume policy).
- `cargo clippy --offline --locked -p server --all-targets -- -D warnings`: passed, including type checking.
- Scoped `rustfmt --check` and `git diff --check`: passed.
- Initial test compilation caught a missing mock `subscribe` method; corrected before the passing run.
- No real-provider startup, second binary build, hotpatch, Resume or publication was performed.

If interrupted, resume from this committed source and retained evidence at
`target/local9-second-failure/`; keep the serving dist intact. Recheck live
identity and frozen payload before planning any separately authorised cutover.

## Prepared one-command operator cutover — LOCAL-9 33587794

This is the current handoff, superseding the manual first/second build instructions
above. **The actual cutover has not been executed.** After this AgentRun and all
other jobs finish, an operator may run the following one command as **appuser in
an independent SSH TTY inside the same container namespace**:

```bash
python3 /var/tmp/local9-hotfix-33587794-prepared/cutover.py --execute
```

The program requires a TTY and rejects server/Codex ancestry. Do not invoke it
through MCP, an AgentRun, a scheduled task, or a child detached from this agent.
An external SSH/container-entry command is installation-specific; no unverified
container name or SSH destination is assumed. Keep the SSH session open. HUP is
ignored and logs are flushed to disk, but container destruction or uncatchable
termination cannot be made safe by nohup/setsid. No updater service is installed.

Candidate ELF is
`/var/tmp/lvk-instances/dev/runtime/worktrees/d546-local-3-4-integr/lucky-vibe-kanban/target/debug/server`,
1,353,264,896 bytes, SHA256
`efa3ff3a66f0eeb0a6efc8661efc280592b6a02cf72d9d60cf35a6442c1135d6`.
It was built successfully with the recorded offline command, jobs=2, from exact
clean source `33587794d71f5da0107dd34b6eb6be5393511a70`; only operator scripts and
documentation were added afterward, with no Rust/frontend/dependency changes.
The serving binary remains 45bd9581, hash `3e37ca96…`, PID 960511 when rechecked.
`ldd` resolves libc, libgcc_s, libm and the same Linux loader as the old artifact.
No new server process was smoke-started against live data.

The same 992 real frontend files were verified by their complete manifest digest.
The candidate still depends on this workspace's `packages/local-web/dist` at
runtime. **Retain the workspace/dist and candidate; do not clean or rebuild them
before cutover.** Artifact metadata and build/mock-test logs are retained at
`target/local9-cutover/`, with a private handoff copy under
`/var/tmp/local9-hotfix-33587794-prepared/`. No duplicate 1.35 GB binary is staged.

The auditable source is `scripts/local9_cutover.py`, with isolated tests in
`scripts/test_local9_cutover.py`. Its pinned checks include old/new ELF hashes,
launcher and agent-process-host hashes, frontend manifest, container namespaces,
main and four source refs, exact Integration payload digest `8158b60f…`, and
blocked/unpublished/skip-validation policy. It rejects active agents, hosts,
execution processes, workflows, resource operations, unresolved outbox delivery,
enabled schedules, concurrent Cargo/rustc, and other same-user SQLite handles.
The retained cancelling Orchestration parent is permitted only for the exact
Integration ID, with all agents/hosts exited and outbox deliveries complete.
A changed resource-coordination registration requires a new operator review.

The process captures the old server's actual launch environment in memory, then
backs it up privately for manual recovery. It does not infer PATH or credentials
from SSH. Backup, log and state directory is created atomically as
`/var/tmp/local9-cutover-33587794/` (0700). **It contains credentials in the private
launch-environment JSON and data snapshot; do not paste or upload its contents.**
Stage names and exit codes contain no credentials. Every repeat execution refuses
once that directory exists, including after an early abort; do not delete it to
blindly retry. Review the recorded state first.

After two idle/frozen-state checks, the program holds the existing launch lock,
uses a verified pidfd to send SIGTERM, waits up to 60 seconds, and refuses any
force kill. It copies the stopped private data tree and obtains a separate
SQLite backup through the backup API (including committed WAL pages), checks
integrity, rechecks idle state, then fsyncs and atomically renames the binary.
It releases the launch lock immediately before invoking the unchanged start.sh
under the captured environment. It verifies executable hash/cwd/owner, listener
ownership, HTTP health and new startup-log evidence, then rechecks the frozen
state and idle jobs. No Integration actions, Git publication or DB restore occur.

The initial cutover deadline is 300 seconds; failure recovery uses bounded
60-second stop and 45-second health waits (launcher timeout 10 seconds). There
is no force-kill fallback. Kernel/filesystem hangs and process/container loss
still require operator recovery. A maintenance window with **no new user/API
submissions or manual launchers** is required: read-only idle checks cannot
atomically prevent new submissions without changing server admission behaviour.

Expected durable outcomes (`cutover.log`, `state.json`, `exit-code`):

| Exit | Last stage | Meaning |
| --- | --- | --- |
| 0 | SUCCESS_NO_RESUME | New binary healthy; original Integration still blocked |
| 2 | ABORT_OR_STOP_UNCONFIRMED_OPERATOR_CHECK | Precondition failed or stop was not confirmed; inspect actual PID/listener |
| 3 | ROLLED_BACK_NO_DB_RESTORE | New startup/check failed; saved 45bd9581 binary restored and health/frozen state verified |
| 4 | MANUAL_RECOVERY_REQUIRED_NO_FORCE_KILL | Ownership/termination/recovery uncertain; no blind retry |

For exit 4 or interruption, inspect state and the actual PID/executable/listener
from SSH. Do not kill an unverified PID, start a second server or restore SQLite.
Once all relevant processes are proven stopped, the saved executable is
`/var/tmp/local9-cutover-33587794/server.old` (expected hash `3e37ca96…`). Use the
verified atomic executable rollback procedure above. To reproduce the captured
startup environment after a manually verified rollback, use:

```bash
python3 - <<'PY'
import json, subprocess
from pathlib import Path
backup = Path('/var/tmp/local9-cutover-33587794')
env = json.loads((backup / 'launch-environment.json').read_text())
subprocess.run(['/var/tmp/lvk-instances/dev/start.sh'], env=env, check=True, timeout=10)
PY
```

That recovery command is **not** permission to bypass PID/listener checks. Do
not automatically restore the DB. After a successful cutover, separately check
authenticated UI/MCP and only then perform the previously authorised formal
Resume of the retained four-source Integration. The program never resumes the
old two-source run or creates another Integration.

Validation: Python syntax check and six isolated fake-service tests passed:
actual executable switch/health success, health failure with rollback, repeat
refusal, stale PID refusal, changed payload, busy jobs and changed artifact
(the repeat check shares the success test). Real read-only preflight passed
artifact/ref/payload/coordination/PID checks and correctly refused this active
AgentRun before any stop or state-directory creation. An initial HTTP check
exposed internal-token Host handling; the final code uses loopback Host for the
internal request, public Host for health, disables proxies and refuses redirects.
The underlying 33587794 source already passed 21 Integration tests, 7 dispatch
tests and server all-target Clippy as recorded above; those unchanged tests were
not repeated. Actual cutover and live candidate startup remain operator work.


## Third failure: retry startup projection (20 October, 13:32 UTC)

The prior read-only investigation established that attempt 2 reused the original
input message UUID as its canonical event UUID. Its attempt and Native Audit
reference differed, so the Started batch was quarantined for an idempotency
collision. Both the input and Running fact were lost from the projection. The
provider nevertheless completed step 1 with `outcome=ready`. The host had not
sent the new instruction (`575bde99…`, no command receipt); it was still waiting
for Running and blocked when the run became terminal. The older failed Steer
`d74d0df1…` was a separate command. The final-result projection guard would also
have rejected this degraded run. Child success was not Integration success.

The source repair preserves the original input message identity but derives the
retry startup **event** UUID from the attempt UUID and message UUID. Initial
launch retains its historical event UUID. Same-attempt redelivery has the same
identity and content. Provider-mapped events already include the attempt UUID;
Started lifecycle and terminal events retain their journal event identities.
No schema or host protocol change is needed.

A formal, authorised Retry can now reconcile this specific historical failure
before creating its next attempt. Durable command and workspace authority checks
run first. Reconciliation requires a terminal latest attempt, exactly the known
startup-collision quarantine, matching original input, an exited registry,
confirmed absent host/provider, and a complete matching journal and Native Audit.
It verifies the canonical input bytes, appends the two missing startup facts with
stable identities, and rebuilds the projection from all canonical events. The
rebuilt state must preserve every state field except projection status and update
time. A late Running fact does not resurrect a terminal attempt. Unknown errors,
live processes, missing/corrupt evidence and other projection discrepancies fail
closed. Original audit files, events and quarantine rows are retained. A crash
between appending facts and rebuilding leaves the run degraded and permits the
same evidenced reconstruction to be retried; no provider is launched by it.

The recovery is not a bypass for Steer and does not adopt the previous candidate
as a completed step. The normal Retry, provider readiness, structured result,
clean candidate/ancestry, review and publication gates still apply. The retained
`9790327408a36f9fcef87ad15e781f40d0a5501c` candidate remains available in the
Integration checkout; its step has not yet been accepted by the host. Existing
same-Integration reservations are reusable by the formal intervention transaction;
foreign reservations still reject it. Do not manually clear reservations or
replay the old failed Steer.

Product-completion settlement now reserves the SQLite writer before reading the
node and plan (`append_product_completion`, `BEGIN IMMEDIATE`). An isolated
DELETE-journal regression reproduces code 5 with the old deferred transaction
and passes with the reservation. The historical live lock holder remains unknown;
this proves a susceptible operation and a scoped correction, not that a specific
historical task held the lock. Global journal, timeout and pool settings are unchanged.

### Operator handoff for this repair

1. Rebuild the source commit containing this section after review, with the
   existing bounded/offline server build procedure. Record its full commit and
   candidate hash. **The earlier `33587794` cutover program and pinned hash must
   not be executed unchanged for this repair.** No new executable or cutover
   program is staged by this source-repair run.
2. In a separately approved maintenance window, re-inspect actual PID, launcher,
   frontend dependencies, jobs and current frozen payload; prepare a fresh
   one-shot SSH cutover with new hashes and state guards. Back up the currently
   serving `33587794` executable (`efa3ff3a…`), launcher and WAL-safe SQLite state.
   Stop/switch/start only from an independent operator SSH session, never from
   this server's AgentRun/MCP. On startup failure restore only the saved executable
   after confirming process ownership/termination; do not automatically restore DB.
3. Check HTTP/MCP health, main `b385a2d2d1a8683fbdcb8d931356d34f4264169c`, all four
   frozen source refs, unpublished state and `skip_validation=true`. Then explicitly
   Resume **only** `c68ce019-9349-4679-976f-326120191c08` through the existing UI.
   Keep `0771cd56…` and LOCAL-11 untouched. Do not create a replacement Integration.
4. Verify the historical quarantine is retained, the new attempt has Current
   projection and Running, the new instruction has a delivery receipt, and the
   host accepts step 1 before proceeding to steps 2–4. Stop for evidence review
   if a guard rejects; do not loop Resume or erase degradation flags. Publication
   remains the formal Integration's responsibility and the result stays unverified.

Source tests exercise isolated persistence/host-journal and Integration boundary
fixtures; they do not prove a real Codex/provider or live four-source Integration
has completed. No deployment, live recovery, production DB write or target/source
branch movement is performed by this repair run.

Read-only handoff check during this repair: payload SHA256
`9a871dca4b729996e063b77e4c4021bb97527980059a6a770d1c5086c655f25f`;
blocked, unpublished, zero accepted steps, null result, skip-validation true.
Four saved commits match their source branches, main remains frozen, and the
step-1 candidate checkout is clean. Attempt-2 journal sequence 748 has a matching
canonical terminal fact and timestamp; the registry is exited. Recheck these
conditions at cutover, rather than treating this observation as a future lock.

### Source validation for the third-failure repair

All Cargo commands used `--locked --offline`, two build jobs, the workspace's
own target directory and sccache. Tests used isolated databases/journals/Git
fixtures; no real provider launch or live Integration run was used.

| Check | Result |
| --- | --- |
| `cargo test -p db models::orchestration::tests` | 11 passed; includes atomic/idempotent completion and writer contention |
| `cargo test -p db --lib models::agent_runtime::tests` | 22 passed; canonical projection, quarantine, audit, cursor and SQLite retry |
| `cargo test -p db --lib models::integration::tests` | 17 passed; ownership, durable intervention, reservation and skip-policy fences |
| `cargo test -p local-deployment --lib agent_run_port::tests` | 19 passed; cancelled attempt, real collision, verified reconstruction, rejected corrupt/incomplete/live evidence, interrupted rebuild retry, startup redelivery and command authority |
| `cargo test -p server --lib routes::integrations::runtime::tests` | 21 passed; readiness/Steer retry and terminal fences, structured ready acceptance, step receipt idempotence, sequential progression and skipped validation |
| `cargo test -p services --lib dispatch_tests` | 7 passed; targeted/concurrent Create, explicit Resume and retained skip policy |
| `cargo check -p local-deployment` | Passed; affected source also compiled by tests/Clippy |
| `cargo clippy -p db -p local-deployment -p server --all-targets -- -D warnings` | Passed |
| `cargo fmt -p db -p local-deployment -p server --check`, `git diff --check` | Passed |

The new product-completion test was also run with only that transaction reverted
to deferred BEGIN: it failed with SQLite code 5, then passed after restoring the
fix. Initial regression-fixture failures were corrected (registry terminal
constraints, attempt Starting fact, skipped admission's empty validation plan).
The passing checks above are the final runs. The initially selected server binary
and services file-name test filters selected zero tests; they were corrected to
the library/module filters above and are not counted as coverage.

There are 97 distinct passing tests across these scoped groups. The retry/audit
and host step-acceptance boundaries are tested in separate fixtures; this is not
an end-to-end live Codex/4-source success claim. No frontend or new deployment
binary was built for this repair. Cargo replayed an old dummy-dist build warning;
the actual preserved dist was checked to contain 992 files and a real index, not
the placeholder. SQL changes use runtime queries; no schema or query-macro
metadata changes are involved.


## Prepared one-command operator cutover — LOCAL-9 0a5cef88

Preparation only, 20 October 2026. Run the following **after this Codex Run has
finished**, from an independent SSH TTY as `appuser` inside the same container.
Keep that session open and arrange an exclusive maintenance window: no new
UI/API submissions, agents, builds or manual launchers. Do not invoke it through
MCP, an AgentRun, or a detached child of Codex.

```bash
python3 /var/tmp/local9-hotfix-0a5cef88-prepared/cutover.py --execute
```

This is the new one-shot script; the previously executed
`/var/tmp/local9-hotfix-33587794-prepared/cutover.py` and its
`/var/tmp/local9-cutover-33587794/` backup/output remain untouched. Do not rerun or
delete them. The new staged script SHA256 is
`25cb0f49f4a9c43e3084db3e393cc21931979c4650a218c16efa3f89c4877ca8`.
Its audited repository source remains `scripts/local9_cutover.py`; tests are
`scripts/test_local9_cutover.py`. No new daemon or updater service is introduced.

### Verified artifact and runtime

The candidate was built from exact clean source
`0a5cef88bc71aba5b23d60f970629073d8b46446`, with:

```bash
RUSTC_WRAPPER=/usr/local/bin/sccache SCCACHE_DIR=/var/tmp/lvk-instances/dev/runtime/shared/lucky-vibe-kanban-40384b04-3346-4f93-a96e-8b42696b0aaa/cache/sccache CARGO_BUILD_JOBS=2 cargo build --locked --offline -p server --bin server
```

Candidate path:
`/var/tmp/lvk-instances/dev/runtime/worktrees/d546-local-3-4-integr/lucky-vibe-kanban/target/debug/server`

- New SHA256: `25bba6a706168ee16f3c682f7f7725d3dc35aa2966d05122e725ebf751f74d80`
- Size: 1,354,244,880 bytes; ELF64, little-endian, x86-64.
- Independently observed serving PID: 989363; `/proc/989363/exe` SHA256:
  `efa3ff3a66f0eeb0a6efc8661efc280592b6a02cf72d9d60cf35a6442c1135d6`
  (the previously deployed 33587794 artifact, 1,353,264,896 bytes).
- Installed path:
  `/var/tmp/vibe-kanban/worktrees/1f9a-chatgpt/lucky-vibe-kanban/target/debug/server`.
  The candidate and installed binary are distinct files. Both checkouts share
  `/repos/lucky-vibe-kanban/.git`; the deployment is not an independent Git copy.
- `ldd` resolves libc, libgcc_s, libm and `/lib64/ld-linux-x86-64.so.2`, as for the
  serving binary. `file` is unavailable; the ELF header was validated directly.
- Debug rust-embed reads the preserved workspace `packages/local-web/dist` at
  runtime: 992 real files, manifest digest
  `944540d7eb8081ac35cc53c06a9a86bf3f6f9bf53e470273ee17286efffbe599`.
  Retain this workspace, dist and candidate; do not rebuild/clean them before
  cutover. The cached dummy-dist Cargo warning was not an actual replacement.
- Launcher hashes, host executable hash and namespaces were rechecked and remain
  pinned in the script. The unchanged start.sh uses nohup+setsid run.py, which
  changes to the installed checkout, writes server.pid and execs server. The
  actual process environment was checked privately; the operator program captures
  and reuses it rather than relying on SSH environment variables.

The build took 1m34s with available memory about 13 GiB, disk about 423 GiB, and
no other Cargo/rustc or AgentRun. Only this authorised preparation Run was active.
Runtime source remains the pinned commit; subsequent changes are limited to the
handoff script, tests and this document, which the script explicitly permits.
Any other tracked source change, untracked file or changed frozen ref refuses.
There are no new migrations or host protocol changes; binary-only replacement
is appropriate for this specific patch. No candidate was smoke-started against
live data. The preceding 97 Rust tests/Clippy results belong to the source-repair
run and were not rerun for these script/document changes.

Build metadata/log and mock-test evidence are retained in
`target/local9-cutover-0a5cef88/` and the private prepared directory above. There
is no duplicate 1.35 GB candidate copy. The normal workspace target/cache is kept.

### Retained state and preconditions

The exact payload digest is
`9a871dca4b729996e063b77e4c4021bb97527980059a6a770d1c5086c655f25f`:
blocked/unpublished, result null, validation empty, four unchanged source refs,
`skip_validation=true`, main frozen at
`b385a2d2d1a8683fbdcb8d931356d34f4264169c`. Candidate `97903274…` remains tentative.

The orphaned parent and integrate node are still `running`; their two target /
workspace reservations remain. This is not treated as permission to ignore
arbitrary running work. The script permits only this exact parent/node and
reservation ownership, a single product-validation-gated plan, its exact linked
`succeeded/projection_degraded` child, exited agent hosts and delivered outbox.
The host scheduler excludes blocked Integrations; generic orchestration recovery
cannot accept child success without product validation. No reservation is cleared
and no projection is repaired by the cutover program.

The historical failed Steer `d74d0df1…` is allowed only by exact command ID,
agent, failed status and envelope digest. It is not the current intervention.
Server startup may retry and reject that historical command at the workspace
ownership guard, recording another failed receipt; this is existing startup
behaviour, not a Resume or replay requested by the updater. Every other unresolved
command, pending inbox, active agent/process/workflow/resource operation, enabled
schedule, unexpected parent/node/reservation, build/provider process or other
same-user SQLite handle refuses cutover. This Run must end and its host exit first.

Read-only preflight passed artifact, source, frontend, frozen state, coordination,
PID and listener ownership checks, then correctly refused this active AgentRun.
The staged `--preflight` mode returned exit 2 without creating the cutover state
directory or sending signals. It is safe to run that mode for inspection; it does
not waive execution's TTY/ancestry/namespace checks. Resource coordination was
empty; any later registration, holder or queue requires a new operator review.

### Execution, outcomes and rollback

The program creates `/var/tmp/local9-cutover-0a5cef88/` exactly once, mode 0700.
It saves the old executable, startup files and private launch environment, takes
the existing launch lock, repeats frozen/idle checks, sends verified-pidfd SIGTERM
and waits (no force kill). After confirmed stop it copies private data and makes
an SQLite backup via the backup API, including committed WAL pages, with integrity
checking. It atomically switches the executable and invokes the same start.sh.
Process identity/hash, listener ownership, HTTP health, fresh startup log evidence,
frozen state and idle jobs must pass. It neither publishes Git nor calls Resume.

The initial deadline is 300 seconds; rollback gets a separate 120-second budget,
with stop bounded at 60 seconds and health at 45 seconds. Kernel/filesystem hangs,
container loss or SIGKILL cannot be made safe by Python deadlines. A read-only idle
check is not an admission lock; the exclusive maintenance window remains essential.

| Exit | Durable final stage | Operator meaning |
| --- | --- | --- |
| 0 | `SUCCESS_NO_RESUME` | New executable healthy; original Integration still blocked |
| 2 | `ABORT_OR_STOP_UNCONFIRMED_OPERATOR_CHECK` | A precondition/stop check failed; inspect actual PID/listener, do not blindly retry |
| 3 | `ROLLED_BACK_NO_DB_RESTORE` | Saved 33587794 executable restored and health/frozen state checked |
| 4 | `MANUAL_RECOVERY_REQUIRED_NO_FORCE_KILL` | Ownership/termination/rollback uncertain; leave recovery to the operator |

Repeat execution or a TTY/ancestry/namespace refusal exits before entering the
cutover (normally exit 1), preserving the existing state directory. Logs contain
fixed stage/class names, never exception text, environment or HTTP bodies.
`cutover.log`, `state.json`, `exit-code` and private `launcher.log` are durable in
the new directory. Do not delete it to rearm an aborted attempt.

On startup/health failure, automatic rollback stops only a verified new PID,
atomically restores `server.old` with the pinned `efa3ff3a…` hash, starts via the
same launcher/environment and rechecks health and frozen refs/payload. **No SQLite
restore occurs.** Startup audit/command receipts remain recorded. For exit 4 or an
interruption, inspect PID/executable/listener and these logs from SSH first; do
not start a second server or kill an unverified process. The recovery files are:

- `/var/tmp/local9-cutover-0a5cef88/server.old`
- `/var/tmp/local9-cutover-0a5cef88/launch-environment.json`
- `/var/tmp/local9-cutover-0a5cef88/db.v2.consistent.sqlite`
- `/var/tmp/local9-cutover-0a5cef88/data/`

The environment and data snapshots contain credentials: keep them private and
never paste them into chat. Restore an executable manually only after ownership
and termination are proven; do not automatically roll back DB or reuse the old
33587794 cutover's backup paths for this operation.

After exit 0, obtain a **fresh read-only ChatGPT/MCP check** of health, main,
source refs, retained Integration and skipped-validation policy. Formal Resume of
`c68ce019-9349-4679-976f-326120191c08` is a separate later action. Never resume the
old two-source `0771cd56…`, create a replacement, or include LOCAL-11 here.

Validation: AST/compile and `git diff --check` passed. Fourteen isolated mock tests
passed, including real process switch, launcher failure and health-failure rollback,
committed-WAL backup, repeat refusal, stale PID, changed source/main/payload/hash,
active AgentRun, foreign reservation, unresolved command, missing product gate,
read-only preflight and exception-text suppression. Mocks exercise the real Git
and SQLite predicates but substitute temporary launchers, binaries, HTTP service
and global process/coordination checks. They do not prove live Codex startup,
real MCP authentication or Integration recovery. No live cutover was executed.
