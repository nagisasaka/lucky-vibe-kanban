---
title: "ChatGPT MCP integration implementation record"
description: "Connection boundary, reuse map and validation record for the opt-in LVK MCP entry point."
---

## Status

Implementation in progress on `feature/chatgpt-mcp`. The development checkout is
`.worktrees/chatgpt-mcp` inside the current workspace. The live checkout remains
at `a5c779b7`; this feature has not been deployed or merged. M0 has a first
transport/authentication/read implementation; M1–M3 and the acceptance audit are
not complete. The authoritative requirements are [the supplied specification](../specs/lvk-chatgpt-mcp-spec.md)
(version 1.0, 2026-10-13).

## Connection decision

Use the existing Rust server and `rmcp` 1.8 Streamable HTTP transport at `/mcp`.
Leave the existing `vibe-kanban-mcp` stdio binary and tools unchanged. The new
entry point is absent unless `LVK_MCP_CONFIG` names an operator-owned JSON file.
Invalid or incomplete configuration prevents startup. Do not change a running
server's configuration or restart it as part of source validation.

Use an existing, maintained OAuth authorization server. LVK is a resource
server, not an authorization server: code issuance, browser login, PKCE,
registration and revocation remain with the provider. The resource server uses
RFC 7662 introspection over HTTPS on every request, with no positive token cache.
It requires an active bearer access token, exact issuer, configured owner
subject, MCP resource audience, future expiry, and applicable scopes. Scope
names are `lvk:read`, `lvk:write`, and `lvk:merge`. A configured project allowlist
further restricts the single owner's access. Tool arguments cannot supply an
identity. An unavailable provider produces 503, never anonymous access.

Existing reverse-proxy Basic authentication and Codex authentication are not MCP
OAuth credentials. A future installation must route only `/mcp` and its protected
resource metadata to the OAuth-protected entry point; preserve existing Web/API
protection. Subpath deployments must preserve the configured public base URL and
strip that prefix consistently before routing to LVK. Login/link return-path
verification remains an explicit acceptance test.

The official Secure MCP Tunnel was evaluated: it avoids public ingress, but its
organization/workspace access boundary does not itself supply a verified
per-request owner identity to this LVK resource server. OAuth provides the
required revocation and scope checks without treating tunnel membership as a
user assertion. Do not implement a second transport/authentication option here.
The operator still needs to configure their authorization server and ChatGPT
connection; no account access or real ChatGPT connection has been claimed.

Sources checked 2026-10-13:

- [OpenAI authentication](https://developers.openai.com/plugins/build/auth)
- [Official Secure MCP Tunnel](https://developers.openai.com/api/docs/guides/secure-mcp-tunnels)
- [Plugin packaging](https://developers.openai.com/plugins/build/plugins)
- [RFC 7662 introspection](https://www.rfc-editor.org/rfc/rfc7662)
- [Keycloak OIDC endpoints](https://www.keycloak.org/securing-apps/oidc-layers)
- [Keycloak MCP authorisation](https://www.keycloak.org/securing-apps/mcp-authz-server)

Keycloak documents native resource indicators as experimental. A concrete
installation guide must pin a suitable provider version and verify its resource
binding and PKCE metadata. Do not silently assume any OIDC installation accepts
the MCP `resource` parameter. Pre-registering the ChatGPT client avoids adding
CIMD or anonymous dynamic-registration infrastructure for this single owner.

## Reuse map and remaining implementation

| Capability | Existing implementation | Required adapter work |
| --- | --- | --- |
| Local stdio MCP | `crates/mcp/src/task_server`, `vibe_kanban_mcp.rs` | Preserve global/orchestrator routers; remote tool subset is separate |
| Project and Issue | `routes/local_remote.rs`: `get_local_project`, `create_local_issue`, `update_local_issue_checked` | Bounded projections, project/owner checks and update revisions; reads must not call metadata-initialisation helpers |
| Workspace start | `routes/workspaces/create.rs`: `create_and_start_workspace` | Durable admission before side effects, Issue/repository checks, existing executor defaults and actual Goal configuration |
| Continue | `routes/sessions/mod.rs`: `follow_up` | Confirm current run/session/workspace; preserve stored executor configuration |
| Steering | `routes/agent_runs.rs`: `dispatch_control` | Reuse command identity/idempotency and validate latest run |
| Questions | `routes/sessions/questions.rs`: `pending`, `answer` | Exact question/run/Project mapping, duplicate/direct-answer handling; approval links remain separate |
| Formal Integration | `routes/integrations/mod.rs`: `create`, existing admission/runtime | Freeze explicit selections/heads, reuse validation/publishing/Wiki handling; no direct target writes |
| Memory, files, Wiki, diffs | Existing workspace readers used by `routes/project_chat/tools.rs` | Scoped bounded readers without entering Astra inference; shared extraction only if necessary |
| Runtime/operation audit | Existing run/event/command records | Durable unique operation admission with safe audit identity, recovery and uncertain outcomes |
| UI links | `packages/local-web/src/routes` and `AppNavigation.ts` | Exact Project/Issue/Workspace routes, validated HTTPS base/subpath |

Astra's event ingestion and automatic monitoring remain removed. Do not route
MCP calls through project chat, modify its prompts/settings/history, or create
an upper-model session. New reads use database projections and existing readers.

## Acceptance tracking

T01–T18 remain the completion criteria. Initial tests cover the HTTP handshake,
tool discovery/call, missing/expired/revoked/foreign-owner/insufficient-scope
authentication, provider failure, configuration rejection, bounded 105-project
pagination, allowlist filtering, stale cursor, and subpath link construction.
These do not yet prove write idempotency, execution independence, questions,
Integration, package installation, or browser login return behaviour. Record
actual test results after execution; do not count implemented tests as passed.

M4 is undecided until the required service and package are complete. No automatic
notifications or individual ChatGPT memory synchronisation are planned.

## Operation admission

The migration `20261013000000_remote_mcp_operations.sql` adds only remote request
receipts. Existing Astra operation records require a conversation and message,
so reusing them would create unwanted chat state. Existing Integration requests,
AgentRun command identities and question-answer reservations remain the domain
execution authorities.

The new table reserves `(principal, request_id)` before effects and stores a
canonical input hash, safe target IDs, process generation and receipt. Different
input with the same key conflicts. An unfinished receipt from a previous process
is exposed as uncertain and never acquires permission to replay. No generic
worker queue or inference loop is added. Domain adapters must still reconcile
existing IDs; a success receipt means the requested operation was accepted or
performed, not that coding, merging or Wiki publication has completed.

The helper now backs `create_issue`, `update_issue` and `get_operation`. The
verified issuer and subject form the receipt principal. Scope checks happen
before admission; target membership and current revisions are checked inside
the owned operation. The task is spawned before the first database await, so
an MCP disconnect cannot strand a newly admitted request before execution.
Uncertain outcomes are retained and never automatically replayed.

Issue creation uses the receipt ID as the existing Board service's supplied
Issue ID. It requires self-contained requirements and starts no Workspace or
model. Updates reuse the Board's revision-checked service, preserve omitted
fields, and return conflicts rather than overwrite newer requirements. That
service now advances its timestamp by at least one millisecond, even during
same-millisecond updates or a backward clock adjustment. No custom Issue table
or execution scheduler was added.

Insufficient write scope returns a standard MCP OAuth challenge. Inspect
`get_operation` with the original request key after a lost response; do not
create a new key merely because the original request timed out. Receipts return
operation state and canonical links without exposing input text or credentials.
Execution, steering, question-answer and Integration-start adapters are still
required; these Issue tools do not imply that M2 is complete.

## Validation record (foundation)

- `cargo test -p server --lib remote_mcp:: -- --nocapture`: 5 passed, 0 failed;
  200 unrelated server tests filtered out. Initial dependency/test build: 9m 11s;
  test execution: 0.21s. Log: `/tmp/lvk-chatgpt-mcp-tests.log`.
- `cargo fmt --all` and `git diff --check`: passed for this increment.
- The first `cargo check -p server --lib` found the missing reqwest `form`
  feature. It was enabled; the subsequent successful test build compiled the
  affected code. This failed attempt is not counted as a passed check.
- The build-script warning about creating dummy frontend assets came from the
  initial compilation. Existing built frontend assets were then copied from the
  live checkout into this isolated checkout; these tests do not exercise frontend
  rendering. Full browser/link verification remains pending.

## Board and execution read increment

The remote tool subset now includes `get_project`, `list_issues`, `get_issue`,
`list_issue_workspaces`, `get_workspace`, `list_workspace_runs`, and
`get_execution`. Project/Issue/Workspace membership is validated at the service
boundary. Requirement and execution text pages require the previous revision
when continuing; direct UI changes invalidate stale pages. List cursors identify
real records within the requested parent and reject mixed targets.

Runtime responses use canonical lifecycle/projection/Goal fields and selected
text sections. They do not serialize native session references, host paths or
request envelopes. Agent text is explicitly not independent verification proof.
Integration/Wiki and verification readers are still required before M1 is complete.

The new tests use the complete current SQLite migration chain in memory, with
105 Issues and two Workspaces for one Issue, Unicode requirement pages, explicit
external UI changes, scoped runtime records and sensitive fields omitted from
responses. The first Board/runtime read run passed 6 tests (200 unrelated server tests
filtered); build 1m 11s, execution 0.76s. Log:
`/tmp/lvk-chatgpt-mcp-read-tests.log`. Question-reader and shared-service
regression checks are a separate subsequent increment.

For the upcoming execution adapter, the real Goal entry is
`ExecutorConfig.execution_mode = ExecutionMode::Goal` (or `PlanWithGoal`), not a
`/goal` prefix. In-flight updates use the existing `AgentRunPortCommand::GoalUpdate`
via `routes/agent_runs.rs`. Preserve stored executor/model/permission settings.

## Question projection reuse

`list_questions` and `get_question` validate Project, Issue, Workspace, Session,
run and question identity. They reuse `sessions::questions::pending` for Codex
nonblocking questions, including direct-answer reconciliation and attempt expiry.
The existing canonical pending-question query was moved unchanged from Astra's
private tool helper into `sessions::questions::pending_blocking`; Astra retains a
wrapper calling that helper. No model code, prompt, setting or monitoring logic
was changed. This small shared extraction keeps blocking-question/approval rules
consistent without routing MCP through the Astra model service.

Question lists are previews with identifiers. Full titles/options and canonical
question data are available as revision-checked JSON pages. Execution approvals
are explicitly distinguished and require the existing LVK UI. Answers are not
yet exposed by this read increment.

## Validation record (Board/runtime/questions read increment)

Final candidate after limiting approval disclosure:

- `cargo test -p server --lib remote_mcp:: -- --nocapture`: 7 passed,
  200 unrelated tests filtered, build 27.17s, tests 0.79s.
  Log: `/tmp/lvk-chatgpt-mcp-read-final.log`.
- `cargo test -p server --lib routes::project_chat::tests::`: 20 passed,
  tests 12.57s. Log: `/tmp/lvk-chatgpt-mcp-astra-regression.log`.
- `cargo test -p server --lib routes::sessions::questions::tests::`: 2 passed,
  tests 0.04s. Log: `/tmp/lvk-chatgpt-mcp-question-regression.log`.
- Rust formatting and whitespace checks passed. No full backend suite ran.

This verifies the read increment, not M0–M3 completion. Still required: scoped
activity/audit, repository/memory/diff/Wiki and Integration/result readers; write
adapters and their execution/retry tests; the actual Plugin/Skill and operational
configuration; remaining T01–T18 evidence and the optional UI decision.


## Activity, Integration evidence and Issue operations increment

`list_run_commands` and `get_run_command` read commands sent through other LVK
entry points, including delivery changes. Only selected instruction text is
exposed; raw envelopes and execution approval commands stay private. Lists and
text pages are bounded. `list_integrations` and `get_integration` keep validation,
exceptions, publication and Wiki evidence separate. The existing Wiki observer
is shared through a pool-based helper without changing the builtin caller.
Stored Wiki status is not presented as currently verified success when current
repository receipts cannot be observed.

Local validation for this increment:

- `cargo test -p server --lib remote_mcp:: -- --nocapture`: 10 passed;
  compilation 41.12s, tests 2.36s. Log `/tmp/lvk-chatgpt-mcp-write-final.log`.
  Includes 105-command and 105-Integration fixtures, live direct-command status,
  stale Integration pages, Issue scope/revision/retry checks, no model/Workspace
  creation from Issue writes, and disconnect during admission before DB access.
- `cargo test -p server --lib routes::local_remote::tests:: -- --nocapture`:
  6 passed in 0.02s (build 0.86s); `/tmp/lvk-chatgpt-mcp-board-regression.log`.
- `cargo test -p server --lib routes::integrations:: -- --nocapture`:
  14 passed in 12.82s (build 1.43s), covering existing candidate validation,
  publication races, exception limits and semantic outbox behaviour;
  `/tmp/lvk-chatgpt-mcp-integration-regression.log`.
- `pnpm run format` and `git diff --check`: passed. No frontend source changes.
- An initial test build used an unavailable `futures` test macro. It failed to
  compile; the test now uses the standard library's future poll helper. This was
  a test-only compilation failure, not a runtime or domain failure.

Remaining required work includes repository/memory/diff/Wiki detail readers,
Project summary completeness, execution/continue/steering/question/Integration
write adapters, the actual Plugin/Skill package and installation guide, and the
full T01–T18 audit. ChatGPT real-account validation has not been performed.
