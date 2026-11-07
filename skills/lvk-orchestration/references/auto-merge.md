# Auto-merge

## User-facing auto-merge terminology

- Name the LVK feature **自動マージ** in Japanese and **Auto-merge** in English, consistently in user conversations, specifications, explanations, status reports, and coding-agent handoffs intended for users. Use the same feature name even when source code or tool results use an internal workflow label; do not present internal labels as alternative user-facing feature names.
- Preserve exact tool names, API routes, database identifiers, type names, file paths, commands, and historical quotations. Mention internal identifiers only when technically necessary, and distinguish them from the user-facing feature name rather than renaming machine-readable contracts.
- Explain capabilities separately from the name: LVK validates changes, uses AI to resolve conflicts when needed, and incorporates the result into the target branch. Preserve existing approval boundaries; the name does not promise unconditional or fully unattended success. Report merge success, target publication, and subsequent Wiki maintenance separately.

## Start, verify and publish

Before starting, verify authorization, the selected source Workspace commits and expected target HEAD using live MCP schemas. Submit the frozen selection through the supported Auto-merge operation and retain its receipt and conversation identifiers. Continue questions and repairs in that assigned conversation. Do not merge directly to bypass guards.

The assigned Codex owns candidate preparation, conflict repair, risk-based validation and enabled OpenWiki maintenance. Record executed checks and justified skips separately with actor, reason, remaining risk and exact SHA. OpenWiki runs in that same candidate branch after code validation, with a separate Wiki commit; ordinary coding agents must not update canonical OpenWiki. Never execute retired .llm-wiki stages.

Read merge/candidate, target publication and Wiki evidence separately. A publish request is not confirmed Git application. Finish the admitted candidate if target advances; publish it unchanged only if the actual target is its ancestor and host checks pass. Otherwise preserve evidence and stop InReview; do not start automatic tracking/rebase chains. Publication success drives the generated system Issue/Workspace state and archive lifecycle in LVK itself. It is distinct from the coordinating AI's cleanup after a transfer. Never manually manipulate the system tag to simulate this lifecycle.

## Risk-based test planning and auto-merge validation

- Plan necessary tests from change risk, evidence tied to the already-tested SHA, the merge delta, conflict resolutions, and newly unverified scope; do not mechanically repeat every test at every stage. For small UI changes, choose relevant rendering and targeted behavior checks rather than imposing unrelated E2E tests. Distinguish builds needed to produce artifacts from repeated test execution.
- Keep responsibilities thin: the coordinating AI plans any targeted tests; the assigned Codex prepares and repairs the candidate and records validation or justified skips; the host enforces publication conditions and stores evidence. Inspect the actual repository configuration (for example, `.lvk/integration-validation.json` when present); the configured commands are reference inputs, not a mandatory host-run suite. Verify live execution capabilities.
- When existing validation evidence is sufficient, skip validation and report the rationale and remaining risks after completing the work. This includes omitting redundant additional validation based on the tested SHA, completed checks, and the auto-merge candidate's delta; it does not mean publishing unverified changes. Check conflict resolutions and newly unverified scope. This judgment does not waive required checks or override an actual execution denial.
- Run tests required before merge/publication on the auto-merge candidate and obtain passing results before publishing it. Tests explicitly acceptable after merge but before deployment may be grouped at that later stage; do not silently move pre-merge requirements there. Report passed, failed, not run, deliberately skipped, and real-model behavior not exercised distinctly.
- Auto-merge starts its assigned Codex immediately in one normal conversation. Read conversation and integration evidence before diagnosing a stalled run; acceptance is not completion.

- Never merge, publish, answer execution approvals, or alter significant design decisions without appropriate authorization. Respect LVK's revision, commit, merge and permission checks.

- For auto-merge, distinguish merge success, target publication, and OpenWiki maintenance as separate outcomes. Verify and report each applicable outcome; never infer Wiki success from a successful merge. When explicitly asked to monitor the complete workflow, include Wiki completion. Otherwise, do not prolong the coordinating assistant's turn solely to wait for Wiki.

## Resume, transfer and cleanup

For the same objective, first assess the existing Issue, Workspace and Session using [execution.md](execution.md). Failure does not itself require a replacement.

- Before fully moving work to another Issue, explain why replacement is preferable to continuing the existing Issue and verify that required results/evidence and remaining work are handed over. Record the migration reason and new-Issue link on the old Issue, and a link back to the old Issue on the new one.
- After a full handoff, use `Cancelled` if the old Issue will not continue, or `Done` if its own objective was achieved. Do not close the old Issue for a partial migration. Preserve each past run's actual outcome; later success must not rewrite earlier failure or cancellation.
- When the AI marks the old Issue `Cancelled` as part of a migration, complete the cleanup gate below.

## General Issue and Workspace archiving

Apply these rules to every Issue or Workspace archive, including ordinary archive requests unrelated to a transfer. First verify the deployed implementation's behaviour; do not assume archiving physically deletes a worktree. If it does, even an explicit MCP archive request requires one confirmation identifying the target and worktree deletion scope, unless the user has already approved that same target and scope. Do not execute while the target is busy or bypass a busy rejection. Verify and report actual archive and physical-deletion outcomes separately, including deletion failures. This confirmation does not replace other deletion safeguards.

## Explicit Issue deletion

Verify the deployed catalog before using `delete_issue`. This physically deletes
the Issue and related database records under existing Board constraints; unlike
archive, it cannot be reversed by unarchive. It does not physically delete
Workspace directories or worktrees. Obtain explicit authorization for the exact
Issue and respect host confirmation controls; implementation, merge or transfer
permission does not authorize deletion.

Remote MCP takes `project_id`, `issue_id` and a stable `request_id`, requiring
`lvk:read`, `lvk:write` and current Project access. Recover a lost response with
`get_operation(project_id, request_id)` or the identical original request.
Accepted/running/uncertain is not successful deletion. A new request for an absent
Issue fails; replaying a successful request returns the original receipt. Never
use a new request ID to retry an unknown outcome. stdio retains `delete_issue(issue_id)`.

## Cleanup completion gate

For an AI-directed complete transfer that leaves the old Issue Cancelled, record cleanup explicitly for the old Issue and every child Workspace. Confirm idle state and that required results, evidence and uncommitted work do not survive only in the old folders. Apply the general Issue and Workspace archiving rules above; this gate adds responsibility for cleanup after a transfer.

Read back Issue archive, each Workspace archive and physical deletion independently, including deletion errors or uncertain outcomes. Report cleanup as **unprocessed**, **awaiting confirmation**, **complete**, or **failed (with details)**. Cancelled alone is never cleanup complete. Awaiting confirmation holds only old cleanup; independent successor work can proceed. A human's simple Run cancellation or Cancelled status change grants no automatic archive permission.

For LOCAL-236 → LOCAL-238, reciprocal links and Cancelled satisfy only transfer/status steps. The gate must still report old Issue/Workspace archive and physical deletion as unprocessed or awaiting confirmation until verified. This example is a walkthrough, not authorization to archive those records again.
