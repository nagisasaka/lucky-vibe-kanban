# Formal Integration dispatch recovery

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
