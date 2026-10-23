# TODO creation source — Change Manifest

## 調査（変更前）
初期main/HEAD: `aa59ab5855ae90b0fe3cf65a2017ff60be52df61`。以下の行はこのrevisionに固定。
- UI: `packages/web-core/src/features/todos/ui/TodoPanel.tsx:53` → `model/todoApi.ts:28` → `crates/server/src/routes/todos.rs:43`。
- stdio MCP: `crates/mcp/src/task_server/tools/todos.rs:80` → 同じREST POST。既存内部clientは `crates/mcp/src/task_server/mod.rs:249` でloopback用内部tokenを設定。
- HTTP MCP: `crates/server/src/remote_mcp/todos.rs:165` のreceipt処理 → `Todo::create`。
- 共有保存: `crates/db/src/models/todo.rs:143`、migration `crates/db/migrations/20261018000000_project_todos.sql`。get/listは同ファイル120/136行のSELECT *とTodo DTO。更新は185行で限定列のみ変更。
- 表示: `TodoPanel.tsx:404`、カテゴリ内のTodoRowをProject/Workspace両方で共有。
- 信頼境界: `crates/server-access/src/lib.rs:629` で内部tokenを検証したPrincipal、その他はweb認証。678行でtokenを除去しACTOR/extensionへ渡す。

## 最小方針
- DB enum ui/mcp/unknown、既存行unknown、DB triggerで不変。入力DTOに属性を追加しない。
- 通常RESTはmiddlewareのlocal/basic/webのみui、他はunknown。stdioは内部認証限定の専用作成入口でmcp。HTTP MCPはサーバーdispatchでmcp。任意headerや本文のラベルは使用しない。
- trusted内部token所有者は内部入口を実行できる。これは既存のローカル能力境界であり、AI/人間・発案者を証明しない。
- 再試行は既存の内容比較を維持し、最初の作成経路を保持。分類/フィルター/権限体系は追加しない。

## 検証計画
DBの旧データ移行・不変性・再試行、外部MCPのreceipt/読取と偽装拒否、実サーバー+stdio MCP+Chromiumの隔離E2Eを拡張。生成型/SQLx metadata確認、対象frontend型検査とbuild。Rustはjobs=2、対象テストと必要binaryのみ。無関係な全体gateは実行しない。
共有資源snapshotは空（resources/holders/queueなし）。専用一時instance、DB、動的portを使用し、自プロセスのみfinallyで停止する。
開始時RAM available 13GiB、swap空き4.5GiB、ディスク空き55GiB。結果と最終SHAは完了時追記。

## ビルド環境の観測
- sccache 0.18.0は存在するが、既存serverの実cacheは `/home/appuser/.cache/sccache`。指定canonical cacheとの差を記録し、共有serverの停止・再設定は行わない。
- `pnpm install --frozen-lockfile` 成功。初回 `pnpm exec prettier` はrootに未配置で失敗、web-coreのprettierへ修正。最初の相対パスも修正済み。
- sqlx-cliは未インストール。metadata検査の前提として用意する。TODOのクエリは動的queryであり新規query! metadataは生じない。

- 対応するserver/stdio MCP binaryは同じrevisionを使用する。旧stdio clientの汎用内部REST経路は識別不能なのでunknown（MCPだと推測しない）。既存の作成・更新DTOは不変、読取は属性の追加のみ。旧応答を読むRust DTOは欠落属性をunknownとして扱う。
- Chromium 147.0.7727.15は既設 `/opt/lvk-browsers` から起動成功。不要なinstall待機は自プロセスのみ停止し、既設browserを変更しない。
- web-core単独ESLintは設定不在で失敗。`pnpm --dir packages/local-web exec eslint ../web-core/src/features/todos/ui/TodoPanel.tsx --config .eslintrc.cjs` は成功。

- ビルド開始後のsccache再観測では、actual cacheが指定canonical shared pathになっていることを確認。共有serverを手動停止・再設定する操作はしていない。
- `CARGO_BUILD_JOBS=2 RUSTC_WRAPPER=/usr/local/bin/sccache SCCACHE_DIR=<canonical cache> cargo test -p db todo --lib`: 5 passed（初回依存build 8m14s、試験0.02s）。
