---
title: "Recovering a stopped formal Integration"
description: "Review validation risks, send Integration instructions and resume an existing execution without bypassing publication checks."
---

## Review the stop before continuing

Open the local Board's Integration panel, then **Open integration Workspace**.
Review the selected Issues and source commits, target branch, base commit B,
candidate R, host command results and saved error. Git publication, Issue Done
and subsequent Wiki processing remain separate results.

For a completed required check that fails, cannot run (for example, command exit
126/127), or is stopped after the validation deadline, the host saves a
confirmation. Reconnecting does not dismiss it. The Integration retains its
reservations while you decide:

- **Fix environment and retry** reruns the affected check on the same clean R.
  Previous failed evidence remains in the confirmation history.
- **Continue with this exception** requires you to acknowledge the displayed
  risk. It accepts only that check for the displayed candidate and selection.
  Further checks still run. Failed/unavailable evidence stays failed/unavailable;
  the UI separately records the exception, local operator and decision time.
- **Cancel Integration** stops the product without publishing the candidate.

An unanswered confirmation does not publish. A changed candidate, base, selection,
Issue requirement revision or validation plan invalidates the corresponding
exception. The publication path still checks source identity, checkout cleanliness,
completed process cleanup, validation command identity and before/after commit
identity. Git publishes R only if the target still equals B.

Storage failures, unknown execution ownership, unconfirmed processes, dirty or
conflicted checkouts, permission failures and target advancement are technical
stops, not validation exceptions. Repair the cause first. Target/source changes
require a new explicit selection; do not force-push or edit saved approval data.

## Give the Integration agent instructions

The Integration Workspace has a dedicated instruction field. Other execution-only
Workspace kinds retain their existing restrictions. Ordinary Workspace input is
unchanged. Session creation and direct publication remain owner-controlled.

During integration, **Send instruction** uses the existing canonical Steer command
and audit. It checks the exact Integration, Workspace, Session and current AgentRun,
and serialises against freezing the validation candidate. Providers must support
steering; an unsupported or undelivered instruction is an error, not acceptance.

After a blocked execution, **Resume after repair / send instruction** retains the
Integration and Session. You may leave the instruction blank to retry after an
external repair. The host reacquires reservations and rechecks sources and B.
When Create acceptance is uncertain, it reconciles the saved immutable request
and uses that same execution identity. A confirmed terminal AgentRun uses the
existing Resume mechanism when a provider session exists, otherwise Restart.
Additional text is delivered using the existing instruction command. The host
validates the resulting candidate again and does not reuse an old exception for a
changed candidate.

Instructions do not answer a validation confirmation and do not grant tool
execution approval. Existing question and tool-approval controls remain separate.
The product's saved recovery history retains the prior error, candidate/check
results and instruction. Canonical commands/events retain dispatch and delivery
outcomes. If an agent finishes before the instruction reaches it, inspect the
saved instruction and resume explicitly; do not assume it was processed.

## Recover the reported database lock

The reported historical Integration is
`66198606-4010-448b-a3be-f8c7e2f27494`, with Workspace
`60a671bf-3ef1-4294-aa67-8b8869d328f1`. This implementation task does not authorise
resuming it, approving its checks, merging it or changing deployment settings.
These steps are for an operator after installing and validating the change:

1. Open the existing Integration and inspect its saved error, dispatch receipt,
   Workspace and Session. Do not assume the absence of `agent_run_id` proves that
   the execution was never accepted.
2. Resolve the competing SQLite writer or underlying storage problem. Do not
   delete database journal files or blindly terminate a process holding the DB.
3. Use **Resume after repair / send instruction** on that existing Workspace.
   The host reconciles the original Create envelope before retrying. A still-live
   or unconfirmed writer prevents a second launch.
4. If the selected source, requirements or target changed while the Integration
   was blocked, cancel/reselect explicitly. An old candidate is not published
   against a new target.
5. Review any host validation confirmation independently. Resuming an execution
   does not approve failed checks. Verify Git publication separately from Done
   and Wiki results.

Preparation failures before a durable Create envelope exists still require
inspection and explicit reselection. Publication-intent failures use the existing
**Reconcile publication (no remerge)** operation. They never replay a Git write
just because the database receipt is absent.

## SQLite boundary and external comparisons

Agent identity and retry admission previously began a deferred transaction,
read idempotency state, then wrote. Concurrent SQLite writers can make that upgrade
fail immediately; WAL's stale snapshot cannot be rescued by a longer busy timeout.
Both admission paths now use `BEGIN IMMEDIATE` before reading. The existing
five-second database busy timeout bounds waiting. Persistent failure is retained
as failure; retries reuse the durable request. The historical incident has not
been re-executed against its live database, so this is not a claim that its exact
lock holder was identified.

Official references retrieved on 12 October 2026:

| Reference | Adopted | Not adopted |
| --- | --- | --- |
| [GitHub protected branches](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches) | Re-evaluate approval after candidate/base changes; retain publication gates | Permanent administrator bypass lists and disabled status checks |
| [GitLab merge request approvals API](https://docs.gitlab.com/api/merge_request_approvals/) | Bind an explicit approval to an expected revision; reject stale answers | GitLab roles, merge engine and approval-policy framework |
| [GitLab job control](https://docs.gitlab.com/ci/jobs/job_control/) | Separate manual intervention/confirmation from execution outcomes | Treating optional/skipped jobs as evidence that LVK's required tests passed |

LVK reuses Integration payloads/reservations, Orchestration events/outbox,
AgentRun commands and ExecutionProcesses. There is no second merge engine or
independent execution ledger. Explicit product resumption has an audited event;
ordinary late lifecycle observations still cannot reopen terminal orchestration.

## Authority and validation limits

Formal Integration is a local Board capability. Decisions use the existing local
operator identity (UUID 1) and existing Origin/relay-signature boundary; this
change does not introduce per-person RBAC or an OS sandbox. A local process with
full access to the server API/database is within that existing trust boundary.
Reference text and normal prompts never populate a confirmation decision.
Do not expose the local operator API to untrusted clients.

Provider execution and notification delivery depend on the configured host.
Notifications use the existing notification preferences. Browser confirmation and
saved API state remain available when a notification cannot be delivered. The
source test record and completion report distinguish isolated automated checks
from live provider, browser and deployment verification.

## Automated regression coverage

The new tests cover durable decision races and replay, cross-project and stale
confirmation rejection, ordinary prompt/approval schema separation, exact owned
Workspace/Session/Agent instruction permission, explicit resumption versus late
terminal events, and a real WAL writer that both releases within and exceeds the
bounded wait. The publication regression uses an isolated real Git repository and
SQLite execution evidence with a mock failed check; it verifies exact-R publication,
target advancement rejection and no second publication.

UI regressions exercise acknowledgement-required controls, retained failed
results, restored decisions, Integration-only intervention, running versus blocked
instruction routes (including no AgentRun), stable resend identities and disabled
publication-phase controls. These component tests do not claim a live provider or
browser end-to-end run.
