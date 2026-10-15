---
title: "Automatic merge and human review"
description: "Merge selected commits in order, test each cumulative result, and review AI resolutions in a visible Integration Workspace."
---

## Start from the Board

Select one Workspace per Card, the observed source commits, their merge order,
and the local target branch. Starting creates one ordinary Board Card named
`Auto merge: … → target`, with a linked Integration Workspace. The Card,
Workspace, link and request are saved together. Repeated delivery of that request
returns the same execution. Queued work is visible before the agent starts.

The Integration Workspace appears in the normal Workspace list and beneath its
Card on the Board. Open it there or use **Open integration Workspace** in the
start dialog. Conversation, file differences, test results, resolution review,
additional instructions and cancellation are available in that Workspace.
Existing provider questions and tool approvals remain available too.

Selection adopts committed Git objects. Later work on the source branch, dirty
files, active Goals, other Workspaces attached to the Card, and work in other
repositories do not prevent merging those objects. Uncommitted changes are not
included. The start dialog rejects a changed observed HEAD until explicitly
reselected; after admission the selected commit stays fixed.

## Merge and test each source

The target HEAD at preparation is base B. The Integration merges the first source
on its own branch and submits a committed candidate. LVK runs the applicable
validation commands on that candidate. After validation and any required human
review, it moves to the next source in the same Workspace and Session.

Each step preserves the previously tested prefix. The host carries earlier
validation commands into subsequent plans, including their required status;
it does not reuse their earlier passing results or exception approvals on a new
candidate. The agent adds applicable tests for the newly merged source and its
interaction with earlier changes. Completed steps retain their commands,
results and exceptions. Repository instructions and tests determine the plan;
passing checks do not prove complete test coverage.

Validation plans use Workspace-specific compiler/test output directories. Inherited
shared output paths such as `CARGO_TARGET_DIR` must not substitute artifacts from
another checkout; dependency download caches can still be shared.

Git computes an automatic merge tree before each step. If there are conflicts,
or the agent changes that tree beyond Git's automatic result, LVK records the
resulting difference for human review. Ordinary clean merges with no additional
edits advance automatically after tests pass.

## Review an AI resolution

The Workspace shows the agent's explanation, the candidate commit, test results,
and the diff from Git's automatic merge tree. Conflict marker removal is visible
in that diff, along with additional compatibility edits. The normal Workspace
diff is also available for the full change.

- **Approve and continue** accepts only the displayed candidate and plan.
- Enter instructions and choose **Request revision** to reject the current
  proposal and ask for another resolution. Earlier proposals remain in history;
  the new candidate is tested and reviewed again.
- **Cancel Integration** stops the operation before publication.

Approval is an explicit UI decision, not text in a prompt. The agent cannot
approve its own result. Changing the candidate or validation plan invalidates
an old approval. Reconnecting retains pending reviews.

## Handle failed tests

For a required command that fails, is unavailable, or does not finish, the
Workspace offers **Fix environment and retry**, **Continue with this exception**,
and cancellation. An exception covers only that check and candidate. The failed
result remains visible; it is never rewritten as a pass. Additional instructions
can ask the agent to repair code and submit a freshly tested candidate.

Storage errors, an unconfirmed writer, a dirty candidate checkout and target
advancement are execution problems. Repair the environment and resume the same
Integration where possible. Preparation retries reuse the existing Card and
Workspace. If the target advanced, make a new explicit selection against it.
Cancellation never rolls back an already published target.

## Publish and move Cards

LVK advances the local target to the exact final tested and approved commit,
provided the target still equals B. It does not create another merge commit at
publication. No push, PR or deployment is implied.

Successful publication moves the originally selected Cards and the auto-merge
Card to Done. There is no separate requirements-completion assessment: Card
text, source links and other work in progress do not disqualify a selected Card.
The column update and its receipt commit together, so a retry does not move a
Card back to Done after a person has reopened it. A missing Card/column is
reported separately from Git success. Older executions may still hold legacy
reservations; those remain respected until released.

OpenWiki and semantic bookkeeping are separate follow-up results. Missing intent
records are reported as evidence limitations, and memory bookkeeping failures do
not reverse Git publication. The existing publication reconciliation action can
retry pending semantic bookkeeping without merging again.

## Compatibility and scope

Existing non-sequential Integration runs retain their saved validation flow.
The sequential contract applies to newly admitted runs. The underlying ownership
record still routes agent commands, cancellation and publication; visibility does
not grant a second concurrent writer access to a candidate under validation.
OpenWiki maintenance Workspaces keep their existing separate execution scope.
