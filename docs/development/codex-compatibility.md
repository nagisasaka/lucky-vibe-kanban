# Codex CLI / App Server compatibility

LVK's server image pins `@openai/codex@0.163.0` in `Dockerfile` (previously
0.154.0). This is the stable npm release checked on 2026-10-18, not the alpha
channel. LVK launches the host's `codex app-server` over stdio, performs
`initialize`/`initialized`, and uses thread/turn RPCs. An executable override
still selects that host's CLI; updating the Dockerfile does not update a running
container or an external host.

For a source-development or external execution host, install the tested version
in that host's managed tool environment with
`npm install --global @openai/codex@0.163.0`, then check `codex --version`. For Docker distribution,
build and deploy a new server image through the normal release process. Do not
replace a running instance's tools as part of a source-only task. This change
has not published or deployed an image.

## Wire types and upgrade boundary

The Rust `codex-protocol` and `codex-app-server-protocol` dependencies deliberately
remain on `rust-v0.144.1`. They describe the wire subset LVK consumes, not the
installed CLI version. Updating these crates to either 0.154.0 or 0.163.0 was
attempted: the newer protocol pulls in rollout/otel/state, requires upstream
WebSocket forks, and conflicts with LVK's SQLx 0.8.6 SQLite link dependency
(`libsqlite3-sys` 0.30.1 versus 0.37). A database dependency migration is outside
this change. No fork patches or failed dependency updates are retained.

The offline smoke runs the real 0.163.0 executable against a temporary local
Responses endpoint, with a temporary Codex home and no credentials or external
model requests. Its captured start/resume/Goal responses and notifications are
checked by Rust deserialisation tests. This establishes compatibility for the
exercised methods, not every optional App Server feature or model behaviour.

```sh
python3 scripts/test-codex-app-server.py npx --yes @openai/codex@0.163.0
cargo test -p executors codex --lib
```

To refresh the sanitised fixture while deliberately upgrading the supported CLI:

```sh
LVK_CODEX_SMOKE_FIXTURE=crates/executors/fixtures/codex-app-server-0.163.0.json \
  python3 scripts/test-codex-app-server.py npx --yes @openai/codex@0.163.0
```

Review the fixture and update the Docker pin and release test together. The
smoke checks native Plan/Default instructions reaching Responses, completed
notifications (including a native Plan item), resume identity, in-flight Steering,
and Goal activation/run/pause/clear. Native
Goal activation, completion ordering and failures also have deterministic Rust
stdio tests. Paid inference, browser interaction, and a Docker image build are
separate validations.

## Plan and retired Plan with Goal

Plan uses `turn/start.collaborationMode.mode = "plan"` and
`settings.developer_instructions = null`, selecting Codex's built-in Plan
instructions. No `/plan` text is prepended. See the
[official App Server contract](https://learn.chatgpt.com/docs/app-server).

Two regressions are covered:

- Applying Auto/Supervised permissions to a legacy `plan: true` preset previously
  cleared its planning flag before resolving the execution mode. The mode is now
  captured first; an explicit Code/Plan/Goal selection still takes precedence.
- After approving a plan, implementation switches to Default mode. A subsequent
  reasoning-depth update previously reused the original Plan flag and re-entered
  Plan. The client now records the transition for subsequent settings updates.

The selector offers Code, Plan and Goal. Old `plan_with_goal` values remain
readable in stored runs, profiles, REST and MCP requests; no database rewrite or
enum removal occurs. A newly launched old value executes as Plan and cannot
implicitly create a Goal. Restoring it into the composer also clears obsolete
Goal options. The separate contract-writing prompt, objective editor and
approval-to-Goal transition are removed. Old `plan-goal-draft` control requests
receive an explicit retirement error. Existing native Goals remain accessible
through normal Goal controls and `/goal resume`; create new Goals explicitly.

## Boundary with agent visibility work

No changes are made to canonical event shapes, adapter/mapper versions or old
Audit fixtures. Native notifications continue to be retained, and child-thread
notifications remain excluded from parent run lifecycle completion. The new
fixture includes 0.163.0 fields such as resume pagination and thread settings;
resume continues to decode only the control fields it needs. Subagent display
and context-usage presentation belong to the separate visibility card. Its
implementation should use the raw notifications and independently verify any
new fields it chooses to project.

## Validation in this workspace

- `cargo test -p executors codex --lib --no-fail-fast`: 124 passed; six ignored
  entries are subprocess fixture entrypoints exercised by their parent tests.
  The first run found an omitted `turn/started` in the updated approval fixture;
  correcting the mock notification sequence made the regression pass.
- `cargo clippy -p executors --all-targets -- -D warnings`: passed.
- The captured 0.163.0 response test was rerun after adding streamed native Plan
  items to the fixture and passed.
- Web-core Vitest tests for executor restoration, canonical controls and Goal
  validation: nine passed. Web-core/local-web TypeScript checks and local-web
  lint passed.
- `pnpm run server:check`: 29 passed, seven skipped (Docker Compose unavailable;
  nginx unavailable for six ingress tests). The release tests were rerun after
  tying the Docker version to the compatibility fixture: three passed.
- The real-binary offline smoke passed all modes, native Plan item conversion,
  resume, Steering and active Goal lifecycle checks. It supplies deterministic
  model responses; it does not establish live model planning quality.
- Browser E2E, live paid inference, container builds and deployments were not
  run. This source-only change has no database migration or generated API type
  changes, and makes no claim of completed formal Integration.

The normal workspace `target/` and installed frontend dependencies are retained
for subsequent development. Temporary smoke homes and local HTTP servers are
cleaned up by the smoke script.
