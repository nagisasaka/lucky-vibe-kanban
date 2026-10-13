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

The admission helper currently has isolated concurrency/restart tests but is not
yet wired to public write tools.

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
