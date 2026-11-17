# Project transfer (LOCAL-396)

`db::project_transfer` exports selected LVK-owned Project data to one standalone
SQLite file and imports it as a new Project. Authenticated API and the existing
Project utilities UI expose explicit export, preview, Repo mapping, unique key
selection and additional import. Project settings also offer import without an
existing Project. Import success refreshes the existing Project list; the new
Project link appears only after its ID is observed, without changing missing-
Project/404 behavior. A delayed list refresh never relabels a committed import
as a failed import. This is not an instance backup or Git transfer.
Existing destination Projects, Issues and Repo settings are never updated.

## Scope and privacy

The fixed allowlist includes Project name/times, statuses, Issues and their
unchanged bodies, named versions, comments, internal relationships, tags, and
Repo identities/display names. Design Notes, Todos, Workflow definitions, assignees/followers/reactions and
all other nonallowlisted Project data remain unimplemented/excluded. Actor
identities, global settings, Repo paths,
scripts, credentials, environment, provider metadata and arbitrary JSON are not
copied. External relationship/parent/reply/tag references are removed and counted;
remaining invalid closure fails validation. All imported internal identities are
new. Issue numbers survive; `simple_id` uses the explicitly selected new key.

Conversation export is **off by default**. `export_project` retains that default;
`export_project_with_options(..., include_history)` requires an explicit opt-in.
Eligible source Runs have a matching Project–Issue–Workspace link and Session,
a terminal ledger status, current canonical projection, and matching terminal
state/latest attempt. The existing reader's typed `RunState` JSON must also have
current schema/reducer versions, matching terminal status and matching
Run/Session/Turn/latest-attempt/cursor identities. Stale, active, malformed,
ambiguous and inconsistent projections are omitted.
The source read transaction also reads their canonical `agent_events` and
DB-confirmed `repository_change_manifests`; it never follows files or HOME paths.

Conversation fields are only canonical user `Message.content` and assistant
`Message.content` with `final_output=true`. This uses the same canonical text
field as `canonicalAgentConversation.ts`'s existing normalized display projection;
it does not reinterpret native events or manufacture text from request envelopes.
Thinking, nonfinal assistant commentary, tool calls/results, provider raw events,
native references, authentication, structured secrets and other event variants
are discarded. `agent_turns.input_message`, request envelopes and raw logs are
never exported. Unsupported/legacy logs are explicitly reported as omitted;
there is no fallback raw copy. Free text may contain sensitive prose or paths;
this is **not** a claim of complete text sanitization. The UI discloses the scope
before optional conversation inclusion.

Manifest selection requires the DB event ID to match an eligible Run, Workspace,
registered Workspace Repo. Required historical Repo identities/display names form
the reference closure even after removal from the current Project preset. The validated payload's
Repo/Workspace/event identities must agree, and an optional task ID must agree
with the linked Issue. Only the typed semantic fields, creation time, commit IDs,
target branch and validated relative changed paths are retained. Repo-wide shared
storage, drafts, publication snapshots, journals and Wiki intermediates are not
copied. Manifest inclusion does not depend on conversation opt-in.

Attachment bytes are excluded. Project attachments and selected completed
Workspace attachments contribute original-name/size missing-file summaries only;
paths, URLs, hashes and live attachment associations are not imported. Imported
summaries survive re-export through the receipt. Ambiguous legacy task attachment
associations are not inferred. Git objects, worktrees, uncommitted changes and
native/provider session continuation are excluded. Users prepare and explicitly
map their own destination Repos; no clone, commit or push occurs.

## Archive viewing and limitations

Imported completed records live only in `project_transfer_history`, a dedicated
read-only display archive. Issue, Workspace, Session, Run and Manifest identities
are remapped consistently, including Manifest Repo mappings. Different identity
kinds and ambiguous Workspace/Session ownership are rejected. No `agent_runs`,
`agent_turns`, provider sessions, process registries, queues or launch gates are
inserted. Archived identities cannot resume native execution or resolve old paths.

`list_imported_history(pool, project)` serves the Project's archive viewer.
This is a separate read-only section in the existing Project UI, **not** recreation
of the ordinary executable Workspace/Session timeline. Imported Manifests are
viewable there with the completed conversation, but are **not** published into
the destination Repo's canonical Manifest reader/Wiki consumption stream. This
preserves the boundary between historic imported evidence and new local Runs.
The archive renderer presents retained bodies as text and unavailable attachment
references, without resolving old roots. Imported Issue bodies, named versions
and comments also use literal text for reading/editing. The frontend permits the
ordinary rich editor only after the report confirms a non-imported Project;
loading or failed lookup stays inert. This preserves stored body strings and
prevents old images/URLs/attachment references from being resolved elsewhere in
the existing Issue UI. Full native timeline/resume and legacy
log fidelity are not implemented.

## Format, transactions and retries

The single SQLite file has exactly the fixed `package` table with one typed JSON
payload, format `lvk-project-issues`, version 2 and a nonnil export UUID. Version 1
Issue-only packages remain readable. Empty/default history fields are omitted
from serialization, preserving the version 1 Snapshot content fingerprint.
Unknown fields/versions, executable schema objects, invalid references/cycles,
nonregular/symlink inputs and oversized packages fail validation. Limits remain
32 MiB for the file and 16 MiB for its JSON payload. Input schema SQL is never
executed in the destination. A read-only immutable connection, trusted schema off
and query-only mode prevent adjacent WAL/journal or supplied views from expanding
input. Packages have no extraction paths or input-controlled filesystem reads.

Export takes one source read transaction and publishes a temporary standalone
file without replacing an existing output. The API uses private temporary files;
preview and import verify the same uploaded SQLite bytes using a SHA-256 digest.
The digest binds review to input bytes; it is not an authenticity signature.

Import validates the entire package and exact one-to-one registered Repo mapping,
then uses one destination `BEGIN IMMEDIATE` transaction for all new records,
archive projections and receipt. Existing key or missing Repo errors fail safely.
Mid-insert or final constraint errors roll back everything, including the receipt.
Export identity and canonical Snapshot content hash are both unique receipt keys:
retries or identical content with a new export UUID are rejected. A meaningfully
changed snapshot can only add a new Project with another explicit key.
Receipts survive Project deletion to keep retries detectable.

## Verification

`crates/db/tests/project_transfer.rs` uses isolated SQLite databases, real migrations
and disposable Git fixtures. It covers internal ID remapping/body preservation,
shared Repo/other Project exclusion, active Run omission, opt-in/out, canonical
message allowlisting, Manifest ownership/metadata, missing attachments, old-root
removal and pool reopening, archive read/re-export, corrupted packages, identity
collisions, v1 receipt fingerprints, duplicate/key rejection and transaction
rollback. API tests cover authentication/CSRF and same-byte preview/import.
These checks do not claim production migration, native provider fidelity or a
restarted running service. No production DB or live GitHub is needed.

The isolated UI fixture is `node tests/project-transfer/browser.mjs`. It uses
real frontend components with synthetic HTTP responses and checks opt-in, byte
binding, file replacement, archive/Manifest and inert text rendering on mobile
and desktop in both themes. It does not establish live server/browser/provider
interoperability; actual SQLite and authenticated API behavior have separate
backend fixtures.
