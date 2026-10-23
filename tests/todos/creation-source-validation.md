# TODO creation source — Change Manifest

## 調査（変更前）
初期main/HEAD: `aa59ab5855ae90b0fe3cf65a2017ff60be52df61`。以下の行はこのrevisionに固定。
- UI: `packages/web-core/src/features/todos/ui/TodoPanel.tsx:53` → `packages/web-core/src/features/todos/model/todoApi.ts:28` → `crates/server/src/routes/todos.rs:43`。
- stdio MCP: `crates/mcp/src/task_server/tools/todos.rs:80` → 同じREST POST。既存内部clientは `crates/mcp/src/task_server/mod.rs:249` でloopback用内部tokenを設定。
- HTTP MCP: `crates/server/src/remote_mcp/todos.rs:165` のreceipt処理 → `Todo::create`。
- 共有保存: `crates/db/src/models/todo.rs:143`、migration `crates/db/migrations/20261018000000_project_todos.sql`。get/listは同ファイル120/136行のSELECT *とTodo DTO。更新は185行で限定列のみ変更。
- 表示: `packages/web-core/src/features/todos/ui/TodoPanel.tsx:404`、カテゴリ内のTodoRowをProject/Workspace両方で共有。
- 信頼境界: `crates/server-access/src/lib.rs:629` で内部tokenを検証したPrincipal、その他はweb認証。678行でtokenを除去しACTOR/extensionへ渡す。

## 最小方針
- DB enum ui/mcp/unknown、既存行unknown、DB triggerで不変。入力DTOに属性を追加しない。
- 通常RESTはmiddlewareのlocal/basic/webのみui、他はunknown。stdioは内部認証限定の専用作成入口でmcp。HTTP MCPはサーバーdispatchでmcp。任意headerや本文のラベルは使用しない。
- trusted内部token所有者は内部入口を実行できる。これは既存のローカル能力境界であり、AI/人間・発案者を証明しない。
- 再試行は既存の内容比較を維持し、最初の作成経路を保持。分類/フィルター/権限体系は追加しない。

## 検証対象と環境

- 初期main/HEAD: `aa59ab5855ae90b0fe3cf65a2017ff60be52df61`。
- 製品実装・生成型・Rust/API試験・Web成果物: `e517630a6ba0c556dcb361f309260fc42886f38a`。DB試験と生成・frontend検査はこのcommitに収録された同一sourceをcommit前から検証。
- 最終ブラウザ試験スクリプト: `e5d1a159f86167218b61dc7222a5c050f6d7139e`。差分は非同期更新の待機だけで、Rust/frontendはe517630aと同一。バイナリはe517630aを再利用し、不要な再buildをしなかった。
- この記録の最終commit SHAはrepository memoryと `.build/todo-validation/result.json` に記録する（自分自身のSHAを文書内に埋め込まない）。
- ユーザー追加指示に従い、他Issueのmain更新をrebaseせず、main更新・デプロイ・他WSの変更/停止は行わない。独立レビューと正式Integrationは上位AIに引き継ぐ。
- 共有資源snapshotはresources/holders/queueすべて空。独自の一時 `LVK_INSTANCE_DIR`・DB・loopback動的portを使い、本番/既存dev serverへ接続しない。
- 全Rustコマンドは同じworkspace `target/`、`CARGO_BUILD_JOBS=2`、既定profile、`RUSTC_WRAPPER=/usr/local/bin/sccache`。`SCCACHE_DIR` は `.lvk-shared/cache/sccache` のcanonical絶対path。cargo clean、別target、compiler flag変更なし。
- sccacheの開始前表示はdefault cacheだったが、ビルド後のactual serverはcanonical shared cacheと一致。serverの手動停止/再設定や統計resetなし。統計は共有なのでこのtaskだけの実績とは扱わない。
- Chromium 147.0.7727.15は既設 `/opt/lvk-browsers` を使用。sqlx-cli 0.8.6はこのWSの `.build/todo-tools` に準備。

## 実行結果と所要時間

以下のRust/SQLxコマンドには上記の共通環境を適用。未計測の時間は推測しない。ログは `.build/todo-validation/` 配下。

| コマンド | 結果 | 所要時間・ログ |
| --- | --- | --- |
| `pnpm install --frozen-lockfile` | 成功 | 時間未計測 |
| `cargo test -p db todo --lib` | 5 passed。旧データ全列/順序/Issueリンク保持、unknown移行、3経路不変、再試行、revision、Project境界 | build 8m14s / tests 0.02s; `db-test.log` |
| `pnpm run generate-types` | 成功。Todo enum/属性だけの生成差分 | build 7m29s; `generate-types.log` |
| `pnpm run generate-types:check` | 成功 | build 24.57s; `generate-types-check.log` |
| `pnpm run web-core:check` | 成功 | 時間未計測; `web-core-check.log` |
| `pnpm --dir packages/local-web exec eslint ../web-core/src/features/todos/ui/TodoPanel.tsx --config .eslintrc.cjs` | 成功 | 時間未計測; `eslint.log` |
| `pnpm --dir packages/local-web run build` | tsc/Vite成功 | Vite 1m17s（tsc時間は別途未計測）; `web-build.log` |
| `cargo test -p server todo_mcp --lib` | 1 passed。外部MCP保存/単体・一覧読取/偽装拒否/編集後保持/scope/receipt/revision | build 7m26s / test 0.46s; `server-todo-test.log` |
| `cargo test -p server http_discovery_authentication_and_tool_call_roundtrip --lib` | 1 passed | build 20.99s / test 0.16s; `server-http-test.log` |
| `cargo build --bin server --bin agent-process-host --bin vibe-kanban-mcp` | 成功。既存依存成果物を再利用 | 25.11s; `binaries-build.log` |
| `node tests/todos/isolated.mjs` | 最終成功。隔離DB/server/stdio MCP/Chromium、UI作成、MCP作成、相互編集、旧schema migration、再読込、CRUD/競合、UI両テーマ/320・390・1000・1600px | 68.165s; `e2e.log` |
| `cargo install sqlx-cli --version 0.8.6 --locked --no-default-features --features sqlite --debug --target-dir target --root .build/todo-tools` | 成功 | build 19.97s; `sqlx-install.log` |
| `pnpm run prepare-db` | 成功。一時DBへ全migration適用。動的TODO queryの追加metadataなし | build/check 6m21s / 全体386.236s; `prepare-db.log` |
| `pnpm run prepare-db:check` | 成功（既存unused metadataの警告のみ） | check 5.62s / 全体9.116s; `prepare-db-check.log` |
| `node --check tests/todos/isolated.mjs` / `git diff --check` | 成功 | 時間未計測 |

長い処理は空のworkspace targetからの初回依存準備と、DB試験・server通常build・server試験・SQLx checkの依存feature/コンパイル方式の違いによるもの。例えばserver試験は `server-access/test-support` を有効化する。SQLxはcodegen済みbinaryの実行ではなくcargo checkを行う。別target/profileへの意図的な切替ではない。短いserver再buildは `crates/server/build.rs:8` の毎回Git識別情報更新によるもの。後続の3バイナリは25.11秒で再利用できた。

SQLx再生成は無関係な既存metadata14ファイル（例: coding_agent_turns）を削除したため、task開始時と同じ内容へ復元してからcheckした。既存metadataの整理をこの変更へ混ぜない。

## 失敗と修正、未実施

- 初回E2E（24.304s）は新設した `uncheck()` がReact非同期更新を即時判定して失敗。既存試験と同じclick後のexpect待機へ修正。
- 2回目E2E（29.769s）は新設したMCP更新待ち5秒とUIの5秒pollが競合。既存と同じ15秒timeoutへ修正。製品のpoll間隔・挙動は変えていない。
- 上記の失敗ログは `e2e-first-failure.log`、`e2e-second-failure.log` に保持。修正後の全隔離E2E成功を完了根拠とする。
- 補助コマンドの初期エラー: rootのprettier不在/相対path違い、web-core単独ESLint設定不在、`/usr/bin/time`不在。既存packageツール/configとbash timeに修正。これらを検証成功とは数えない。
- Web buildに既存Browserslist/Tailwind/chunk-size警告あり。今回の型エラー・browser runtime errorなし。
- 無関係な全体check/lint/backend:test、デプロイ、実Auth0/ChatGPT、Firefox/WebKit/実機は未実施。正式Integration検証と独立レビューは上位AIの担当。

## 成果物・後片付け・残る境界

- `screenshots/creation-sources.png`、`project-light.png`、`project-mobile.png` を実画像でも確認。経路表示は本文より小さく、両テーマとモバイルで既存操作を妨げない。その他layout/workspace画像も同ディレクトリ。
- E2Eのfinally完了後、このWSのserver/MCP/agent-process-host PIDと一時instanceディレクトリが残っていないことを確認。shared resource operationの待機/保持なし。
- `target/` とnode_modulesはレビュー・追試用に保持。`.build/todo-tools` 約146MiBもmetadata追試用、`.build/todo-validation` 約1.4MiBはログ/画像/結果として保持。他WS・共有cache・稼働成果物の削除なし。
- ディスクは本run開始時の実測55GiBから、完了時点で空き23.2GiB。RAM availableは重build中も約8.5GiB以上を観測。最終resource値は `result.json` にも記録。
- UIはWeb認証で到達するTODO REST経路を意味し、ブラウザーを操作した人や発案者の証明ではない。trusted内部token所有者は内部MCP入口を実行できる既存の能力境界を維持。
- 同じrevisionのserver/stdio MCPを組み合わせて検証した。旧stdio clientの汎用内部REST経路はunknown（MCPと推測しない）。Rust読取DTOは属性欠落時もunknownとして読める。作成/更新DTOと権限体系は不変。
