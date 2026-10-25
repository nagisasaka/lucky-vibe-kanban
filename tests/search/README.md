# LOCAL-180 search regression

MCP preserves literal surrounding whitespace; browser search trims it. Both use
case-insensitive substrings over separate fields, without normalisation,
patterns or tokenisation. TODO searches body/category; design notes search
current title/Markdown body (not additions). Issue filtering shares one server
helper while stdio retains sort/offset/count and HTTP MCP retains UUID cursors
and metadata.

Run the targeted Rust tests sequentially with `CARGO_BUILD_JOBS=2`:

```sh
cargo test -p db search --lib
cargo test -p server board_reads_use_real_schema_bound_pages_and_reject_mixed_targets --lib
cargo test -p server routes::local_remote::board_api::tests --lib
cargo test -p server todo_mcp --lib
cargo test -p server design_note_mcp --lib
cargo test -p server http_discovery_authentication_and_tool_call_roundtrip --lib
```

The board regression uses the real migrated in-memory schema, sparse matches,
full titles past 500 characters, duplicate status/tag names, combined filters,
and pagination. It also assigns both tags A/B to one Issue and checks that
`tag_id=A, tag_name=B` returns zero while the A/A pair matches in both the shared
helper and HTTP MCP projection; the isolated test repeats this through real stdio.
It preserves the old response assertions and enables SQLite
`query_only` while exercising filters, including a Project without statuses.

The browser/stdio regression starts a fresh instance with this checkout's built
binaries and Vite sources. It uses no running dev instance or production data.
No coding agent is started. Browser, MCP, Vite and server stop in `finally` and
the temporary instance is removed. Evidence remains in
`.build/search-validation/` (source/build identity, logs, results, screenshots).

```sh
cargo build --bin server --bin agent-process-host --bin vibe-kanban-mcp
LVK_TEST_SERVER="$PWD/target/debug/server" node tests/search/isolated.mjs
pnpm --filter @vibe/web-core exec vitest run src/shared/lib/listSearch.test.ts
```

This tests the real app locally. It does not validate real OAuth/ChatGPT accounts
or deployed UI. The unchanged Kanban hook is outside the new browser helper.
