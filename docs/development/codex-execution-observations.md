# Codex execution observations

Workspace chat uses the existing AgentRun canonical events and replaceable usage snapshots. No child scheduler or separate persistence model is introduced. The native protocol baseline is the pinned `rust-v0.144.1` source; this change does not upgrade the CLI.

## Usage

`thread/tokenUsage/updated` supplies `tokenUsage.last.totalTokens` for latest parent context occupancy and `modelContextWindow` for capacity. `total.inputTokens` plus `total.outputTokens` supplies cumulative usage; cached input is already part of input and is not added again. Context is the last provider measurement, not a live estimate of an in-flight request. Compaction can lower it.

Optional context fields travel in the existing usage snapshot JSON through host journal, database and WebSocket replay. No migration is required. Older snapshots/canonical token events retain cumulative usage with unknown context. No model-specific capacity is guessed. Missing/invalid capacity produces no percentage. Child or unscoped usage is excluded from parent usage. Starting a new Run or attempt clears the previous Run's gauge until a measurement arrives.

## Children

`subAgentActivity`, child `turn/started`/`turn/completed`, and `collabAgentToolCall.receiverThreadIds`/`agentsStates` provide observations. A spawn supplies immediate parentage and, when available, its prompt. Interactions do not establish parentage. Root activity is not shown as a child. Tool completion is not child completion; an interruption request does not prove termination. A message or resume with no fresh child state invalidates earlier completion to unknown.

Existing collapsible task cards show only observed children in their own Run. They keep the last observed lifecycle separate from answer/tool details. After a Run ends, an unresolved child is labelled unknown with its last observed state, not displayed as still actively streaming. Replaying canonical events reconstructs the cards on reload or another device. Historical Runs without child observations cannot be reconstructed by guessing.

Codex mapper v5 adds these observations. The v4 replay mapper retains the previous collaboration projection; earlier mapper entrypoints remain available. Persisted canonical events need no rewriting. New CLI fields are optional and unknown child states are not treated as successful completion.

## Completion and integration

Before the Codex client emits success (including Goal/Plan completion), every observed child must have terminal evidence. Running, pending, unknown, or missing child state changes the parent exit to Failure. This prevents the existing host shutdown from aborting unresolved child work while recording a successful Run. It deliberately does not wait indefinitely or invent a continuation. A child's terminal failure may have been handled by the parent and does not itself force parent failure.

Executor Failure becomes AgentRun Failed. Automated orchestration handoff requires source-node and AgentRun success; an Integration Agent must succeed before its proposal is accepted. These gates remain unchanged. Explicit adoption of an immutable source commit is a separate user action: it checks writer settlement, not that the last coding Run succeeded.

Coverage is limited to observations delivered by Codex. An unannounced child or work detached outside the provider containment cannot be inferred. A version that omits child terminal notifications may fail closed even after useful parent work; report this compatibility limit rather than declaring success. The [anonymised usage fixture](../../crates/executors/src/executors/codex/usage-observation.fixture.json) preserves a real notification captured from this task’s own Native Audit (134,953 latest tokens / 258,400 context capacity versus 6,347,875 cumulative tokens). Native child activity and child turn completion were also observed in that run. This does not substitute for browser/reconnect testing against a deployed build.


## Validation in this workspace

- Codex executor tests: 133 passed, 6 ignored initially; final event/guard regression run: 134 passed, 6 ignored, with the unchanged slow MCP startup test excluded after its initial successful run. The later legacy usage fallback regression also passed separately.
- Database snapshot round-trip and host journal usage replay: one focused test each, both passed.
- Frontend Agent Runtime model/render tests: 35 passed, including unknown context, historical Runs, replay, child disclosure and lifecycle changes.
- `pnpm run generate-types` and `pnpm run generate-types:check`: passed.
- `pnpm run ui:check`, `pnpm run web-core:check`, `pnpm run local-web:check`: passed.
- Scoped ESLint for `ContextUsageGauge.tsx`, unused-i18n-key check and `git diff --check`: passed.

No deployed-build browser reload/reconnect, separate-device session or full Integration validation was performed. No CLI version update, merge, publication or deployment was performed. Native event observations above came from this task's own running agent, not from a deployed build of these changes.

The workspace's normal `target/` (approximately 25 GiB) and `node_modules/` (approximately 982 MiB) remain for build reuse, as required by repository guidance. Disposable validation logs were removed after recording results. The existing sccache server retained its default `/home/appuser/.cache/sccache` location rather than the requested shared cache path; it was not reconfigured.
