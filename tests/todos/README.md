# TODO regression checks

The DB tests cover text preservation, explicit completion, concurrent revisions,
Project/card boundaries, pagination and creation retries. External MCP tests use
the real migrated schema and operation receipts, including read/write scopes.

```sh
cargo test -p db todo --lib
cargo test -p server todo_mcp --lib
cargo test -p server http_discovery_authentication_and_tool_call_roundtrip --lib
```

The browser check starts its own loopback server with a fresh `LVK_INSTANCE_DIR`
and automatically assigned ports. It never uses a running instance, real Project
or agent execution. It checks the real DB/API and stdio MCP alongside the UI.
Processes and temporary instance state are removed in `finally`; screenshots
and logs remain in `.build/todo-validation/`.

```sh
pnpm --dir packages/local-web run build
cargo build --bin server --bin agent-process-host --bin vibe-kanban-mcp
node tests/todos/isolated.mjs
```

Use `LVK_CHROMIUM_EXECUTABLE=/absolute/path/to/chromium` to use an installed
Chromium instead of Playwright's default browser. This does not test a live
ChatGPT account or external OAuth provider.

The same isolated run also calls `layout.mjs`: it adds 24 mixed-height categories
(including a 30-line item and unbroken text), checks that a short column is filled
before the tall first category ends, and checks pairwise card bounds, category
order and horizontal overflow at 1600/1000/390/320px. It opens and resizes an edit
form, saves multiline content, checks Tab order, reaches the last category at
each width, opens deletion confirmation and deletes a category's last item.
The existing run also covers completion/reopening, creation, dark/light themes,
reload and the single-column workspace panel. These checks use the real isolated
API/SQLite and built frontend, with no browser route mocks.

Layout uses sparse CSS Grid placement with measured row spans. Category sorting,
item order and DOM/Tab order remain unchanged; categories are read top-to-bottom,
left-to-right when their tops align. Dense backfilling and column-major CSS
columns are intentionally avoided because they can change that reading order.
There is no category-collapse control; expansion coverage uses the existing edit
form and deletion confirmation.

Creation-source coverage is recorded in [the change manifest](creation-source-validation.md).
The isolated run migrates populated pre-feature TODO rows in its own stopped
instance, then checks `unknown`, UI creation, authenticated stdio MCP creation,
read APIs, rejected provenance overrides, and both interfaces editing all three
sources. The internal creation endpoint requires the existing local-agent token;
ordinary internal REST calls remain `unknown`. This records the entry route,
not an author identity. The HTTP MCP receipt test covers the other transport.
