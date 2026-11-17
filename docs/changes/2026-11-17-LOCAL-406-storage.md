# LOCAL-406: repository artifact ownership

Shared `persistent` is reserved for user-retained outputs. The retired startup
importer no longer reads legacy Change Manifest files. Applied migrations and
`repository_manifest_imports` rows remain unchanged. Final Manifests continue to
use the instance DB; no file fallback, deletion or live migration is performed.

This change does not authorise deployment or disposal of existing files. Follow
the existing restart and recovery procedures. Do not run the old and new lock
implementations concurrently. Keep recovery artifacts and matching DB backups
until recovery is independently verified. A clean Git checkout or a successful
AgentRun alone does not prove that restoration/publication finished.

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

Startup runs schema migrations without inspecting retired repository artifacts or
legacy reservations. It does not read or acknowledge old artifact layouts, scan
user `persistent/knowledge`, or modify historical state to permit startup. Old
files, import-history rows and final Manifests remain untouched. There is no
legacy compatibility reader, replacement gate or bypass setting.

DB-only status, Manifest discovery and completion-history readers neither create
nor require artifact directories. User shared persistent/cache/tmp links and
provider access remain available. This change does not alter development bundle
or restart procedures (LOCAL-407).

## Startup gate removal inventory

The retired upgrade-only mechanism has been removed in full:

- `crates/db/src/models/repository_state/cutover.rs`: legacy JSON reader, writer
  query, storage-root/layout checks, held-lock probe, acknowledgement write,
  refusal errors and both dedicated cutover tests.
- `crates/db/src/models/repository_state.rs`: module and public export.
- `crates/db/src/lib.rs`: calls from `new` and `create_pool` (also used by
  `new_with_after_connect`). Both retain migrations and the runtime schema guard.
- `crates/utils/src/repository_memory.rs`: `artifact_layout` field and default;
  generated `shared/types.ts` follows the Rust type. Stored payload fields are
  not rewritten on startup.
- `crates/db/Cargo.toml` and its lockfile entry: the now-unused direct `libc`
  dependency. Other crates retain their own dependencies.

Normal startup regression coverage is in `crates/db/tests/startup.rs`. It uses
private subprocess instances for both constructors, empty databases, three
historical audit_failed/reserved records without hosts or leases, existing
Manifest/import history and all former layout marker variants. It compares
history and migration receipts across repeated startup, with malformed retained
files, a held old lock and a missing old storage root. Current run exclusivity,
restoration, cleanup ownership, schema checks and official restart procedures
remain separate and unchanged. No replacement gate, exception list, setting or
historical correction is introduced.

Canonical OpenWiki is read-only in ordinary coding runs. These stale references
were inventoried for authorised maintenance; they have **not** been updated by
this change:

| Page | References requiring removal/update |
| --- | --- |
| `openwiki/quickstart.md` | LOCAL-406 navigation row describing cutover refusal |
| `openwiki/architecture/system.md` | cutover source metadata and startup refusal/layout paragraph |
| `openwiki/concepts/repository-memory.md` | cutover source metadata; DB startup inspection, refusal conditions and layout acknowledgement paragraphs |
| `openwiki/operations/openwiki-maintenance.md` | cutover source metadata; cutover-boundary link and read-only refusal paragraph |
| `openwiki/operations/development.md` | cutover source metadata; dedicated refusal-test link and layout/missing-root bypass paragraph |

Applied SQL/checksums, past Change Manifests, live data/files/backups, merge,
deployment and restart are outside this implementation. The operational warning
against concurrently running old and current lock implementations remains; it is
not an application startup check.
