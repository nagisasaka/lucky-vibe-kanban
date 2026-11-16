# Project transfer core (LOCAL-396, first slice)

`db::project_transfer` implements a deliberately partial, additive transfer of
Project-owned Issue records. It is not yet exposed by server routes, MCP or UI.
It does not provide a whole-instance backup or a supported production migration
command. No runtime authentication boundary has been added or bypassed.

The public library operations are:

- `coverage()`: describe included free text and all exclusions before export.
- `export_project(pool, project_id, output)`: take a source read transaction and
  publish a standalone SQLite file without replacing an existing output path.
- `inspect_package(path)`: validate the file and return scope, counts, source
  Repo identities/display names and missing attachment summaries for review.
- `import_project(pool, path, key, organization_id, repo_map)`: insert a new
  Project and its records in one destination `BEGIN IMMEDIATE` transaction.

A future authenticated caller must authorize the source Project and destination
Project creation, display the scope and free-text notice before export, collect
an explicit destination key, and map every source Repo to a user-selected,
already registered destination Repo. The core neither registers nor touches Git
repositories. It does not read provider HOME directories or shared Repo storage.

## Portable data and reference closure

| Source | Included fields / handling |
| --- | --- |
| `projects` | Source ID for remapping, name, creation/update time, Issue numbering high water |
| `local_project_metadata` | Color and order; organization is explicitly supplied at import |
| `local_project_statuses` | Project-owned status identity, name, color, order, hidden flag |
| `local_issues` | Title, body, number, status, priority/dates, completion time, order, parent, archived flag, times, current version pointer/revision |
| `issue_versions` | All selected Issues' named bodies and version revisions |
| `local_issue_comments` | Message, times and same-Issue reply structure; no actor identities |
| `local_issue_relationships` | Outgoing relationships with both endpoints inside the selected Project |
| `local_tags`, `local_issue_tags` | Project-owned tags and selected Issues' internal tag links |
| `project_repos`, `repos` | Referenced Repo ID and display name only; no paths, scripts, configuration or Git bytes |
| `project_attachments`, `attachments` | Project-level attachment name/size summaries only; no file path, hash, bytes or live attachment association |

An external parent/reply pointer, outgoing external relationship or external tag
link is excluded and counted in the report. Its foreign record and ID are not
imported. Invalid remaining closure fails export/import rather than expanding
selection into another Project. All imported row IDs are new. Issue numbers and
numbering high water survive, while `simple_id` uses the explicit new Project
key. Version insertion reconciles the existing Initial-version trigger without
changing it or disabling it. Arbitrary JSON metadata is not copied.

No `Workspace`, `Session`, `Run`, conversation, legacy execution log, native audit,
Change Manifest, design note, Todo, actor assignment/follower/reaction, queue,
process registry, recovery journal, lock, environment, credential or global
configuration is included in this slice. This is an implementation limit, not a
change to the agreed eventual Project portability scope. In particular, completed
Run conversations and DB-confirmed Project-owned Change Manifests still need a
safe display projection and their own fixture coverage. No unsupported legacy
log is copied as raw data.

Bodies/comments are preserved text, not sanitized content. They may contain
sensitive prose and historical path or attachment references. Those strings are
not rewritten, followed or resolved by the transfer core. The core creates no
attachment records or old-root/worktree associations. Before user-facing rollout,
the existing viewer must explicitly represent unavailable attachments and must
not silently resolve historical attachment references against old storage.

## File and transaction contract

The package is a single SQLite database with exactly the fixed `package` table
containing one typed JSON payload. It has format `lvk-project-issues`, version 1,
a non-nil export UUID and the explicit allowlisted snapshot. This distinct format
identifies the partial scope; it must not be advertised as a complete Project
archive. SQLite schema SQL from input is never executed in the destination.
Unknown fields/versions, other database schemas, views/triggers, bad references,
cycles and oversized/non-regular inputs fail validation. The file limit is 32 MiB
and payload limit is 16 MiB. Symlink inputs are rejected. There are no extraction
paths or input-controlled attachment reads. Input schema and payload are read
inside one read-only transaction with `trusted_schema=OFF` and `query_only=ON`.
The package connection is immutable so adjacent WAL/journal files are never
consumed as archive content. Callers must retain a private, stable input file
for the duration of inspection/import; this library is not a sandbox against
concurrent writers with access to that file.

Export uses one source transaction, allowlisted SELECTs and a temporary output
in the requested directory, then publishes without replacing an existing file.
Import validates input before writing. An exact one-to-one Repo mapping is
required, and destination Repo existence/key conflicts are checked under the
writer transaction. Repo settings and existing Projects/Issues are never updated.
Any insertion or final constraint failure rolls back all rows and the receipt.

`project_transfer_receipts` commits the export UUID, canonical snapshot SHA-256,
new Project ID and coverage/missing-reference report together with the import.
Repeated UUIDs or identical snapshots (including reordered copies with a new
export UUID) are rejected. Changed snapshots may create another new Project with
a different explicit key; they cannot update the prior import. Receipts survive
Project deletion to keep retries detectable, without blocking the existing
Project deletion lifecycle. The fingerprint is duplicate detection, not a
signature or proof of archive authenticity.

## Verification and remaining integration

`crates/db/tests/project_transfer.rs` uses task-owned temporary SQLite databases,
the real migrations and disposable Git fixtures. It exercises export/import,
old-root removal and reopening, internal reference remapping, shared-Repo
isolation, missing attachment summaries, duplicate/key rejection, malformed
inputs and transaction rollback. These are DB-level checks, not evidence of
browser history rendering, authentication integration or production migration.

Remaining work includes authenticated API/upload/download entry points, existing
UI scope/repo/key selection, optional conversation exclusion and safe completed
conversation projection, Manifest ownership/remapping, legacy-log omission
reporting, and end-to-end viewer verification with the old root unavailable.
