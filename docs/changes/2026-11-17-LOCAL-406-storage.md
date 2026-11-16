# LOCAL-406: repository artifact ownership

Shared `persistent` is reserved for user-retained outputs. The retired startup
importer no longer reads legacy Change Manifest files. Applied migrations and
`repository_manifest_imports` rows remain unchanged. Final Manifests continue to
use the instance DB; no file fallback, deletion or live migration is performed.

This change does not authorise deployment or disposal of existing files. Before
upgrading, stop all old server/agent hosts and settle unfinished coding and Wiki
operations using their original version and fixed paths. Do not run the old and
new lock implementations concurrently. Keep the old artifacts and matching DB
backup until recovery is independently verified. A clean Git checkout or a
successful AgentRun alone does not prove that restoration/publication finished.

## Internal data placement

- `INSTANCE/runtime/tmp/repository-memory/REPO_UUID/knowledge/`: coding drafts,
  source publication snapshots, Wiki original/restoration journals, frozen Sync
  inputs and Bootstrap reports. This is outside repository shared storage and
  outside the shared cache/tmp clear API. The default non-instance runtime root
  follows the existing runtime/worktree volume policy.
- `INSTANCE/runtime/cache/repository-memory/REPO_UUID/document-inventories/`:
  deterministic frozen-source inventories. Missing whole inventories can be
  regenerated and must match the DB-owned reference/digest; corrupted content is
  not silently accepted.
- `/tmp/lvk-repository-locks/REPO_UUID/`: one container-wide Repo lock domain,
  independent of Workspace and instance path aliases. Lock files are never
  removed by artifact cleanup. Container replacement may discard these locks
  only after all old processes have exited; recovery artifacts and their Git/DB
  targets must stay on their volume.
- Instance DB `repository_coding_runs`: immutable Run identity and completion
  history/diagnostics that are read later. Recovery snapshots are not moved into
  this table. Applied historical migrations and import-marker rows are untouched.

Successful source completion cleans its draft/snapshot only after exact Git tree,
DB Manifest and frozen publication identity reconciliation. Invalid drafts remain
for inspection. Wiki inputs/reports are cleaned only after restoration and durable
publication/reconciliation. Auto-merge journals are cleaned after restored final
SHA and Git/DB publication bookkeeping. Failures and unknown results retain files.
Cleanup errors surface through existing completion diagnostics. The existing Wiki
state row atomically retains only the exact confirmed cleanup IDs; the repository
monitor retries after crashes or partial deletion without interpreting error prose.
Auto-merge stays in its existing post-processing phase until journal cleanup ends. There is no time-based expiry.

The first upgrade performs a read-only refusal check for unfinished legacy Run/publication
or instruction-restoration files and held legacy locks. A missing legacy storage
root remains unacknowledged; verify its original volume rather than treating it as empty. It never reads legacy
`events/`, imports history, moves data or deletes old files. This check cannot
prove an old binary will never start again: stopping all old processes is an
operator prerequisite. A refusal requires offline settlement using the old version
and original absolute paths. After successful preflight only `artifact_layout: 2`
is saved in the existing Wiki state row; subsequent starts and new registrations
never inspect user `persistent/knowledge`. No automatic file migration or bypass
is provided; all import-history rows and final Manifests stay unchanged.

DB-only status, Manifest discovery and completion-history readers neither create
nor require artifact directories. User shared persistent/cache/tmp links and
provider access remain available. This change does not alter development bundle
or restart procedures (LOCAL-407).
