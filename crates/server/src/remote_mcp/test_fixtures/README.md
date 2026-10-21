These SQL files are read-only reference copies from LOCAL-19 commit `7a8ca336`:

- `crates/db/migrations/20261018180000_web_push.sql`
- `crates/db/migrations/20261020190000_kanban_lifecycle.sql`

They are used only in temporary databases by the LOCAL-11 migration compatibility
test, never registered in the production migrator. All shared migration files at
that commit were byte-identical to LOCAL-11's base. The test covers the union of
both migration sets, a fresh database and upgrades from either side. These are
compatibility evidence, not an integration of LOCAL-19's functionality.
