# OpenWiki admission regression

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

PR observation, semantic integration recovery, event selection, frozen inputs and
Workspace Memory remain available for explicit maintenance and existing owners.
They do not grant admission. Repository-memory enablement still governs memory
and enabled candidate maintenance; it is not an event auto-sync setting.
