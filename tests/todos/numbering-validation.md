# TODOリストの不変番号: 実装・検証記録

2026-10-30、Workspace `9a9cc6f9-4750-4a22-8261-9902a1aad860`、branch
`vk/9a9c-todo-project-tod`。基点は `01ae4ed89eb3262c8d43d87ed966339864fd2f34`。
対象はKanbanの状態ではなくProjectのTODOリスト項目。番号の契約は
[README](README.md#project内の不変番号)を参照する。

## 実行結果

| コマンド | 結果・範囲 |
| --- | --- |
| `node scripts/dev-cargo.mjs test -p db todo --lib` | 8件成功。migrationの既存全属性・同時刻UUID順、32件の並行作成、削除後の非再利用、DB再接続、Project独立性、番号不変、UUID再送時counter保持。 |
| `node scripts/dev-cargo.mjs test -p server todo_mcp --lib` | 1件成功。実migration、scope、receipt、番号/UUID取得更新、不正形式・不存在・別Project、revision競合。 |
| `node scripts/dev-cargo.mjs clippy -p db -p mcp -p server --all-targets -- -D warnings` | 成功。 |
| `pnpm run web-core:check` / `pnpm run local-web:check` | 成功。 |
| `pnpm --dir packages/local-web exec eslint --config .eslintrc.cjs ../web-core/src/features/todos/ui/TodoPanel.tsx --max-warnings 0` | 成功。 |
| `pnpm --dir packages/local-web run build` | 成功。既存のBrowserslist・Tailwind・chunk size警告あり。 |
| `node scripts/dev-cargo.mjs build --bin server --bin agent-process-host --bin vibe-kanban-mcp` | 成功。今回のsourceから構築し、以下の隔離試験に使用。 |
| `node tests/todos/isolated.mjs` | 成功。実SQLite/API/stdio/Chromiumで番号表示・番号更新・UUID互換・拒否ケース、移行/再読込、UI編集、関連Issue、dark/light、320/390/1000/1600px、長文・配置回帰。agent実行なし。 |
| `pnpm run generate-types:check` / `pnpm run prepare-db:check` | 成功。生成型とSQLx metadataを新schemaと照合。一時DBへ全migrationを適用。 |
| `node scripts/dev-cargo.mjs test -p server http_discovery_authentication_and_tool_call_roundtrip --lib` | 1件成功。HTTP認証・discovery・tool呼出しの既存回帰。 |
| `node --check tests/todos/isolated.mjs` | 成功。E2E成功後のstdoutメッセージ訂正も構文検査済み。 |

初回DB試験は旧migration fixtureに番号列がなく失敗し、fixtureに新migrationを適用して修正した。
remote MCPの追加fixtureはProjectキー重複、その後Project許可不足で失敗した。
固有キーと明示的な接続許可を設定して、許可済みの別Projectでも項目所属の照合が失敗することを検証した。
初回の型検査は型生成の完了前で失敗、ESLintは設定の自動探索に失敗したが、生成完了後と設定明示後に成功した。

初回E2Eは番号表示・stdio参照後、過去の「統合 / 自動マージ」ボタンのlocatorでtimeoutした。
現行IntegrationPanelはそのボタンを返さないため、TODOと独立した古い確認ブロックを除去して再実行した。
成功後の差分は結果メッセージの訂正と、JSONの数値に合わせて `number` のts-rs型を `number` に明示する注釈・生成型の変更。
採番・migration・MCP・UIの実行時処理は変更していない。型生成/整合確認と両frontend型検査を再実行し、E2Eの重複実行は省略した。

## 試験成果物の識別

隔離E2Eに使用したファイルのSHA-256。checkoutのHEADだけをbinaryの識別根拠にしていない。

```text
server:             5fefa82d63486a2ec0b3e14e8abe90b80e24acc911aae0a7ac8fa8bc7038d6d9
agent-process-host: 2d5fc1f15664f4068435f60757bd10c0b2b3b03760badae9a283de58ef78a2df
vibe-kanban-mcp:    3ade5a880ec3796a13c86eff4c7e35dfe34491967cd74cfdae4d5fc04d74917c
dist/index.html:   add1712ce8eab7e830457da138569c074412fbc9560da6a5513553798cd03888
```

## 時刻と境界

主要ステージのUTC時刻（分単位）: 08:03開始、08:05主要実装、08:13 DB回帰成功・
試験binary構築、08:14 remote MCP回帰成功、08:16 Chromium E2E成功、08:19 Clippy成功、08:24 SQLx確認成功、08:25 HTTP MCP認証回帰成功、08:27 最終型検査成功。

live DB・他Workspace・他branch・既存プロセスを変更していない。稼働MCPへの操作、main merge、公開、
live再起動、deployは行っていない。OpenWikiは読取のみ。独立レビューはコーディネータが後続する。

実Auth0/ChatGPT接続、live DB移行、混在版のclient/server、Firefox/WebKit、実機モバイル、全backend suiteは未検証。
検証は今回の変更とその利用境界に絞った。新stdioの番号指定を旧serverで利用できるとは扱わない。
LOCAL-281との後続統合では共有Rust/MCP/生成型の競合とmigration version重複を確認し、
同じschema/APIに対応するclient/serverを使う必要がある。移行以前に削除されたTODOの番号履歴は復元しない。
