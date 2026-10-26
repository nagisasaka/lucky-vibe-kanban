# Project key migration compatibility

Version `20261025120000` originally assigned every existing Project `LOCAL`
before creating its unique index. It succeeds for zero/one Project and rolls
back for multiple Projects. Its original SQL remains byte-for-byte immutable.

`src/migration_compat.rs` resolves two immutable SQL variants, each with explicit
LF/CRLF checksums, through SQLx's public `MigrationSource` / `Migrator::new` APIs. If the original checksum was
applied, the original SQL is selected for normal SQLx validation. Otherwise only
an absent receipt or the exact compatible checksum selects the compatible SQL.
Historical LF/CRLF receipts select their exact SQL bytes even across checkout line
endings. Unknown checksums and modified source artifacts fail before the legacy
Windows checksum fallback. Neither
applied checksum nor history receipt is updated by this compatibility layer.
SQLx records the checksum of the SQL it actually executes, in the same transaction.
Line-ending compatibility is limited to this one version. Validation of all other
migration versions is unchanged; this is not a general cross-platform history converter.

Compatible assignment sorts by Project UUID, preserving `LOCAL` for the first
and assigning `LOCAL2` through `LOCAL99999` to the others. Existing Issue rows,
including historical `LOCAL-N`, are untouched. The default stays `LOCAL`; the
ordinary API still requires an explicit unique key. More than 99999 Projects
fails the existing ten-character CHECK and rolls back, rather than truncating a
key. No aliases, key edits, or Issue renumbering are introduced.

Use `db::run_migrations` for startup and
`SQLX_OFFLINE=true DATABASE_URL=sqlite:<explicit-existing-authorised-path> cargo run -p db --bin migrate_compat`
for explicit migration tooling. `scripts/prepare-db.js` uses this same entry point
on its own temporary database. Do not use raw `cargo sqlx migrate run` on an
instance upgraded with the compatible variant: the directory-only source knows
only the original variant and will correctly report a checksum mismatch. Do not
repair that mismatch by editing `_sqlx_migrations`. Older server binaries also
reject the new checksum; downgrading is not supported by this change.

`tests/project-keys/migration_compat.py` is a lightweight SQL/checksum model using
disposable in-memory fixtures; it does not execute the Rust selector or the full
historical migration chain. Rust tests in `migration_compat.rs` exercise the real
SQLx/startup path (`cargo test -p db --lib migration_compat::tests`). Existing tests
using raw `sqlx::migrate!` on empty databases still exercise the original variant;
they are not coverage of the compatibility selector.
