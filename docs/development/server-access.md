---
title: "LVK server authentication implementation and verification"
description: "Auth0 Web login, OAuth MCP, locked setup, route boundaries and evidence."
---

## Investigation (2026-10-15)

Work is isolated in `feature/auth0-access`, based on `b9d78ceb`. No main merge,
push, deployment, Auth0 account operation or certificate change is part of this task.

- HTTP: Axum 0.8, `crates/server/src/routes/mod.rs`; frontend and API share one origin.
  `/api` contains project/workspace/session/file/agent/runner/relay APIs. WebSocket
  upgrades mostly share `SignedWsUpgrade`; events/workflows also use SSE.
- Existing remote MCP: rmcp stateless Streamable HTTP `/mcp`, 40 tools, external
  introspection, one subject and project allowlist. Tool implementation and durable
  receipts remain; production transport now uses common Auth0 JWT/member checks.
- Existing local/cloud-compatible identity was UUID 1. New member IDs are separate;
  historical synthetic workspace ownership remains UUID 1, without attribution to
  a new person. New authenticated operations use the current principal.
- SQLite/SQLx migrations live in `crates/db/migrations`. New migration
  `20261015000000_server_access.sql` adds settings, members, sessions, one-use keys,
  browser transactions, confirmations and audit. Existing business rows are untouched.
- Existing nginx in `deploy/server` terminates HTTPS. Host ACME tooling retains
  Let's Encrypt IP certificate acquisition/renewal and the existing TLS volume.
  The launcher continues certificate validation/reload without restarting agents.
- Authentication code is isolated in `crates/server-access`, without a provider
  plugin framework. OIDC uses `openidconnect`; JWT uses `jsonwebtoken`; Basic uses
  Argon2id. Existing reqwest handles bounded HTTPS without redirect following.
- Scope follows root AGENTS: real frontend build before server tests, sequential
  Rust builds, no private remote/desktop checks, no local Docker release build.
  Canonical OpenWiki is read-only; the retired Wiki pipeline is not invoked.

## Boundaries

| Surface | Admission |
| --- | --- |
| Frontend static/login assets, `/api/health`, `/api/access/status` | Public, no business data |
| Setup exchange | One-use CLI key + browser-only limited session |
| Auth settings/test/confirmation | Setup/recovery session or active administrator + CSRF |
| Members/audit | Active administrator; recovery cannot read business/member data |
| Business `/api/*`, files, relay pairing/proxy | Current deployment mode + active Web user; unsafe requests require CSRF |
| WebSocket and SSE | Initial admission and periodic session/member/configuration revalidation |
| `/mcp` | Auth0 mode, explicit enablement, every-request JWT + scope + member check |
| Protected Resource Metadata | Public only when Auth0 MCP enabled; no business data |
| Runner poll/report/settled | Exact existing token/installation-authenticated callback routes |
| Local stdio MCP | Explicit owner-only file capability, sent only to loopback HTTP; nginx strips it |
| Separate project preview origin | Browser-bound one-use handoff, origin-scoped preview cookie tied to a live LVK Web session; local previews retained |

The server is one trusted development team. Members can execute code under the
same OS account, so server files, credentials and projects are not isolated from
one another at the OS level. HTTP authentication protects the external boundary.
The internal capability is not a member impersonation mechanism; its actor is an
explicit internal-agent identity. It cannot use auth-management endpoints. Existing
relay signatures/pairings are not treated as LVK membership. Relay/WebRTC account
interoperability is outside these local tests; the standard public ingress already
strips relay credentials and does not expose a cloud-relay authentication bypass.

Public source development switches to the existing built-asset `preview` mode
after removing legacy nginx Basic. Vite dev filesystem/editor/HMR endpoints are
outside the Rust API boundary, so explicit dev mode is rejected in that placement.
Legacy Basic-protected hot reload remains available. This does not replace nginx
or alter the IP certificate/renewal arrangement.

MCP admission requires `lvk:read`; write and merge tool calls also receive HTTP
403 with an `insufficient_scope` challenge before dispatch when the corresponding
scope is absent. Tool handlers retain a second permission check. MCP POST bodies
are limited to 2 MiB before parsing; rmcp remains responsible for protocol validation.

## Sessions and configuration

The public preset starts locked. Local mode is an explicit placement, not inferred
from the incoming socket address. Public URL and optional forced mode are deployment
settings (`LVK_PUBLIC_URL`, `LVK_AUTH_MODE`); the GUI cannot override them. There is
no environment Client Secret. Configuration changes are encrypted in the database
using the owner-only `data/access.key` file. Missing/corrupt data or key fails closed.
Recovery keys allow replacing configuration without opening business APIs.

Setup/recovery keys and sessions expire after 15 minutes. The CLI prints a key only
on explicit invocation; only its SHA-256 hash is stored. Issuance invalidates previous
unused keys and limited sessions. OAuth transactions are browser-bound, expire after
five minutes and are consumed atomically. Candidate configuration activates only
after a verified login and explicit confirmation in that same authorized browser.
Revision checks and one transaction revoke all old sessions during a switch.

Web cookies are host-only, HttpOnly, SameSite=Lax and Secure on public deployments.
Web sessions have a 12-hour absolute and 30-minute authenticated-request idle limit
(background API activity counts). Logout deletes the LVK session. Streams recheck
within two seconds. JWT keys are cached for ten minutes; unknown-kid refreshes are
throttled to once per 30 seconds. Metadata/token HTTP requests have 5-second connect,
10-second overall timeouts and a 256 KiB body limit. Only configured Auth0 tenant
origins are fetched; JWT `iss`, `jku`, and `x5u` never choose fetch destinations.

## Evidence

Initial protocol gate, before building the settings GUI:

`CARGO_TARGET_DIR=.build CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 cargo test -p server-access`

Passed: 6 tests, covering mock OIDC PKCE/state/nonce/issuer/audience, one-use browser
transactions, setup/recovery concurrency, fail-closed configuration, Basic/session
CSRF, persistence, MCP resource/audience (including arrays), scope errors, membership,
revocation, Basic transition and unknown-kid throttling. This is a local mock IdP/JWKS
result, not Auth0 or ChatGPT interoperability evidence.

Public-IP Auth0 callback, production Google/GitHub connection, ChatGPT OAuth/MCP,
refresh, consent and cancellation remain unverified: no tenant/client credentials
or interactive account authorization were supplied. The setup UI performs an actual
Web login when configured; it never marks ChatGPT verified from a mock or Web result.

Additional completed local checks:

- `cargo check -p server -p mcp` with the same isolated target/debug settings:
  passed; first dependency compilation took 31m32s. Final all-target Clippy below
  also compiles the updated production/test code; this is not a deployment check.
- `NODE_OPTIONS=--max-old-space-size=4096 pnpm --dir packages/local-web run build`:
  passed (latest build 12m10s). The first attempt exhausted Node's default heap;
  increasing this build-only limit resolved it. Existing bundle-size/Browserslist
  notices remain.
- `PLAYWRIGHT_BROWSERS_PATH=$PWD/.build/playwright node tests/server-access/browser.mjs`:
  passed against the real built UI with mock API responses. Covers locked setup,
  limited recovery, no public Local choice, exact IP callback display, CSRF headers,
  no pre-auth business requests, and no credential localStorage. This does not
  validate an Auth0 or ChatGPT account.
- `TMPDIR=$PWD/.build/tmp LVK_TEST_NGINX=/usr/sbin/nginx LVK_REQUIRE_NGINX_TEST=1 node --test --test-concurrency=1 deploy/server/*.test.mjs`:
  final run: 33 passed, 1 skipped (Docker Compose not available), 99.13s.
  Earlier runs hit the fixture's 3-second request / 20-second case deadlines under
  concurrent build load. Fixture budgets are now 10 / 60 seconds; no assertion,
  production timeout, TLS verification or authentication check was relaxed.
  Native nginx TLS/proxy fixtures ran; Docker image creation and container smoke
  were not run.
- `pnpm run developer:check`: 21 passed. This uses fake updater/container fixtures,
  not a real deployment.
- `python3 -m unittest discover -s scripts/local-runner -p 'test_*.py'`: 12 passed,
  local fixtures only, no physical Runner/device operated.
- Shell/Node/Python syntax checks passed for updated smoke, health and operator
  scripts. The disposable image smoke now exercises real HTTPS locked setup and
  backend Basic. Its browser's explicit internal-file fixture transport is a
  trusted local check, not evidence of end-user Web/OIDC login.

- `pnpm run local-web:check`, `pnpm run web-core:check`, `pnpm run ui:check`,
  and `pnpm run remote-web:check`: passed. Scoped ESLint for modified local/shared
  frontend files and the legacy frontend-path guard passed.
- Native nginx suite after adding managed ingress: 6 passed, including actual
  forwarding of Web credentials and removal of internal/relay capabilities.
  Targeted launcher/access tests: 16 passed, including IPv4/IPv6 certificate
  identity verification with TLS chain verification enabled.

- After restricting public source development to built assets,
  `node --test deploy/server/development.test.mjs deploy/server/access.test.mjs`:
  5 passed. `node tests/server-access/preview-static.mjs`: passed using real Vite
  preview and built assets; a private local fixture was not served through `/@fs`,
  and the dev HMR client endpoint was absent. Temporary listeners were stopped.
- Final `pnpm run format` passed; subsequent scoped Rust/Node formatting covered
  the later HTTP scope and source-development boundary fixes.

Final authentication and server regression checks:

- `CARGO_TARGET_DIR=.build CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 cargo test -p server-access`:
  13 passed in 62.84s, including Web/MCP JWKS rotation, authentication attempt limits,
  actual SSE body revocation, concurrent last-admin protection and the earlier protocol cases.
- `cargo test -p server --lib remote_mcp -- --test-threads=2` with the same build environment:
  33 passed in 96.28s; first code-generation build took 50m37s. These exercise the
  existing transport/tools/receipts, using legacy external-token fixtures where applicable.
  They do not establish an actual Auth0 or ChatGPT connection.
- The resulting `.build/debug/deps/server-7a693b5871dd0dd6` test binary, invoked with
  each filter and `--test-threads=2`: `routes::local_remote` 6 passed;
  `routes::sessions` 40 passed; `routes::project_chat` 29 passed;
  `middleware::origin` 6 passed; `routes::frontend` 1 passed. No full server suite
  or paid agent execution was run.

- `CARGO_TARGET_DIR=.build CARGO_BUILD_JOBS=1 CARGO_PROFILE_DEV_DEBUG=0 cargo clippy -p server -p mcp -p server-access --all-targets -- -D warnings`:
  passed; final incremental run 16m24s. Earlier runs reported collapsible nested
  conditions in the new authentication/MCP/CLI code. These were changed to Rust
  let chains; no lint suppression or authentication relaxation was added.

Final updated transport/authentication and stdio MCP checks completed successfully:

- `CARGO_TARGET_DIR=.build CARGO_BUILD_JOBS=1 CARGO_PROFILE_DEV_DEBUG=0 cargo test -p server -p mcp -p server-access --lib --bin vibe-kanban-mcp --no-run --message-format=json`:
  build succeeded in 170m15s. The emitted test executables were then run sequentially
  with `--test-threads=2`: server `remote_mcp` 33 passed (40.21s),
  `server_access` 13 passed (26.72s), `mcp` library 10 passed (0.44s), and
  `vibe-kanban-mcp` 2 passed. This includes the final HTTP 403 insufficient-scope
  transport change and local stdio authentication integration.
- After the host restart, the persisted Cargo `build-finished: success` artifact,
  individual test logs and aggregate exit statuses were checked. All four suites
  exited zero. No additional compilation was started: the final source files
  predate these results and the working tree contains no unstaged source edits.
- The host interruption was reported as memory thrashing. On resumption memory
  pressure had subsided, but the root filesystem had only about 367 MB available.
  Further heavyweight builds need adequate disk and memory capacity; these
  successful checks do not establish that concurrent builds are safe on this host.
  No other workspace's processes or caches were modified.

## Operational migration decisions

Use a separate preregistered Auth0 Regular Web Application with
`client_secret_basic` for ChatGPT. Do not reuse the LVK Web app credentials.
No DCR/CIMD implementation or Auth0 metadata proxy is added. The exact callback
comes from the current ChatGPT UI; registration capability and consent are still
real-account acceptance steps.

Existing nginx Basic remains an additional entry restriction until the operator
selects backend authentication after successful setup. Once Auth0 is active,
nginx credentials alone never authorize business APIs. An existing local agent
uses its explicit owner-only file capability; it is removed at nginx and never
inherited as an environment secret. Raw local API clients and the host updater
must authenticate too. Configuration/member management cannot use this capability.

Already admitted durable work/agent processes are not cancelled by Web logout or
member disable. Future API requests and open Web streams lose admission; existing
work retains its original actor and durable receipt. Historical UUID-1 records
are left unattributed. The UI uses a separate shared-workspace action flag instead
of rewriting ownership to the current user. A component browser regression proves
shared access works while the existing cloud ownership restriction remains.

`PLAYWRIGHT_BROWSERS_PATH=$PWD/.build/playwright node tests/server-access/workspace-actions.mjs`
passed: actual Workspace UI component opens shared workspaces without assigning
historical ownership; an unowned cloud workspace retains its click restriction.


## Requirement acceptance after follow-up (2026-10-15)

The successful scoped tests establish component behaviour, not complete acceptance
of every item in the original request. The following distinctions supersede any
reading of the test counts as a claim of full end-to-end completion.

| Original requirement | Implementation and evidence | Remaining acceptance work |
| --- | --- | --- |
| Three modes, locked public setup, no remote MCP in Local/Basic | Common access middleware, explicit Compose presets, key and mode tests | Actual image/Compose startup was not executed |
| One-use setup/recovery and browser-bound initial administrator | Transactional key exchange, hashed grants, PKCE/state/nonce, confirmation; mock HTTP tests | Verify the real CLI and browser flow against the packaged process |
| Web authentication and safe Basic to Auth0 switch | Mock transition success/failure and old Basic rejection; separate OS processes verify persisted active settings before, during and after activation | Real Auth0 and packaged server startup remain unverified |
| Persistent settings and sessions | A child test executable opens a new SQLite pool and key file; final activation is checked after the parent pool closes | This tests the production authentication library in fresh processes, not the full packaged server lifecycle |
| Auth0 JWT and MCP permission enforcement | Common production HTTP entry plus actual access middleware, JWT verification and rmcp project tool; scope denial and disabled member checks | JWKS are trusted fixtures; real Auth0 discovery and ChatGPT interoperability still need account verification |
| Revocation of continuous connections | Actual SSE body termination; real TCP WebSockets through SignedWsUpgrade for disable/expiry; actual preview WebSocket proxy cancellation | Real browser/provider session lifecycle remains part of E2E |
| Stable user identities, membership approval, admin-only management | Issuer/subject mapping, shared membership, last-admin transaction tests and UI | Real Google/GitHub identities and multi-browser workflows remain unverified |
| Existing feature preservation | 82 route regressions re-run; public separate-origin preview restored with dedicated admission; existing preview proxy tests retained | Preview requires explicit authorisation; browsers blocking third-party cookies use a separate tab. IP-only same-origin preview remains prohibited as before |
| IP HTTPS, Auth0 Free, ChatGPT OAuth | Existing nginx/ACME retained, native TLS fixtures and administrator instructions | Real IP callback, Free tenant settings, consent, resource/audience, token renewal and cancellation need actual accounts; no compatibility claim |
| Setup/admin GUI | Real built frontend tested against mock API responses | Browser fixture does not exercise the Rust backend or a real identity provider |

The previously identified JWT/tool, WebSocket and persistence component-integration
gaps now have executed regression tests. Real accounts and packaged deployment E2E
are explicitly separate from these local results; they are not claimed complete.
Previously admitted agent work continues after membership disable; subsequent
requests and connections are denied. This documented execution boundary remains.

### Disk cleanup

Only the inactive, task-owned `.build/debug/incremental` directory was removed,
after checking process working directories, executable paths and open file paths.
It contained about 6 GiB of reproducible Rust incremental state. Source, Git history,
compiled test executables, test logs, browser fixtures, live server data and other
workspaces were retained. Root filesystem availability increased from approximately
261 MiB to 6.2 GiB. No shared resource was held (coordination snapshot was empty),
and no build, deployment or service restart was performed for this review.


## Follow-up implementation and evidence

Optional public preview origins now have their own admission flow. The existing
`VK_PREVIEW_DOMAIN` allowlist and canonical public URL determine permissible HTTPS
origins; reserved ports, the application origin and arbitrary redirect targets are
rejected. An authenticated Web user explicitly confirms a handoff. A 60-second
one-use grant is bound to the initiating preview browser, exact origin and parent
Web session. It travels in a POST body, never in a URL. Preview cookies cannot
access LVK APIs and are stripped before forwarding to project code. They expire
with the parent session, configuration revision or member disable. Preview SSE
and WebSockets have a revocation lease. No domain is required for the existing
public-IP installation; same-origin untrusted project HTML remains forbidden.
Browsers blocking third-party cookies can use a separate preview tab.

Added acceptance coverage uses actual WebSocket TCP connections for membership
and expiry revocation; a fresh child test process reopens the SQLite database and
key before/during/after Basic to Auth0 activation. A downstream integration test
joins the real access middleware and JWT verifier to real rmcp tool dispatch,
including project reads, insufficient-scope responses and member disable. Its
explicit dev-only `test-support` feature seeds trusted fixture JWKS; normal server
builds have no network or authentication bypass switch. The standard Auth0 issuer
restriction is unchanged. These fixtures do not prove real Auth0/ChatGPT or packaged
container interoperability.

- Combined `cargo test -p server -p mcp -p server-access -p preview-proxy --lib --bin vibe-kanban-mcp --no-run`
  with `.build`, one Cargo job and debug info disabled: passed in 8m27s.
- The resulting test executables were run sequentially with one test thread:
  server `remote_mcp` 34 passed (50.45s); production SignedWsUpgrade revocation
  1 passed (4.07s, disable and expiry cases); server-access 17 passed (58.63s,
  including its child-process helper); preview-proxy 34 passed; internal MCP
  library 10 and CLI 2 passed.
- The new preview handoff case covers wrong-browser exchange, replay, cross-origin
  cookie reuse, inability to use preview credentials for business APIs, stripping
  main/preview credentials before project forwarding and logout revocation.
- Existing local_remote 6, Session 40, project_chat 29, Origin 6 and frontend 1
  regressions passed again with the final server test executable (82 total).
- Targeted launcher/access tests: 17 passed. The native nginx TLS suite passed
  all 6 cases after replacing the former blanket preview-denial expectation with
  backend admission checks. An earlier broad run had that outdated expectation
  fail; its other 34 cases passed and Docker Compose was skipped. No assertion
  for authentication, credential stripping or TLS verification was removed.
- Full `pnpm run format` and subsequent scoped Rust/Node formatting passed.
  Root `pnpm exec prettier` was unavailable; the installed local-web Prettier
  successfully formatted the deployment files instead.

Builds ran under a foreground monitor with one Cargo job. Large compilation
waited for peer compilers; smaller checks only started with sufficient available
memory. The monitor would terminate only its own process group below 1600 MiB
available memory or 2000 MiB free disk. No such stop was necessary. Other workspace
processes, caches and live server state were not modified.


Final follow-up lint and capacity checks:

- `cargo clippy -p server -p mcp -p server-access -p preview-proxy --all-targets -- -D warnings`
  with the same isolated target, one job and disabled debug info: passed in 10m14s.
  This includes normal server code and test targets. The normal dependency graph
  (`cargo tree -p server -e normal,build,features --offline`) contains no
  `test-support` feature.
- After all owned builds settled, process working directories, compiler arguments
  and open files were checked before removing only the owned incremental cache.
  This recovered 1.87 GiB; available disk was 8.57 GiB immediately afterwards.
  Test executables and evidence logs were retained. No live deployment or other
  workspace cache was changed.
- The first small type check found one missing fixture initializer field after
  adding preview configuration; it was corrected before all successful runs.
  No assertion, production authentication rule or resource stop threshold was
  weakened to obtain the final results.
