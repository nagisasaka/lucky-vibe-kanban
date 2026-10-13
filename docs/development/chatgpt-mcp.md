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
New Workspace and formal Integration adapters are described below. The full
acceptance audit and native execution evidence are still required before M2 is complete.

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

This records the read checkpoint, not M0–M3 completion. Later increments below
add activity, Integration evidence and write adapters. Remaining work includes
repository/memory/diff/Wiki details, Project summary completeness, full native
execution acceptance, Plugin/Skill and operational configuration, the remaining
T01–T18 audit and the optional UI decision.


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
Project summary completeness, full native execution acceptance evidence,
the actual Plugin/Skill package and installation guide, and the
full T01–T18 audit. ChatGPT real-account validation has not been performed.


## Steering, continuation and answers increment

`steer_execution` selects the current active run by explicit Project, Workspace,
Session and run IDs. `continue_execution` selects the latest terminal run and
reuses the existing Session executor configuration and follow-up admission.
It does not replace the saved model, permissions or native Goal mode. A new run
created directly in LVK invalidates the previous continuation selection.

`answer_question` requires the exact kind, ID, run and revision returned by
`get_question`. Async answers use the existing durable `agent_question_answers`
reservation and Session lock. The shared function now accepts an injectable
final dispatcher for tests; production calls the same AgentRun control service
as before. An identical direct UI answer that wins a race is reused and its
actual command ID is reported. Different answers conflict.

Blocking input delivery now rechecks canonical pending state under the Session
lock for both UI and MCP. Remote answers also pin the originating question event
so a repeated provider input ID in a later attempt cannot receive an old answer.
Question revisions include event/attempt identity, and pending blocking reads
expire old attempts even if the state projection has not caught up. Execution
approvals remain available only through the LVK link.

All three tools require `lvk:read` and `lvk:write` and use the existing durable MCP
receipt admission. Domain errors after dispatch are uncertain because an error
can follow an accepted command; they are never automatic retry permission.
Receipts describe instruction/answer acceptance separately from implementation
completion. The generic receipt no longer asserts that implementation has not
started; that assertion belongs specifically to Issue-only results.

Runtime adapter tests use the real migrated database and parent/lifecycle
checks. Steering and continuation replace the runtime-action boundary in
fixtures. Question tests use the real Session queue and answer reservation
service, replacing only final delivery. They do not claim a real Codex launch,
saved-profile provider execution or ChatGPT session was exercised. New-workspace and formal Integration admission adapters are described below;
full native execution acceptance evidence remains pending.


Validation for this increment (scoped suites only):

| Command filter (`cargo test -p server --lib … -- --nocapture`) | Passed | Test time | Build time | Log under `/tmp/` |
| --- | ---: | ---: | ---: | --- |
| `remote_mcp::` | 13 | 4.76s | 39.54s | `lvk-chatgpt-mcp-runtime-final.log` |
| `routes::sessions::` | 40 | 0.89s | 0.97s | `lvk-chatgpt-mcp-runtime-sessions.log` |
| `routes::agent_runs::` | 3 | 0.07s | 1.40s | `lvk-chatgpt-mcp-runtime-controls.log` |
| `routes::project_chat::tests::` | 20 | 14.45s | 1.31s | `lvk-chatgpt-mcp-runtime-astra.log` |
| `routes::integrations::` | 14 | 16.06s | 2.37s | `lvk-chatgpt-mcp-runtime-integration.log` |

`pnpm run format` and `git diff --check` passed. An initial new question fixture
violated the existing state projection constraint (positive attempt number with
zero event sequence). The fixture was corrected to represent a valid projection;
the final suites above all passed. No migration constraint was weakened.


## New Workspace and formal Integration admission

`start_workspace` reads the exact Issue revision and agreed description, checks
explicit repository memberships and branches, then calls the existing
Workspace creation/start pipeline. The normal creation UI uses the global
`executor_profile` as its default; this adapter uses the same identity, with no
hardcoded model or authentication override. Optional `execution_mode` and a
positive Goal budget use the native executor fields, never a `/goal` string.

The existing Worktree creation service accepts an internal, stable Workspace ID
from the MCP receipt. The external UI request schema and DirectFolder flow stay
unchanged. If admission fails partway through, the uncertain receipt links that
Workspace ID so it can be inspected without starting another attempt.

`start_integration` requires `lvk:merge` in addition to read access. It validates
explicit Issue/Workspace/repository associations and passes one frozen selection
per Issue to existing formal admission (up to its existing 100-Issue limit).
It also pins the selected target commit. Formal admission observes and checks
that target under its existing mutation lock; its later validation/publication
checks remain intact. Existing Integration executor defaults are preserved.
No direct Git merge, target write, Done update or Wiki invocation was added to
the MCP adapter.

These two tools wait for domain **admission**, which returns Workspace/Session/
run IDs or an Integration ID; they do not wait for coding or publication to
finish. Admission is owned by a task started before the first DB await. Losing
the waiting MCP request cannot cancel it. A concurrent retry may return the
still-running operation receipt; inspect `get_operation` using the same key.
Do not interpret a running receipt without a result as accepted coding.

`list_project_repositories`, `list_repository_branches`,
`get_integration_candidate` and `get_execution_defaults` provide the IDs, local
branches, commit evidence and default executor identity needed to construct
these requests. Creation repository discovery shares the existing configured
Project defaults reader with Astra. No read invokes Astra or mutates defaults.
Candidate commit evidence explicitly says eligibility is not yet validated;
formal admission still checks every related Workspace, settled writers,
cleanliness, semantic finalisation and head identity.

Adapter fixtures use migrated temporary databases and actual temporary Git
repositories (including 105 local branches for pagination). They replace the
Workspace/Integration launch boundary; their success is not evidence of an
actual Codex provider launch or a full remote formal Integration. Existing
formal Integration regression tests run separately against the shared rules.
The production adapters compile against the real services, while complete
native execution acceptance and ChatGPT real-account verification remain pending.


Validation for the admission increment:

- `cargo test -p server --lib remote_mcp:: -- --nocapture`: 16 passed in
  7.00s (build 36.18s); `/tmp/lvk-chatgpt-mcp-launch-final.log`.
- Final adapter subset `remote_mcp::launch_tests::`: 3 passed in 2.66s
  (build 0.85s); `/tmp/lvk-chatgpt-mcp-launch-adapters-final.log`.
- `routes::workspaces::create::`: 6 passed in 0.00s (build 1m06s);
  `/tmp/lvk-chatgpt-mcp-launch-workspaces.log`.
- Final `routes::integrations::`: 15 passed in 11.38s (build 38.12s),
  including a real temporary target-branch update before formal admission;
  `/tmp/lvk-chatgpt-mcp-launch-integrations-final.log`.
- `routes::project_chat::tests::`: 20 passed in 13.01s (build 0.99s);
  `/tmp/lvk-chatgpt-mcp-launch-astra.log`.
- `pnpm run format`, subsequent Rust formatting and `git diff --check` passed.

An initial compilation found that the accepted Session ID lives in
`AgentRunPortSnapshot.state.session_id`; the adapter was corrected before the
successful suites above. No production configuration or running server changed.

The complete acceptance audit must still prove native execution/profile binding,
finish repository/memory/diff/Wiki detail readers and Project summaries, and
supply the real Plugin/Skill package and tested installation/configuration guide.
Domain admission failures currently remain conservative uncertain receipts;
refining rejected-versus-accepted reconciliation and meaningful source/target
conflict codes is part of the remaining acceptance audit.

## Workspace memory and Wiki evidence

`list_workspace_wiki`, `get_workspace_wiki_page` and `get_workspace_memory`
require explicit Project, Workspace and repository IDs. Both the local Issue
membership and the Workspace–repository association are checked. The response
contains the existing Workspace link, including the configured public subpath.
These tools read reference data only; they never initialise memory, maintain
OpenWiki, create a conversation or request model inference.

Wiki reads reuse the shared OpenWiki Viewer loader, including its private-file,
symlink, UTF-8 and size checks. Lists contain paths and bounded titles, with a
20-item default and a 50-item maximum. Selected page bodies and Workspace memory
use character offsets (8,000 default, 16,000 maximum). Later pages require a
revision bound to the Project, Workspace, repository and content. A changed or
foreign revision requires restarting the read. Wiki list revisions also detect
body edits during traversal. The source is explicitly the selected checkout;
these reads do not claim that Wiki publication has succeeded.

The existing memory-store constructor can create missing directories, so the
MCP adapter uses a separate read-only constructor. It opens existing records
without requiring or creating repository state. Missing memory is returned as
unavailable, whereas unsafe, invalid, unreadable or oversized records fail
explicitly. The existing 128 KiB memory-record limit and Viewer limits (512 KiB
per page, 2,000 pages, 16 MiB total) remain enforced. Error messages omit host
paths. No private memory from another Workspace is included.

Code, diff and semantic-manifest readers remain required, as do the outstanding
Project summaries, native execution/admission acceptance evidence and M3 package.

Validation for this increment:

- `cargo test -p server --lib remote_mcp::evidence_read:: -- --nocapture`:
  3 passed in 4.79s (initial dependency rebuild 3m05s),
  `/tmp/lvk-chatgpt-mcp-evidence.log`.
- Final `cargo test -p server --lib remote_mcp:: -- --nocapture`:
  19 passed in 10.23s (build 34.30s),
  `/tmp/lvk-chatgpt-mcp-evidence-final.log`. This includes transport/authentication,
  receipts, Issue operations, execution/question adapters and Integration reads.
- `pnpm run format` and `git diff --check` passed;
  `/tmp/lvk-chatgpt-mcp-evidence-format.log`.

The new fixtures cover 105 Wiki pages, multibyte text, bounded continuation,
changed and foreign revisions, missing/deleted targets, unknown parameters,
Project/Workspace/repository membership, unsafe links, oversized memory and
absence of provisioning/inference. They use temporary directories and an
isolated migrated SQLite database. No real ChatGPT account, live repository
memory, native provider execution or deployment is exercised by these fixtures.

## Code and per-path diff evidence

`list_workspace_files`, `get_workspace_file`, `list_workspace_diff` and
`get_workspace_diff` reuse the same Project/Workspace/repository membership,
revision pagination and Workspace links as the Wiki readers. Code paths come
from Git's index. Diff paths include untracked changes against the existing
Workspace target merge base. Lists default to 20 and allow at most 50 paths;
text pages default to 8,000 and allow at most 16,000 characters. A checkout on a
different branch is rejected rather than attributed to the stored Workspace.

The normal Git CLI diff reader populates a temporary index with `git add`, which
can run clean filters and write Git objects. The MCP read path instead uses
libgit2 through GitService to enumerate changes without staging. It reuses
GitService's merge-base and head identities, and the shared unified-diff
formatter. It does not run external diff, textconv, clean filters or hooks.
There is no alternate merge or publication path.

The reference reader rejects private dot paths, credential/secret directories,
key-file extensions and non-relative paths. It limits each source side to
512 KiB. Binary, oversized, unsupported and missing content remain explicit;
failed reads are not empty successful diffs. Renames are represented per path
(source deletion and destination addition). Text equality does not prove the
absence of permission or other non-text changes. Source head and base commit
are returned separately from publication status.

On Unix servers, file reads descend from an open directory with `openat`,
`O_NOFOLLOW` and `O_NONBLOCK`. This rejects symlinks in every relative component
and avoids blocking on a FIFO. The existing `nix` dependency gains its `fs`
feature; no runtime or package is added. Non-Unix hosts return an explicit
unsupported read error rather than use an unsafe fallback. This implementation
is intended for LVK's Linux server image; Windows file reads are not advertised.

Validation for code/diff reads:

- Initial `cargo test -p server --lib remote_mcp::code_read:: -- --nocapture`:
  2 passed in 1.74s (dependency rebuild 4m31s),
  `/tmp/lvk-chatgpt-mcp-code-tests.log`.
- Final `cargo test -p server --lib remote_mcp:: -- --nocapture`:
  21 passed in 10.39s (build 1m18s), `/tmp/lvk-chatgpt-mcp-code-final.log`.
- `cargo test -p utils --lib reference_file:: -- --nocapture`:
  1 passed in 0.00s (build 1m06s), `/tmp/lvk-chatgpt-mcp-code-files.log`.
- `pnpm run format`, subsequent Rust formatting and `git diff --check` passed;
  `/tmp/lvk-chatgpt-mcp-code-format.log`.

Fixtures use real temporary Git repositories and an isolated migrated database.
They verify 105 changed files, bounded traversal, current direct edits, stale
body revisions, untracked additions, deletion, binary/oversized content,
private/foreign paths, leaf and ancestor symlinks, changed checkout branches,
an unchanged index and no invocation of configured clean/external-diff commands.
The low-level test also checks FIFO rejection and bounded reads. No live
Workspace, credential or deployment is modified. Full T01–T18 acceptance and
ChatGPT connection remain unverified; the package/Skill and connection guide,
Project summaries, semantic-manifest evidence and admission audits remain open.

## Skill and Plugin distribution

The portable package now lives under `plugins/lvk`, with one `lvk-work` Skill,
an Issue template, result-check guidance and an MCP dependency. The Skill keeps
Issue creation, execution admission, command delivery, completed agent turns,
formal publication and Wiki evidence distinct. It uses current IDs/revisions,
existing execution settings, scoped reads and durable request IDs. It promises
neither automatic notification nor personal-memory synchronisation.

The [connection and operation guide](chatgpt-mcp-setup.md) covers the opt-in LVK
configuration, external OAuth provider requirements, HTTPS/subpath routing,
ChatGPT Web's custom MCP and Plugin Creator steps, controlled workspace import,
metadata/Skill refresh, revocation, and an account verification checklist.
`scripts/chatgpt-mcp/configure.py` builds a new distribution with a consistent
public URL and a relative marketplace; it never installs or publishes it.

On 2026-10-13 the linked Agent Plugins 1.0 MCP schema rejected the per-server
`extensions.com.openai.auth` field shown in the OpenAI packaging example.
The package therefore uses the valid portable `type`/`url` declaration and the
host's explicit OAuth setup, backed by LVK discovery and mandatory token checks.
There is no anonymous fallback and no embedded credential. The provider recipe,
ChatGPT login/install, callback, subpath browser return and real-account workflow
have not been executed here; they remain distinct from package validation.

The package validator loads the actual Rust-advertised tool catalog, validates
34 example inputs and their scope requirements, and resolves every bundled Skill
reference. It also checks the official manifest schemas and the Skill's tool
names. The examples are schema fixtures, not simulated proof that ChatGPT follows
the Skill. Representative account-level behavioural checks remain in the guide.

Package validation evidence:

- Final `LVK_MCP_TOOL_CATALOG=… cargo test -p server --lib
  remote_mcp::tests::plugin_tool_catalog -- --nocapture`: 1 passed in 0.01s
  (build 35.88s), `/tmp/lvk-chatgpt-mcp-package-catalog-final.log`. The same test
  deserialises and validates the operator configuration example.
- `scripts/chatgpt-mcp/validate.py`: source and configured packages pass both
  published schemas; 34 real tool input schemas/scopes and all Skill references
  pass, `/tmp/lvk-chatgpt-mcp-package-validation.log`.
- Python `test_package.py`: 2 passed in 0.009s, including unsafe URL rejection,
  subpath consistency and refusing to overwrite an existing distribution;
  `/tmp/lvk-chatgpt-mcp-package-config-tests.log`.
- Skill Creator `quick_validate.py`: passed,
  `/tmp/lvk-chatgpt-mcp-skill-validation.log`.
- `pnpm run format` and `git diff --check`: passed,
  `/tmp/lvk-chatgpt-mcp-package-format.log`.

An initial manifest check rejected the OAuth extension described above; it was
removed rather than weakening schema validation. The `list_questions` example
was corrected to include its required Session ID. No account permission, live
OAuth grant, global plugin installation or server deployment changed.

Remaining acceptance work includes complete Project summaries, semantic
verification evidence, native execution/formal-admission
boundary checks, rejected-versus-uncertain reconciliation, actual public ingress
and browser/subpath behaviour, and the final T01–T18/M4 audit. Package validation
alone does not complete those requirements.

## Current execution identity and validation completeness

`list_workspace_runs` now resolves equal creation timestamps by SQLite admission
order, matching the existing steering/continuation validator. Its cursor uses
the same order. Both run lists and `get_execution` expose `is_current_run` per
Session, so a Workspace with multiple Sessions can have multiple current runs.
This is selection evidence, not permission to steer: the existing write boundary
still rechecks identity, lifecycle and Workspace membership immediately before
admission.

Execution pages bind their revision to the run and selected section as well as
the stored update time and event sequence. A revision from another run or section
is rejected even when their timestamps match. Detail responses include the
native Goal token budget, tokens used and elapsed seconds; absent projection
fields remain null rather than becoming fabricated zeroes.

Integration summaries count skipped checks explicitly and include unrecognised
result strings in `unknown_or_unrun`. Every check belongs to one result category;
exceptions remain a separate dimension. Truncated source `done_result` text now
has an explicit flag. Publication and current Wiki evidence remain separate.

Validation at this checkpoint: `CARGO_TARGET_DIR=/var/tmp/lvk-target cargo test
-p server --lib remote_mcp:: -- --nocapture` passes 23 tests in 11.34s after a
1m 04s build (`/tmp/lvk-mcp-current-runs.log`). The new test traverses 107 runs
across two Sessions with identical timestamps and reversed UUID order, compares
current-run selection with the actual write validator, rejects foreign section
and run revisions, and rejects a deleted cursor. Existing projection and
Integration tests cover Goal usage and complete result-category counts. These
are isolated SQLite fixtures; they do not prove live provider execution or
ChatGPT account behaviour. No runtime server binary was built or deployed.

`pnpm run format` and `git diff --check` pass; formatting output is retained in
`/tmp/lvk-mcp-current-runs-format.log`. Temporary dependency links were removed.

## Project activity navigation

`get_project` now returns Workspace/Session totals, counts of current Session run
states, non-terminal Session counts and canonical unresolved specification
question/approval counts. It includes up to 20 active Session summaries with
Issue, Workspace, Session and current run IDs. Follow the returned cursor with
`list_project_sessions(active_only=true)` for the rest. The same tool can list
all Sessions, including completed work. Archived Workspaces remain visible and
are labelled; their existence is not permission to resume them.

Counts use existing local Board membership and the existing question service,
including direct-answer reconciliation and attempt expiry. This performs no
model inference or new execution. Reads can reconcile an existing answer receipt
when the canonical service already has delivery evidence; they do not answer or
remove a pending question merely because it was viewed. Live aggregation is not
an atomic snapshot, so concurrent changes require a refresh.

`list_project_statuses` exposes all status IDs and Issue counts in pages of 20
(default), at most 50. The initial status summary retains its 50-item bound and
now provides a continuation cursor. Both new lists reject foreign or deleted
cursors. The Skill and executable schema examples include both real tool names.

This closes Project activity and question navigation gaps. Current-head merge
classification and structured semantic verification evidence remain separate
unfinished acceptance work; Issue status or a terminal run is not treated as
proof of publication.

Validation for Project navigation:

- `CARGO_TARGET_DIR=/var/tmp/lvk-target LVK_MCP_TOOL_CATALOG=… cargo test
  -p server --lib remote_mcp:: -- --nocapture`: 24 passed in 12.52s, build
  1m 07s (`/tmp/lvk-mcp-project-read.log`).
- After adding the moved-Issue/expired-active-cursor case, the focused
  `remote_mcp::project_read::tests` filter passes both tests in 2.33s, build
  31.64s (`/tmp/lvk-mcp-project-read-final.log`). Fixtures contain 105 Sessions
  and 105 statuses; they verify bounds, continuation, exact question/approval
  counts, direct-answer observation, expiry and no additional model/job records.
- Package/schema validation passes for 36 actual tool inputs and bundled Skill
  references (`/tmp/lvk-mcp-project-package.log`).
- `pnpm run format` and `git diff --check` pass
  (`/tmp/lvk-mcp-project-format.log`). Temporary dependency links were removed.

These tests use isolated databases and the existing provider-delivery fixture.
They do not establish a live ChatGPT connection or native provider execution.
No deployment, main-branch merge or user authentication change was performed.

## Recorded Workspace verification

`get_workspace_verification` reads the existing immutable Change Manifest whose
identity is the selected AgentRun. Project/Workspace/repository membership and
run ownership are checked before storage access. The manifest must match all
three stored identities. This exposes no arbitrary file path and does not scan
other Workspaces' memory or create missing storage.

The response separates recorded passed/failed/not-run counts from current Git
state. It returns the manifest's source/base commits, record time, current head
when observable and whether that head matches the recorded source. These are
agent-recorded test claims, not independent validation. Even an equal head does
not establish verification of uncommitted changes. Missing records are explicitly
unavailable; an unreadable, oversized or invalid record is an error. Publication
and Wiki status remain unobserved here; use formal Integration evidence for them.

Test previews use 20 items by default, at most 50, with explicit truncation and
continuation. `get_workspace_verification_test` provides complete command/result/
summary JSON in character pages up to 16,000. Revisions bind target, run, selected
test and observed head; a changed head requires refreshing the evidence. Nothing
executes the recorded command or starts a model. Both tools are included in the
Skill reference and schema examples.

Verification-read validation:

- Initial MCP run: 26 passed, 1 failed in 13.98s after a 2m 23s build
  (`/tmp/lvk-mcp-verification.log`). The nested flattened target DTO rejected
  valid input before the ownership check. Flattening only the scalar run identity
  fixes the input boundary while retaining rejection of unknown fields.
- Corrected focused tests: 2 passed in 0.57s, build 2m 17s
  (`/tmp/lvk-mcp-verification-focused.log`).
- Final `CARGO_TARGET_DIR=/var/tmp/lvk-target LVK_MCP_TOOL_CATALOG=… cargo test
  -p server --lib remote_mcp:: -- --nocapture`: 27 passed in 14.49s, build 33.51s
  (`/tmp/lvk-mcp-verification-final.log`). The added coverage includes 105
  recorded tests, Unicode preview/detail bounds, mismatched identities, corrupt
  records, stale revisions, real temporary Git head/branch changes and missing
  storage. Reads create no model, command or operation records in the fixtures.
- Official package/schema and Skill reference validation: 38 real tool inputs
  passed (`/tmp/lvk-mcp-verification-package.log`).
- `pnpm run format` and `git diff --check` passed
  (`/tmp/lvk-mcp-verification-format.log`); temporary dependency links removed.

No runtime server binary was built or deployed. Real ChatGPT account behaviour,
provider execution and public ingress remain outside these fixture results.

## Current merge observations

`get_workspace_merge_status` observes a selected Workspace/repository pair.
`list_project_merge_status` provides the same bounded summaries across a Project,
using stable WorkspaceRepo IDs as cursors. `get_project` includes counts by
Workspace/repository pair, up to 20 summaries and a continuation cursor. Workspaces
with no repository association are counted separately, not presented as clean.

The read uses existing GitService identity/ancestry operations and a read-only
libgit2 status projection. It does not fetch, refresh the index, execute configured
filesystem monitors/filters, or run merge commands. Counts include tracked and
untracked changes, including private files without returning their names/bodies.
The existing default `.git`, `node_modules` and `.lvk-shared` exclusions apply.
Sparse checkouts return unknown because this reader does not claim CLI-equivalent
sparse status semantics. Missing checkouts, branch changes and conflicts also
remain explicit unknown states.

An `included` result requires no observed worktree changes or Git operation plus
either source ancestry in the observed target or a published formal Integration
record pinned to this exact Project, Issue, Workspace, repository, source commit
and target ref. For a squash publication, its result commit must still occur in
the observed target history. A previous publication of a different source head
does not establish publication of new commits. Legacy direct/PR history without
a source identity leaves ambiguous ancestry cases unknown. Only when all legacy records fit the inspection bound and each published
commit is an ancestor of both observed heads, with the source since advanced,
can they establish post-publication work outside the target history. A later
unreadable or unrelated record keeps that case unknown. Inspection is capped
at 50 matching publication records; incomplete or unreadable evidence cannot
establish a clean result.

`included` is local target-history evidence, not proof of test success, unchanged
semantics after later target commits, remote server publication or Wiki receipt.
Remote-tracking refs are explicitly local observations without fetching. `unmerged`
means observed worktree changes or a source outside that history without matching
publication evidence. Refresh before selecting candidates; the existing formal
Integration admission still performs its own source/target validation and locks.

Merge-read validation:

- Scoped MCP suite: 29 passed in 16.21s, build 3m 39s
  (`/tmp/lvk-mcp-merge-read.log`).
- Focused merge tests after adding legacy post-publication detection: 2 passed in
  2.32s, build 1m 10s (`/tmp/lvk-mcp-merge-final.log`). The final ambiguity check
  also passes both tests in 2.28s, build 31.63s
  (`/tmp/lvk-mcp-merge-final-recheck.log`).
- Fixtures cover real Git ancestry and squash publication, new commits after
  publication, tracked/private untracked changes, ambiguous old/later records,
  sparse or switched checkouts, unchanged index bytes and no configured
  filesystem-monitor execution. A 105-pair Project verifies exact unknown counts,
  bounded traversal, stale cursors and Issue membership changes. Reads create no
  Astra/model, command or remote-operation records.
- Package validation passes 40 actual tool input examples and Skill references
  (`/tmp/lvk-mcp-merge-package.log`). `pnpm run format`, final Rust formatting and
  `git diff --check` pass (`/tmp/lvk-mcp-merge-format.log`).

These results establish the read projection over isolated fixtures. Native
execution/formal-admission boundary tests, admission uncertainty/error review,
public ingress/browser links, stdio compatibility and the full T01–T18/M4 audit
remain. No server deployment, target merge or authentication change was made.

## Formal admission reconciliation and public diagnostics

The remote formal Integration adapter now reconciles an admission error against
its existing durable request key after canonical admission has returned. It uses
the same mutation lock and exact Project/repository/target/source/executor
comparison as canonical idempotency. An existing matching record returns its ID
and actual status, including `blocked`; this proves admission, not publication.
Confirmed absence returns `failed` with `integration_accepted: false`. Changed
source/target selections have explicit codes. A failed lookup or mismatching
record remains `uncertain`, requiring inspection before any new request. The
adapter does not repeat admission or launch another job during reconciliation.

Public error conversion preserves static validation messages and replaces
formatted domain, IO and SQL diagnostics with a safe instruction to refresh and
inspect. Private paths and diagnostic context do not become MCP error text.
Operation receipts also expose the already stored safe target IDs, without the
internal principal key or input fingerprint.

Validation for this change:

- `cargo test -p server --lib remote_mcp:: -- --nocapture`: 31 passed in
  16.06s, build 38.62s (`/tmp/lvk-mcp-admission-suite.log`).
- `cargo test -p server --lib routes::integrations:: -- --nocapture`: 16 passed
  in 11.89s (`/tmp/lvk-mcp-admission-domain.log`). This includes existing formal
  Integration checks and the new durable reconciliation test.
- `pnpm run format` and `git diff --check` passed
  (`/tmp/lvk-mcp-admission-format.log`). Temporary dependency links were removed.

Tests use isolated databases and Git fixtures. Adapter launch fixtures still
replace provider execution; reconciliation tests do not establish a complete
MCP-to-native-execution workflow. Public ingress/browser checks, stdio regression,
the final T01–T18 audit and the conditional UI decision remain outstanding.
No runtime binary was built or deployed and no user authentication was changed.

## HTTPS ingress and conditional UI decision

The distributed server and source-development launchers now read
`LVK_MCP_CONFIG` to generate two exact nginx exceptions for the configured public
MCP prefix. The application, API, preview and separate staging listener retain
Basic authentication. Only the MCP endpoint forwards Bearer credentials; both
exceptions remove cookies, proxy credentials and relay signatures. Source
development sends MCP directly to port 4021, while the distributed server uses
its existing port 3000. These are loopback upstreams, not new public listeners.
A separate optional Compose override configures only the `lvk` service.

Native nginx tests use temporary certificates, ephemeral loopback ports and an
HTTP backend fixture. They verify TLS, the Basic/Bearer boundary, exact routing,
root/subpath forwarding, removed headers, POST forwarding and absence of fixture
credentials from access logs. They do not perform real OAuth login or claim
frontend subpath support. The existing frontend router does not yet consume a
public base path; browser link verification remains required.

The conditional M4 card is deferred. Inspection found no existing MCP Apps UI
resource or SDK dependency in the package manifests/lockfile. The actual
`KanbanContainer` depends on app Project/Workspace/auth/action contexts;
`KanbanCardContent` requires translation, theme and badge components, while
`IssueTaskAttemptCard` carries run/unlink/delete actions. Reusing these would
still require a separate iframe entrypoint and bundle, a read-only projection,
theme/translation setup, an MCP Apps bridge, resource registration/metadata and
CSP plus new browser tests for refresh, replay, fallback and XSS. Estimated change
surfaces are a new UI package/build target, Rust resource handling, image asset
packaging and a dedicated UI test suite; this is more than adapting an existing
standalone status card. No dependency or full-app iframe is added for M4.

The official [MCP UI guide](https://developers.openai.com/plugins/build/chatgpt-ui)
was rechecked on 2026-10-13: it describes a separately bundled iframe resource,
`text/html;profile=mcp-app`, `_meta.ui.resourceUri`, the standard MCP Apps bridge
and explicit CSP domains. A later UI can use `@modelcontextprotocol/ext-apps`
with existing bounded read tools. The current path remains structured results,
text and LVK links. No UI or automatic notification is advertised as implemented;
T19 is not applicable while the conditional UI is deferred.

Ingress validation:

- `LVK_TEST_NGINX=/usr/sbin/nginx LVK_REQUIRE_NGINX_TEST=1 node --test
  deploy/server/mcp.test.mjs deploy/server/nginx.test.mjs`: 6 passed, 9.66s
  (`/tmp/lvk-mcp-ingress.log`). nginx was present; no skips.
- `node --test deploy/server/server.test.mjs`: 13 passed, 1.95s
  (`/tmp/lvk-mcp-ingress-server.log`), including the opt-in Compose document.
- `node --test deploy/server/development.test.mjs deploy/server/instance.test.mjs`:
  4 passed, 0.78s (`/tmp/lvk-mcp-ingress-regression.log`).
- Formatting the deployment files initially failed because the root package does
  not install Prettier. Using the existing local-web formatter succeeded, followed
  by `pnpm run format` (`/tmp/lvk-mcp-ingress-format-final.log`). Temporary
  dependency links were removed. `git diff --check` passed.

No shared server, database, authentication file or active job was modified.
The Compose override was parsed in tests; a Docker rollout was not performed.


## Public-prefix link routing

The local router now accepts a trusted `lvk-basepath` meta tag injected by the
supplied HTTPS launcher into prefixed app documents. TanStack Router handles
incoming and outgoing paths; application destinations continue to use the same
Project/Issue/Workspace route definitions. Host selection reads the router's
internal pathname, so a public prefix is not mistaken for a relay-host segment.
The ordinary unprefixed app is unchanged. APIs and static assets remain at their
existing authenticated root paths on the same origin. This is an alias for one
LVK instance, not support for isolating multiple applications by prefix.

The proxy requests uncompressed HTML for injection, preserves the existing Basic
boundary and blocks prefixed preview paths. Reserved API/asset/source/MCP prefixes
are rejected before generating configuration. No HTML or base path is obtained
from tool input or Issue content.

Validation:

- Native nginx/config tests: 6 passed in 10.18s
  (`/tmp/lvk-mcp-subpath-ingress.log`), including HTML injection after Basic auth,
  preserved frame protection and preview denial.
- `pnpm --dir packages/local-web run check` passed
  (`/tmp/lvk-mcp-subpath-types-final.log`). The first invocation lacked temporary
  dependency links and failed before type checking; it was rerun with dependencies.
- `node --test tests/server/mcp-links.test.mjs`: 1 browser test passed in 113.31s
  (`/tmp/lvk-mcp-links-browser-final.log`). Chromium loads the real local router,
  generated route tree and navigation resolver over a temporary HTTPS Basic-auth
  fixture. Root and prefixed Workspace links resolve exact target IDs; generated
  Issue links retain the prefix, and reload retains the destination. Each fresh
  browser context authenticates, and route checks make no backend API calls.
- The first browser run failed on the fixture's missing Vite HMR WebSocket route
  (`/tmp/lvk-mcp-links-browser.log`). Attaching HMR to the fixture HTTPS server
  fixed the failure; the final run reports no browser exceptions.

The browser fixture deliberately does not mount application data providers or
access a live LVK database. It proves route/auth-return behaviour, not rendered
business data, account permissions or a real ChatGPT workflow. Native execution
and final acceptance/regression checks remain outstanding. No runtime deployment
or authentication change was made.

Changed-file ESLint, `pnpm run format` and `git diff --check` also passed
(`/tmp/lvk-mcp-subpath-lint.log`, `/tmp/lvk-mcp-subpath-format.log`). Temporary
dependency links were removed and all fixture servers/browsers were closed.
