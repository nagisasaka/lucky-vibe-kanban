# SQLite upgrade cross-version fixture

This is an opt-in test harness, not a product compatibility layer. Build the same
Rust fixture against the old revision and the proposed revision. Copy
`sqlite_upgrade_fixture.rs` temporarily to `crates/db/tests/` in each checkout and
build its integration-test executable with the checkout's toolchain:

```sh
node scripts/dev-cargo.mjs test -p db --test sqlite_upgrade_fixture --no-run
```

Run the two resulting test executables through the driver:

```sh
python3 -B tests/sqlite-upgrade/run.py \
  --old /absolute/path/to/old-test-executable \
  --new /absolute/path/to/new-test-executable \
  --repo /absolute/path/to/proposed-repository
```

Remove the temporary copies of the Rust fixture after use. The driver creates and
cleans its own temporary instances and packages, including when a check fails.
Each process has a timeout. Neither instance cutover nor any running service is
invoked. The ignored Rust test also requires an explicit instance path and marker.

Checks use actual DBService startup/migrations and Project transfer APIs. They
cover SQLite 3.46.0/3.54.1 old/new/old reads and writes, UUID BLOBs, chrono dates,
JSON, NULL, issue/comment/version/tag relationships, integrity/foreign keys,
all-table snapshots including migration receipts, bidirectional export/import,
and the existing developer switch `backup_data` function followed by restored DB
reopen with both engines. Project import remaps identities, and deliberately
excludes `extension_metadata`; the fixture checks that existing contract instead
of expecting byte-identical imported databases.

This fixture does not prove compatibility after introducing new SQLite features,
new schema migrations, extensions, attachment payloads, or complete production
data. It does not test Docker delivery or operational cutover. Build and run
results must be reported separately; adding these files does not mean they passed.
