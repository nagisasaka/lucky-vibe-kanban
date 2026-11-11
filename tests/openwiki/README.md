# OpenWiki admission regression

## Public package compatibility (0.9.0)

The pin is `assets/openwiki-version`. Official release:
https://github.com/langchain-ai/openwiki/releases/tag/v0.9.0
The unscoped npm package is `openwiki`; Node.js >=22.22.0 is required.
Install test versions into disposable prefixes, never the shared global runtime.
Set `OPENWIKI_BIN` to the prefix's `node_modules/.bin/openwiki` and
`OPENWIKI_TEST_PACKAGE_ROOT` to its `node_modules/openwiki`. Use a disposable
`HOME` and `OPENWIKI_CONFIG_DIR` for installer/MCP tests; when invoking the Rust
driver, retain the existing `CARGO_HOME` and `RUSTUP_HOME` toolchain paths.
The installer adapter uses `openwiki` on PATH, so prepend the same prefix's bin.

```sh
node --test scripts/openwiki-probe.test.mjs
node scripts/test-openwiki-host.mjs
node scripts/test-openwiki-focused.mjs
# Production restoration/publication guard, including regular or absent CLAUDE.md:
node scripts/test-openwiki-host.mjs --lvk-setup --claude-file
node scripts/test-openwiki-host.mjs --lvk-setup --claude-absent
# Set OPENWIKI_PREVIOUS_BIN to an isolated public 0.7.2 CLI:
node scripts/test-openwiki-install.mjs
node scripts/dev-cargo.mjs test -p services openwiki --lib -- --include-ignored
```

These fixtures use temporary repositories and no model. The version probe uses
public MCP `initialize.serverInfo` and starts no generation lifecycle. In the
public 0.9.0 package, non-TTY `--help` can render an Ink raw-mode error while
exiting successfully; it is not a headless version check. Docker installation,
image smoke and the Rust adapter share `assets/openwiki-probe.mjs`.

The host retains six lifecycle and four retrieval tools. Public schema additions
are `submit_plan.deferUnplannedClaimIssues` and `read.includeProvenance` (both
optional). Begin returns at most 200 changed paths and 50 Claim issues, exact
totals/truncation flags, all Claim-issue page names and per-page-baseline commit
hints. Omitted paths must be investigated through Git; page-local issues are
available through `next_page`. Commit messages are hints, not source evidence.

For focused updates, unrelated stale Claim pages can be deferred by the public
plan flag. The focused fixture proves that debt survives restart and successful
finish, reappears on the next begin, and is automatically queued by a default
plan. A completed scoped lifecycle does not prove whole-Wiki reconciliation.
LVK's audit closure, source-drift rejection, sequential writer and publication
guards are unchanged. These tests do not prove a live model's research quality,
image execution, deployment or canonical Wiki publication.

## Admission and recovery

Run from the repository root:

```sh
node --test tests/openwiki/admission.test.mjs
bash tests/openwiki/recovery.sh
```

The first command guards the standalone admission call sites. The second uses
an isolated instance and the real recovery entry with Git, SQLite and persisted
repository memory. It covers acknowledged publication with an unrelated backlog,
a new pending event, an old pending event and a direct main change. Repeated
recovery must leave state and unacknowledged events intact and create no workspace,
session, agent run or workflow. A CLI trap prevents any model launch if admission
regresses. The runner removes its own instance after the foreground test exits.
It does not restart a server or call a real model.

Remaining admission and recovery boundaries:

- UI `POST /repos/{repo_id}/memory/sync` → `openwiki::sync` → private
  `start_reconciliation_request`: explicit update, re-sync or initial generation.
- Remote MCP `repository_wiki::sync` → `openwiki::sync_checked` → the same private
  admission: explicit authorised request with source and branch preconditions.
- `prepare_run` → `workflow_runtime::bootstrap::start`: initial generation only,
  reached from the two explicit request paths above.
- `recover_repository`: resumes a persisted Bootstrap owner or exact reserved
  AgentRun, then performs existing completion/publication/cleanup. The startup
  and five-second monitor invoke recovery but cannot admit a new owner.
- Auto-merge's existing assigned Codex maintains Wiki within its candidate;
  `integrations/conversation.rs` checks the code/Wiki identity and writer proof
  before publication. `record_wiki_result` records only that candidate's source
  event receipts. No separate maintenance workspace is admitted by publication.

PR observation, semantic integration recovery, event selection and frozen inputs
remain available for explicit maintenance and existing owners. They do not grant
admission. OpenWiki enablement governs Wiki maintenance, not source auto-commit
or Change Manifest publication for normal successful coding Runs. It is not an
event auto-sync setting.
