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
