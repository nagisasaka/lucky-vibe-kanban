# Investigation

## Investigate with the least costly reliable evidence

1. Answer from current LVK MCP facts if sufficient, identifying verified state and any unknowns. Choose the investigation method for reliable evidence while considering data transfer, MCP round trips, available search/filtering, and agent startup/execution costs; do not use a rigid file-count threshold.
2. Before asserting a specific implementation behavior or defect, verify the relevant code and revision when accessible; otherwise explicitly label the claim a hypothesis or design proposal. Pure design discussions do not require source inspection. Use direct MCP reads for known state, diffs, or a small number of located code passages. A public GitHub branch may omit unpublished local changes; do not equate them.
3. For repository investigation, prefer `query_repository` when available: select the registered Project/repository, resolve a commit target (no branch Workspace required), and pin the returned full `source_head`. Use list/glob, explicit literal/regex grep with case selection, line read, fixed-head log and direct/merge-base diff. Trace entrypoints, imports and usage before deciding which component runs. Repeat original arguments with `next_cursor` as `cursor`; check `complete`, `search_complete`, exclusions and stop reasons. First line reads need no prior revision; use returned revisions for consistency. Workspace mode is live/non-atomic and changed revisions require restart; cursors expire on server restart. stdio and remote share the read service. Do not infer absence from unsearched scope or automatically start another agent merely to interpret results. If independent agent investigation is explicitly requested, select or reuse the appropriate Issue, Workspace, and Session, then start the authorized AgentRun with the agreed scope and boundaries.
4. Distinguish *not exposed through current MCP*, *not found in inspected source*, and *verified absent*. Missing a tool in a catalog is not proof of missing implementation. If a needed capability is not visible, make one relevant discovery check when a discovery mechanism is available before declaring it unavailable; distinguish current tool exposure, permission failures, and verified server-side absence. Do not claim deployment mismatch without evidence.
5. Track submissions by stable `request_id`, and use operation receipts after uncertain results; never duplicate the action with a new request ID just because a response was lost.
6. Retrieve an agent's findings when asked or explicitly waiting, and check material claims against code, tests, or auto-merge evidence as appropriate. Completion messages alone do not prove tests, publication, or Wiki success.
7. Review implementations against agreed requirements and intended user workflows. Distinguish confirmed defects, unverified risks or coverage gaps, and product decisions requiring user judgment. Recommend minimal verification or fixes. Surface security/functionality trade-offs rather than silently narrowing agreed capabilities, independently relaxing safety constraints, or changing the product contract.
8. For source investigation, first inspect currently exposed search and range-read tools and their live schemas. Restrict searches to known directories or files, reuse returned context when sufficient, and batch only missing ranges with expected revisions when more evidence is needed. Check completeness, stop reasons, and exclusions; never infer absence from no matches in an incomplete search. If repeated spelling guesses or broad searches accumulate, reassess the search strategy or ask whether a separately scoped investigation is wanted.

### Selective pre-implementation specification review

- **Use REVIEW mode for Codex review-only requests when supported; otherwise use ordinary CODE, not Plan mode.** LVK's Plan-mode flow can proceed from planning into implementation, so it is not a review-only boundary. Explicitly state the review scope and instruct the agent to stop after reporting the review, without adopting product changes or starting implementation unless the user separately authorizes it. Necessary Workspace-local investigation scripts, reproduction fixtures, and tests remain allowed within the agreed review scope; this rule does not impose a blanket read-only filesystem restriction.
- Before implementation, selectively propose a read-only Codex specification review when a change crosses components, affects high-impact data deletion/update/restart behavior, or relies on unknown runtime behavior or implicit dependencies. Do not impose this extra gate on every task, especially small UI wording or appearance changes.
- First agree the objective, constraints, and non-goals with the human through the coordinating AI. Then have Codex compare that agreed specification with the current implementation, citing the inspected revision and relevant code. Return verified facts, concerns or unknowns, and decisions requiring human judgment separately; recommend the smallest justified specification corrections, not scope expansion.
- A specification review authorizes neither implementation nor specification changes. Bring material findings back for agreement and start implementation only with applicable approval; do not repeat confirmations already explicitly given. This early review complements, rather than replaces, implementation review and risk-based tests.

### Share Issue specification reviews through comments

- When asked to review an Issue, follow the agreed review-sharing workflow and the user's recording authorization: preserve the review as an Issue comment rather than replacing agreed requirements. Honor requests for a chat-only review. Clarify only when recording intent is genuinely unresolved; do not repeat an already authorized confirmation or treat every informal discussion as permission to post.
- Read the Issue with `get_issue` and retain its `review_target`. Post the review using `add_issue_comment`, that exact target, and a self-reported `reviewer_label`; the label is not a verified model identity. If the Issue changes, keep the reviewed revision rather than silently relabelling the review as current. Use `list_issue_comments` and `get_issue_comment` to read other reviews, including full paginated bodies when previews are truncated.
- Keep a stable `request_id`; resolve uncertain writes with `get_operation` and read back the saved comment before claiming persistence. In chat, report the main findings and the verified Issue link, distinguishing a saved review from an unsaved draft. Do not require the human to copy review text between AIs.
- Comments are opinions, not adopted requirements or permission. Evaluate findings against the user's decisions; revise the Issue body only as a separate action within existing delegation and approval scope. Posting a review must not change status, start agents, or automatically notify, trigger another review, or create a review chain.

REVIEW uses the same Issue/Workspace/Session/AgentRun model and permission boundaries as CODE. It does not automatically post Issue comments or transition through PLAN approval into implementation. Report through the coding-agent Session and stop; return authorized follow-up implementation to the suitable existing implementation Session.

## Investigation boundaries and artifacts

- For investigation: choose the least invasive reliable method, allowing necessary Workspace-local scripts, reproduction code, fixtures, and tests within the agreed scope. An independent worktree does not isolate databases, credentials, background processes, or external services; verify their targets and avoid unauthorized shared or live-state changes. Report exact file paths, any investigation changes, and verifiable evidence.

- **Keep investigation-only artifacts out of Git commits by default.** Investigation reports, raw logs/JSON dumps, third-party source copies, and temporary reproduction scripts or fixtures belong in Git-untracked storage, with a concise findings summary and evidence references in the coding-agent Session's normal response; the coordinating assistant should summarize retrieved findings in the user's chat when requested or useful; use Issue comments only under applicable review-sharing authorization. Before creating them, verify that the chosen location or exclusion keeps them out of LVK's automatic commits; merely leaving files unstaged is not enough. At the end, inspect Git status and the final diff (including staged changes and commit contents when applicable) for accidental inclusion. Distinguish these temporary artifacts from product source, adopted regression tests/fixtures, and maintained documentation legitimately included in authorized implementation. Honor an explicit user request to preserve specific investigation artifacts in Git; do not interpret this rule as banning all non-product-code files.

## Help and bundled distribution source

For ordinary LVK usage, prefer `ask_help` / `get_help_conversation` when an agent
answer is useful; continuation uses the returned `session_id`. Report starting,
running, answered, failed and cancelled distinctly. Existing provider auth and
costs apply. This is the same REVIEW-based service as the Help UI, without a
hidden Project. Help is guidance, not authorization to edit/build/restart LVK or
create development work.

For direct evidence use `query_repository` with `source:"lvk"` and the existing
read/grep/list/log/diff operation, without Project registration or a host path.
Default commit is the bundled distribution revision; pin returned `source_head`
and show the version/revision. Explicit history/diff targets distinguish past
intent from current behaviour. Cite real commit messages/diffs and label your
interpretation; absent contribution documents do not justify invented PR rules.
GitHub and normal Project reads remain available. Workspace agents can also read
the bundled path exposed in their LVK guide. For requested development, explain
normal Project / Issue / Workspace and an independent public upstream clone at
the bundled commit; a help question alone does not start this work.
