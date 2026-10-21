---
title: "ChatGPT MCP implementation and acceptance record"
description: "Implemented scope, service reuse, T01–T18 evidence and local validation limits for LVK's optional ChatGPT connection."
---

> The internal Astra assistant has since been retired. Astra references and test
> counts below describe the original implementation only. External MCP and
> Workspace question answering remain supported.

## Delivery status

M0–M3 and local M5 acceptance are implemented on `feature/chatgpt-mcp` in
`.worktrees/chatgpt-mcp`. **Implementation/local verification is complete;
real ChatGPT account verification is pending.** M4 is deferred as explained
below; T19 is not applicable. The requirements are the [supplied specification](../specs/lvk-chatgpt-mcp-spec.md).

The feature is not deployed or merged into the target. The live checkout remains
at `a5c779b7`, which already disabled Astra's automatic Workspace monitoring.
This feature does not restart that monitoring, migrate its conversations or
change saved provider credentials. No store submission, external publication,
user-authentication change or existing-job stop was performed.

Use the [connection and operation guide](chatgpt-mcp-setup.md) for provider setup,
server configuration, plugin generation, ChatGPT Web installation, refresh,
revocation and rollback. Historical incremental evidence is retained in Git;
this record describes the final implementation rather than superseded plans.

## Connection and ownership

The existing Rust server exposes opt-in `rmcp` 1.8 Streamable HTTP at `/mcp`.
`LVK_MCP_CONFIG` names an operator-owned JSON file; without it the endpoint is
absent. Invalid/incomplete configuration prevents startup. Existing stdio MCP
remains a separate tool set and transport.

A maintained external OAuth authorisation server performs login, PKCE, client
registration and revocation. LVK performs RFC 7662 introspection over HTTPS on
every request, without a positive token cache. It checks active Bearer type,
exact issuer and owner subject, resource audience, expiry/not-before and scopes.
`lvk:read` is always required; mutations additionally need `lvk:write`, and formal
Integration needs `lvk:merge`. An operator Project allowlist applies to every
target. Model-supplied identity is never authentication. Invalid tokens return
401, missing read scope 403, and provider failure 503. Tokens/provider bodies
are not logged or exported; dynamic SQL/IO diagnostics are sanitised.

The supplied nginx launcher creates only two exact Basic-auth exceptions for
MCP and protected-resource metadata. Only MCP forwards Bearer; both strip
cookies, proxy credentials and relay signatures. UI/API/preview protection
remains in place. The optional Compose override affects only `lvk`. A public
prefix is an authenticated alias for the same application: root APIs/assets
remain required. Trusted HTML metadata configures TanStack's basepath; this is
not isolation for several applications under unrelated prefixes.

The official Tunnel was considered but not selected: its workspace access
boundary does not itself provide this server's verified per-request owner
identity. No second authentication mode, custom OAuth server or new inference
API key is introduced. Keycloak's documented resource-indicator setup is an
operator recipe, not a tested deployment; its feature status and the actual
provider/client login must be verified by the operator.

## Reused services and added boundaries

| Capability | Implementation and reuse |
| --- | --- |
| Board | `routes/local_remote.rs` normal Issue create/update, with checked revisions and shared repository selection; no new Issue kind or automatic start on creation |
| New execution | `remote_mcp/launch.rs` calls `workspaces/create.rs::create_and_start_workspace_admitted`; native executor defaults, Goal mode/budget and setup-gated Session/AgentRun paths remain authoritative |
| Continuation/steering | Existing `sessions::follow_up_checked` and `agent_runs::dispatch_control`; exact Workspace/Session/current-run validation and saved executor configuration |
| Questions | Existing Session question reservations, locks and blocking-input guards; revisions bind exact event/attempt; direct answers and expired attempts invalidate old requests |
| Formal Integration | Existing `integrations::create_with_expected_target`, admission lock, target/source checks, frozen validation, publication and Wiki handling; no MCP `git merge` path |
| State/activity | Existing Project/Issue/Workspace/Session/run/event/command records; reads observe direct UI/Codex changes without inference |
| Memory/Wiki/verification | Read-only existing repository-memory store, bounded OpenWiki snapshot and immutable per-run Change Manifest; no provisioning, maintenance or other Workspace's private memory |
| Code/diff/merge observations | Existing GitService plus libgit2 read helpers; no CLI staging, filters, hooks, fetch or target writes during reads |
| Links | Existing Project/Issue/Workspace routes and AppNavigation; validated public HTTPS base and authenticated prefix alias |

The remote boundary validates actual membership, not just the caller's parent
IDs. Lists default to 20 and cap at 50; text pages default to 8,000 and cap at
16,000 characters. Responses expose truncation, further pages and revisions.
Code reads allow scoped tracked files, reject private/escaping paths and use
Unix `openat`/`O_NOFOLLOW`/`O_NONBLOCK`; binary/oversized/unsupported content is
explicit. Each source side is limited to 512 KiB. Non-Unix reference-file reads
are unsupported rather than falling back to an unsafe implementation.

A single migration, `20261013000000_remote_mcp_operations.sql`, adds durable
request receipts. Existing Astra receipts require a conversation/message and
cannot serve this boundary without creating unwanted chat state. The new unique
`(verified issuer + subject, request_id)` identity, canonical input hash and CAS
admission prevent identical retries from owning the action twice. Different
input under the same key conflicts. Receipts store safe target IDs, timestamps
and results; existing domain command/question/Integration records remain the
execution authorities, not a second task ledger.

An owned task is created before asynchronous admission so losing the MCP caller
does not cancel accepted work. Start calls wait only for native admission and
return the receipt plus Workspace/Session/run or Integration identity and links.
Unknown outcomes are never blindly replayed. Previous-process unfinished
receipts and unreadable terminal evidence return `uncertain`. A formal admission
error is reconciled under the canonical lock: an exact durable record establishes
acceptance (even if blocked), confirmed absence establishes rejection, and
mismatch/query failure remains uncertain.

Recorded tests apply to their manifest's source commit, not automatically to the
current dirty worktree. Merge `included` means observed local history or an exact
formal publication receipt, not semantic correctness, tests, remote publication
or Wiki completion. Missing/sparse/ambiguous Git evidence stays unknown. The
result projections distinguish implementation, validation, publication and Wiki
states, including skipped/unavailable/unknown checks and partial outcomes.

Runtime dependency changes are the maintained rmcp server transport, reqwest's
form feature and the existing nix dependency's fs feature. Python/jsonschema/
PyYAML are package-validation tools only; no new application runtime is added.
The package adds one 57-line Skill, two references, official plugin/MCP manifests,
configuration and validation scripts, examples and the connection guide.

## Public tools and contracts

The actual Rust catalogue is returned by `tools/list`. All require `lvk:read`; only the
last three rows add mutation scopes. Full input schemas come from `tools/list`;
[checked input examples](../../scripts/chatgpt-mcp/examples.json) cover every
tool. No remote shell, arbitrary URL proxy, database editor or credential reader
is exported.

| Tools | Target/input and result |
| --- | --- |
| `get_connection`, `list_projects` | Connection capabilities/scopes and allowed Project IDs/links; no global current Project |
| `get_project`, `list_project_statuses`, `list_project_sessions`, `list_issues` | Project ID; bounded status/counts, Issues and current Sessions/questions |
| `get_issue`, `list_issue_workspaces` | Project/Issue IDs; paged requirements or distinct 1:N Workspace attempts |
| `get_workspace`, `list_workspace_runs`, `get_execution` | Project/Workspace and selected run/section; actual lifecycle, input/final response, Goal usage and evidence links |
| `get_resource_observation` | No arguments; bounded on-demand Linux system-visible statistics and separate cgroup limits |
| `list_run_events` | Exact Project/Workspace/Session/Run; saved commentary and safe execution metadata, bounded cursor pages |
| `list_run_commands`, `get_run_command` | Exact Project/Workspace/run and optional command ID; delivery state and paged direct instructions, never implied implementation completion |
| `list_questions`, `get_question` | Project/Workspace/Session, then run/question/kind; exact question/options/revision; execution approvals link to LVK |
| `list_project_repositories`, `list_repository_branches`, `get_execution_defaults` | Project and repository; eligible IDs/branches and current configured executor/native mode |
| `get_integration_candidate`, `list_integrations`, `get_integration` | Project/repository/Workspace/target branch or Integration ID/section; observed heads, validation, publication and Wiki state |
| `list_project_merge_status`, `get_workspace_merge_status` | Project or Project/Workspace/repository; bounded included/unmerged/unknown observations |
| `get_workspace_memory`, `list_workspace_wiki`, `get_workspace_wiki_page` | Project/Workspace/repository and selected page; read-only bounded evidence with revision |
| `get_workspace_verification`, `get_workspace_verification_test` | Project/Workspace/repository/run and optional test index; manifest-scoped counts/details, not current-worktree proof |
| `list_workspace_files`, `get_workspace_file`, `list_workspace_diff`, `get_workspace_diff` | Project/Workspace/repository and selected relative path; bounded safe code/diff reads |
| `get_operation` | Project + original request UUID; this verified owner's durable acceptance/result/uncertainty |
| `create_issue`, `update_issue` (`lvk:write`) | Project/request IDs; title/spec/status, plus Issue/revision for updates; normal Issue only, no execution |
| `start_workspace`, `steer_execution`, `continue_execution`, `answer_question` (`lvk:write`) | Stable request UUID plus current Issue/repositories or exact Workspace/Session/run; questions additionally require ID/kind/revision; returns acceptance and target IDs |
| `start_integration` (`lvk:merge`) | Project/request/repository, target branch/head and explicit adopted Issue/Workspace/source heads; existing formal admission receipt |

`start_integration` accepts optional `skip_validation`: `true` skips additional
Integration validation (unverified), `false` runs it, and omission or `null`
inherits the server's `integration_skip_validation` preference at admission.
The start receipt's `result.skip_validation` and every `get_integration` section's
`skip_validation` report the saved, resolved boolean, even after the preference
changes. Skipping is not a validation pass and does not bypass admission, Git,
conflict approval, writer settlement, publication or Wiki requirements.
Retry with the same `request_id` and identical payload; changing the flag,
including adding or removing it, follows the existing payload mismatch rule.

## T01–T18 acceptance audit

All rows below passed locally at the stated boundary. External execution uses
fixtures/mocks as required by specification §14.2. These are not claims of live
ChatGPT login, paid Codex execution or production deployment. Test functions live
under `crates/server/src/remote_mcp/` unless a different location is named.

| ID | Evidence inspected and verified |
| --- | --- |
| T01 | `tests::http_discovery_authentication_and_tool_call_roundtrip` exercises HTTP initialise/list/call; `crates/mcp/.../stdio_tests.rs` exercises the existing byte-stream codec/list/API call and CLI argument tests pass |
| T02 | Configuration/claims/HTTP tests reject anonymous, expired, revoked, foreign-owner/audience/issuer and insufficient scopes; Board/runtime/launch tests reject mixed Project and child IDs; nginx tests preserve authentication outside exact MCP paths |
| T03 | Project/Board/evidence/merge tests assert unchanged work/run/chat/operation counts and no filesystem provisioning; read routing has no launch/runtime inference call |
| T04 | `issue_writes_reuse_board_services_and_enforce_scope_retries_and_revision` checks normal Issue content, revision and no run; launch test separately checks agreed specification, native mode and returned identities; Skill uses the self-contained Issue template |
| T05 | `disconnected_start_waiter_does_not_cancel_domain_admission` aborts the caller then observes one durable accepted run; launch result explicitly says implementation is incomplete; canonical creation dispatches server-owned immediate/setup-gated work |
| T06 | Board fixture has two attempts per Issue; `runtime_changes_pin_parent_ids_current_run_and_lifecycle` checks exact dispatch, stale run/session rejection and live-versus-terminal routes; native Session regression checks provider/resume guards |
| T07 | Question projection preserves options/free text and IDs; `question_answers_share_real_reservations_and_reject_direct_duplicates` races UI/MCP through real reservations, checks one delivery and rejects already answered/wrong event/attempt; write-scope checks apply before dispatch |
| T08 | Approval projection returns `requires_lvk` and a canonical link without private command details; answer schema/handler rejects approval kind |
| T09 | Board and command fixtures change state through the canonical stored records and observe it on the next MCP read; command delivery is explicitly not implementation completion; direct answers disappear and new direct runs invalidate old targets |
| T10 | Runtime, verification, merge and Integration tests distinguish blocked/running/failed, test pass/fail/unavailable/skipped/unknown, dirty/unmerged/ambiguous Git, publication and recorded-versus-current Wiki evidence |
| T11 | Scoped-head launch test passes explicit selections to existing formal admission; canonical Integration regression checks target races, required validation/exceptions, exact publication, lost receipts and frozen Wiki inputs; no remote target-write mechanism exists |
| T12 | Operations tests race duplicate admissions, reject changed input and disconnect during admission; runtime/question/launch tests check one dispatch and stable identity; lost post-delivery response stays uncertain without replay |
| T13 | File-backed receipt test closes/reopens SQLite and changes process generation; `reopened_database_preserves_work_questions_and_uncertain_operation_identity` also reopens the full fixture DB and re-reads Workspace/run/question IDs, with no dispatcher or new work; this tests adapter recovery, not a live provider crash |
| T14 | 105-item Project/Issue/command/branch/Integration/verification/code fixtures exercise bounded complete traversal; text tests verify character limits, next offsets and required revisions |
| T15 | Tests reject stale cursors/revisions, deleted/foreign Workspaces, invalid/missing evidence and provider outages; damaged terminal receipts return uncertainty without leaking raw data |
| T16 | Native nginx tests cover root/prefix and auth/header boundaries; Chromium loads the actual TanStack route tree/AppNavigation behind a disposable HTTPS Basic challenge, checks target IDs, prefix, navigation and reload; business-data rendering/live-account permissions remain in the operator checklist |
| T17 | Full source diff preserves Astra UI/history/model/auth/settings and disabled monitor; only two helpers move to shared services. Astra's 29 related tests pass; remote fixtures assert no chat/run creation on reads and no upper-model call path |
| T18 | Official schema validation, 40 actual Rust tool input/scope examples, Skill/reference/dependency consistency and configured-package tests pass (both source and configured HTTPS/subpath distribution). Distribution contains no credentials; URL validation rejects credential/query/fragment shortcuts |

## Final validation record

Production changes are in commits through `1efa6851`; the final audit commit adds
only the file-backed restart test/fixture extraction and this consolidated record.
The checks below cover their affected source; there was no intervening production
edit after the relevant successful check. Cargo uses
`CARGO_TARGET_DIR=/var/tmp/lvk-target`; no feature runtime server binary was built
or deployed. The full approximately 1,000-test suite was intentionally not run.

| Check | Result / duration | Saved log |
| --- | --- | --- |
| `cargo test -p server --lib remote_mcp:: -- --nocapture` | 32 passed, 16.77 s; build 49.92 s | `/tmp/lvk-mcp-receipt-integrity.log` |
| Final Issue-write style recheck | 1 passed, 0.59 s | `/tmp/lvk-mcp-final-issue-recheck.log` |
| File-backed restart test | 1 passed, 2.79 s; build 32.11 s | `/tmp/lvk-mcp-final-restart.log` |
| `cargo test -p mcp --lib --bin vibe-kanban-mcp` | 10 library + 2 CLI tests passed, 0.14 s + <0.01 s; build 4 min 08 s | `/tmp/lvk-mcp-stdio.log` |
| `cargo test -p server --lib routes::sessions::agent_run:: -- --nocapture` | 10 passed, 0.54 s | `/tmp/lvk-mcp-regression-native.log` |
| `cargo test -p server --lib routes::project_chat:: -- --nocapture` | 29 passed, 12.84 s | `/tmp/lvk-mcp-regression-astra.log` |
| `cargo test -p server --lib routes::integrations:: -- --nocapture` | 16 passed, 12.16 s | `/tmp/lvk-mcp-regression-formal.log` |
| `cargo test -p utils --lib reference_file:: -- --nocapture` | 1 passed, <0.01 s | `/tmp/lvk-chatgpt-mcp-code-files.log` |
| `cargo clippy -p server -p mcp -p git -p utils --all-targets --features qa-mode -- -D warnings` | Passed; 2 min 55 s recheck | `/tmp/lvk-mcp-final-clippy-recheck.log` |
| `LVK_TEST_NGINX=/usr/sbin/nginx LVK_REQUIRE_NGINX_TEST=1 node --test deploy/server/mcp.test.mjs deploy/server/nginx.test.mjs` | 6 passed, 10.18 s; no skips | `/tmp/lvk-mcp-subpath-ingress.log` |
| `node --test deploy/server/server.test.mjs` | 13 passed, 1.95 s | `/tmp/lvk-mcp-ingress-server.log` |
| `node --test deploy/server/development.test.mjs deploy/server/instance.test.mjs` | 4 passed, 0.78 s | `/tmp/lvk-mcp-ingress-regression.log` |
| `PLAYWRIGHT_BROWSERS_PATH=/opt/lvk-browsers node --test tests/server/mcp-links.test.mjs` | 1 passed, 113.31 s | `/tmp/lvk-mcp-links-browser-final.log` |
| Local-web typecheck / changed-file ESLint | Passed | `/tmp/lvk-mcp-subpath-types-final.log`, `/tmp/lvk-mcp-subpath-lint.log` |
| Actual Rust catalogue + package/schema/Skill validation | 1 catalogue test, 40 valid tool inputs; 2 configuration tests in 0.009 s | `/tmp/lvk-mcp-final-package-catalog.log`, `/tmp/lvk-mcp-final-package.log` |
| Final test-only `cargo clippy -p server --lib --tests --features qa-mode -- -D warnings` | Passed, 3 min 57 s | `/tmp/lvk-mcp-audit-clippy.log` |
| `pnpm run format` and `git diff --check` | Passed; temporary dependency links removed | `/tmp/lvk-mcp-audit-format.log` |

Initial failures were corrected, not hidden: missing reqwest form support;
flattened DTO unknown-field handling; seven new-code Clippy style findings;
root-package formatter/typecheck dependency lookup; browser fixture HMR wiring.
The Clippy argument-count exceptions are narrowly scoped and reasoned at explicit
service boundaries. No tests were disabled. Rust's dummy-frontend warning does
not establish rendered UI coverage. Native nginx/browser tests are isolated
fixtures; Docker rollout and real account/provider login were not performed.
MCP-created Integrations inherit the saved server validation preference at admission.
The admitted choice remains fixed across retries and resume; skipped runs are
recorded as unverified. Running a regression suite here is not target publication.

## Conditional UI and remaining operator actions

M4 is deferred after examining `KanbanContainer`, `KanbanCardContent` and
`IssueTaskAttemptCard`. They depend on application/auth/action contexts,
translation/theme and mutating controls; there is no standalone read-only MCP
card or existing MCP Apps SDK/bridge. Reuse would require a separate iframe
entrypoint/bundle, projection, translation/theme setup, Rust resource metadata,
CSP, image packaging and security/fallback tests. This exceeds a small component
adaptation. No full-app iframe or unimplemented widget is advertised.

The [official MCP UI guide](https://developers.openai.com/plugins/build/chatgpt-ui)
was checked on 2026-10-13 and describes the separate MCP Apps resource/bridge/CSP
path. Current text/structured results and links work without it. Automatic
notifications, MCP Events, personal-memory synchronisation and Claude support
are not implemented or promised.

The operator must still configure a real HTTPS/provider/client and owner/Project
allowlist, perform an authorised rollout, install the connection plus Skill in
ChatGPT Web and execute the guide's disposable-Project checklist. Record date,
plan/conversation mode, installation path, read/write/consent results, link
return, safe Integration and personal-memory settings/observation. No personal
memory content belongs in fixtures. Web availability, OAuth login and actual
conversation-mode memory behaviour are explicitly unverified.

The portable manifest follows the published Agent Plugins 1.0 schema; the
OpenAI package example's per-server OAuth extension is rejected by that schema.
The package therefore uses standard type/URL fields and the host's public-client
configuration. The guide distinguishes Web Plugin Creator/workspace import from
merely writing a local plugin folder. Disconnecting/revoking the new grant and
disabling `LVK_MCP_CONFIG` withdraws the feature without deleting work or changing
Codex/Astra authentication. Returned code/specification/evidence goes to ChatGPT
under the user's host data settings.

## Read saved Codex progress with `list_run_events`

Discover IDs through `list_workspace_runs`, then call `list_run_events` with
`project_id`, `workspace_id`, `session_id` and `agent_run_id`. Optional `issue_id`
asserts the Workspace's current Issue link. The ordinary HTTP MCP authentication,
active-member check and `lvk:read` scope apply; the tool also checks the complete
Project → Issue → Workspace → Session → Run relationship. Existing membership
policy grants active members all Projects; this tool introduces no new Project
ACL or wider access policy.

`limit` defaults to 20 and accepts 1–50 **scanned canonical records**, including
records withheld from publication. Results contain `items`, `scanned_count`,
`omitted_count`, `has_more`, `next_cursor`, current `status` and
`projection_status`. Each item has its event ID, attempt ID/number, sequence,
recording timestamp and a small `event` object. Serialised item data has a
128 KiB page budget; a byte-limited page stops before the next unread record. Records are ordered by attempt
number and sequence, matching UI history. Sequence numbers restart per attempt.

Pass the returned `next_cursor` as `cursor` to read the next batch. Retain it even
when `has_more` is false, then reuse it for incremental reads later, including
from another connection or after a server restart. A filtered page can have no
items and still have `has_more: true`. An initially empty Run returns a null
cursor; omit it on the next call. Unknown, deleted or foreign-Run cursors fail
instead of silently switching Runs. Each call reads target relationships,
lifecycle and events in one database snapshot; this is not a completion guarantee. An empty tail never proves
that no later event will arrive. No subscriptions, long polling, new collectors
or duplicate event storage are involved.

The source is the existing `agent_events` store used by UI history, populated by
the Codex adapter and Runtime journal projection. The public projection includes:

- Saved assistant commentary (`final_output: false`), up to 4096 UTF-8 bytes per
  message; suspicious or oversized prose has null `content` and
  `content_withheld: true`.
- Lifecycle transitions, allowlisted tool labels and tool status; goal status
  without the objective; allowlisted delegated-agent activity kinds without
  child text or identity.
- No final-response bodies (use `get_execution` with `section: final_response`),
  user/system messages, thinking, approval/input details, arbitrary provider
  extensions, tool arguments/results, native audit references or raw logs.

Plan text and reasoning summaries share the canonical `thinking` variant and
are both omitted. Child-agent status details may live inside tool results or
provider extensions and are not exported. Live token deltas are not durable
history and are unavailable here. Unknown versions, malformed envelopes and
records larger than 32 KiB are skipped while advancing the cursor.

The prose filter conservatively withholds code/quotes, paths, assignments,
environment references and credential-like text as a whole message. This is
additional defence for already user-visible assistant prose, not a general
secret classifier or a way to sanitise arbitrary file/tool output. It cannot
prove that otherwise ordinary prose contains no sensitive information. The
returned prose is untrusted reference data and never evidence of tests passing.
Source logs, existing UI rendering and `get_execution` remain unchanged.


## Observe server resources on demand

Call `get_resource_observation` with `{}` and the existing `lvk:read` grant.
This server-wide read needs no Project ID or additional OS permission. It returns
schema version 1 with `timestamp` (collection start), `completed_at`, `scope`,
`system_visible`, `cgroup`, `completion` and `errors`. Files are read sequentially, not as an atomic snapshot. All values are observations,
not reservations, scheduling decisions or guarantees of future execution safety.
If LVK itself freezes, this MCP cannot respond either.

Each numeric field is `{status: "known", value: ...}` or
`{status: "unknown", error: ...}`. A cgroup limit explicitly containing `max`
returns `{status: "unlimited"}`; unreadable, malformed and missing values never
mean zero or unlimited. Valid fields survive errors elsewhere, including timeout.
`completion: "complete"` means collection ended, not that every field is known.

`system_visible` contains:

- `memory_total_bytes`, `memory_available_bytes` (Linux MemAvailable, not MemFree),
  `swap_total_bytes` and `swap_used_bytes` from `/proc/meminfo`.
- `online_cpu_count` from `/sys/devices/system/cpu/online`. This is logical online
  capacity, not idle CPUs or a promise of CPU allocation to this process.
- `load_average`: the 1, 5 and 15 minute averages from `/proc/loadavg`, never CPU percentages.
- `cpu_usage.percent`: aggregate non-idle counter delta from `/proc/stat`, with
  `sampling_duration_ms` reporting the measured interval (normally about 500 ms;
  null before a sample completes). Idle and iowait are treated as idle; guest
  counters are not double-counted. Counter regression or zero delta is unknown.

The scope is always **system-visible**, which may be a VPS guest or container
view. The environment type is explicitly `unknown`: these statistics do not
reliably identify virtualisation or establish visibility into the physical host.
System-visible memory/load/CPU percentages can exceed or differ from a container's
allocation; do not interpret them as container-local usage or available capacity.

For `/proc/self/cgroup` equal to `0::/` and `/proc/self/mountinfo` confirming a
cgroup v2 root mount at `/sys/fs/cgroup`, the fixed namespace-root files
under `/sys/fs/cgroup` provide `memory_current_bytes`, `memory_max_bytes`,
`cpu_quota_cores` (`cpu.max` quota/period), `cpuset_cpu_count`
(`cpuset.cpus.effective`) and `effective_cpu_bound_cores`. The latter is the minimum
of online CPUs, a known quota (or explicit unlimited) and the effective cpuset;
it is unknown if any required input is unknown. Hidden ancestor limits remain
unknown and can further reduce capacity. A namespace-root `max` does not prove
that every ancestor is unlimited. cgroup v1, nested process membership and other
mount layouts are unsupported/unknown; the tool never substitutes mount-root
statistics for a known nested process cgroup. There is no estimated memory headroom.

Collection reads only fixed Linux statistics paths, accepting at most 64 KiB
per file (one extra byte detects oversized input). The
fixed output contains only numbers, timestamps and constant labels/errors, with
an enforced 8 KiB JSON limit; it never includes raw files, process lists, arguments or filesystem paths.
At most two dedicated worker threads per server process are admitted, without a
waiting queue. A busy caller gets a retryable tool error. The response deadline is
two seconds, preserving received fields. Sampling never occupies Tokio's common
blocking executor. A worker retains its slot until it actually exits, including
when a caller disconnects or times out; a stuck kernel read cannot accumulate
unbounded workers. No daemon, history database, UI, automatic admission or stop
policy is created. Authentication and the normal MCP audit remain unchanged.

Validation uses parser/collector fixtures and timeout/concurrency tests. The
ignored `resource_observation::tests::live_smoke` test is an explicit small live
read, without stress or environment changes. Live Auth0/ChatGPT connectivity and
other operating systems are separate from fixture coverage.
