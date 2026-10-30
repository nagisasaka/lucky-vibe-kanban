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

Long-body checks in `body-expansion.mjs` verify four rendered lines, independent
expansion/collapse, full-text single-click editing, save/cancel, selection,
completion/menu isolation, viewport and width changes, keyboard and touch.
The late-font check uses the installed **Liberation Mono** font. It loads a new
FontFace after rendering to exercise wrapping changes at the same clamped height.
The tall category in the layout check is explicitly expanded to retain its
original packing regression.

For frontend-only work, reuse API/schema-compatible binaries read-only and serve
this checkout through Vite, without a Rust or frontend production build:

```sh
LVK_TODO_BIN_DIR=/absolute/path/to/compatible/target/debug \
LVK_TODO_VITE=1 \
LVK_CHROMIUM_EXECUTABLE=/absolute/path/to/chrome \
node tests/todos/isolated.mjs
```

Vite uses a dynamic loopback port and proxies only to the test-owned backend.
Both close in `finally`. Record the binary build SHA and compare the relevant
API/schema with the checkout before reusing artifacts. See
[the long-body Change Manifest](body-expansion-validation.md) for this run.

## Project内の不変番号

TODOリスト項目はUUIDに加えて `number` と `display_id`（`TODO-1` 等）を返す。
表示・参照には文字列の `display_id` を使用する。Issue番号とは独立し、本文・カテゴリー・完了・関連Issueの変更では番号を変えない。
Projectごとの永続カウンタをTODO挿入と同じSQLite transactionで増やす。
UUIDによる作成再送ではカウンタ更新をrollbackする。削除による番号の再利用はない。
DBのunique indexとtriggerでProject内の重複・番号変更を拒否する。
既存行は `created_at` 昇順、同時刻はUUIDのBLOB昇順で採番し、既存属性を保持する。
過去に削除済みの行は復元できないため、移行時の初期カウンタは現存行の採番結果に基づく。

stdio / remote MCPの `get_todo`・`update_todo`・`delete_todo` は既存の
`todo_id` にUUIDまたは `TODO-N` を渡せる。`project_id` と `expected_revision`
の契約は維持する。`TODO-0`・先頭ゼロ・小文字・空白・整数overflowは拒否する。
番号が同じでも別Projectの項目は別identityであり、番号だけで全Projectを検索しない。
同じ版のclient/serverを使う必要があり、新stdioから旧serverへの番号指定は保証しない。

DB回帰は8接続から32件の同時作成、末尾・全件削除後、DB再接続、Project独立性、
移行の同時刻タイブレークと全属性保持を検査する。remote MCP回帰はscope・receipt・
番号/UUIDの取得更新・競合・不正形式・不存在・別Projectを検査する。
`isolated.mjs` は同じsourceで構築したserver/stdioと実Chromium UIで番号表示と操作を検査する。
live DBや既存MCP接続は使用しない。

今回の実行結果・失敗修正・未検証範囲は[不変番号の検証記録](numbering-validation.md)を参照。
