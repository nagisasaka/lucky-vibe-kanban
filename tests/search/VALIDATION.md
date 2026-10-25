# LOCAL-180 実装検証記録

2026-10-25 UTC。着手時のWorkspace HEADとローカルmainはともに
`271aa60f14dd1ac94b2cf2a9fe0e1878bef3d2ce`。
対象仕様はIssue revision `2026-10-25 14:21:30.016`。
検証はそのHEADに本変更を加えた未commitのソースで実施した。
隔離用バイナリの埋込みrevisionは同HEAD、dirty=true。
後続のsource commitとバイナリ埋込みrevisionを混同しない。

## 変更の対応

- `crates/api-types/src/issue_filter.rs`：両MCP用の検索条件と内部REST入力。
- `crates/server/src/routes/local_remote/board_api/issue_filter.rs`：状態・タグ名の解決と全条件の共通filter。`filter_project_issues(pool, project_id, &IssueFilters)` は読み取り専用。`board_api/issues.rs` の `/issues/filter` をstdioが呼び、外部MCPは同helperを直接呼ぶ。並び替え・offsetは既存処理に委譲する。
- `crates/server/src/remote_mcp/read.rs`：外部の引数追加。UUID cursor・上限・metadata projection・URL・revision・観測時刻など既存応答を維持。引数省略時は従来のbounded SQL経路を維持。
- `crates/mcp/src/task_server/tools/remote_issues.rs`：重複した条件解釈を削除し共通経路へ。stdio公開のページング・並び替え・応答は維持。
- `board_api/mod.rs`：一覧の既定状態INSERTを除き、既存状態だけ読む。既存board検索predicateを共用。
- `crates/db/src/models/{text_search,todo,design_note_read}.rs`：メモリ上の文字列照合。TODO・設計メモは検索一致後にページを切る。検索省略時の既存SQL経路を維持。
- `crates/mcp/src/task_server/tools/{todos,design_notes}.rs`、`crates/server/src/remote_mcp/{todos,design_notes}.rs`：任意searchと説明追加。
- `packages/web-core/src/features/{todos/ui/TodoPanel,design-notes/ui/DesignNotePanel}.tsx`：取得済みデータだけ検索。設計メモの選択・編集中stateは変更しない。編集欄の実装は変更しない。
- `packages/web-core/src/shared/lib/listSearch*`、en/ja common.json：ブラウザ用のtrim付き検索とTODOラベル。
- `docs/development/chatgpt-mcp.md`、`docs/integrations/vibe-kanban-mcp-server.mdx`：追加引数と検索・ページングの契約を記載。
- DB/serverの既存試験ファイル、`tests/search/isolated.mjs`：旧応答のassertionを保ち回帰確認を追加。

## 実行した検証

Rustは全て `CARGO_BUILD_JOBS=2`、既存profileとWorkspace内targetを使用し順次実行。
`RUSTC_WRAPPER=/usr/local/bin/sccache`、SCCACHE_DIRは共有cacheのcanonical pathを使用。
他WorkspaceのRust処理を検知した途中の2回は自分のCargoだけを中断し、181終了確認後に再開した。
以下は再開後の最終結果である。

| コマンド | 結果 |
| --- | --- |
| `pnpm install --frozen-lockfile` | 成功 |
| `pnpm --filter @vibe/web-core exec vitest run src/shared/lib/listSearch.test.ts` | 9件成功 |
| `pnpm run web-core:check` / `pnpm run local-web:check` | 成功 |
| `pnpm run local-web:lint` | 成功 |
| local-webのESLint設定を使った変更UI/helperのscoped lint | 成功 |
| 変更Rustファイルのrustfmt、変更frontend/scriptのPrettier、legacy frontend path guard、unused-i18n-key check、`git diff --check` | 成功 |
| `cargo check -p server -p mcp` | 成功 |
| `cargo test -p db search --lib` | 3件成功 |
| `cargo test -p db todo --lib` | 6件成功 |
| `cargo test -p db design_note --lib` | 9件成功 |
| `cargo test -p server board_reads_use_real_schema_bound_pages_and_reject_mixed_targets --lib` | 1件成功 |
| `cargo test -p server routes::local_remote::board_api::tests --lib` | 3件成功 |
| `cargo test -p server search_tool_schemas_preserve_external_paging --lib` | 1件成功 |
| `cargo test -p server todo_mcp --lib` | 1件成功 |
| `cargo test -p server design_note_mcp --lib` | 3件成功 |
| `cargo test -p server http_discovery_authentication_and_tool_call_roundtrip --lib` | 1件成功 |
| `cargo build --bin server --bin agent-process-host --bin vibe-kanban-mcp` | 成功 |
| `cargo clippy -p server -p mcp -p db -p api-types --all-targets -- -D warnings` | 失敗：変更外の `crates/server/src/remote_mcp/environment_switch_tests.rs:101` に既存のcollapsible_if警告。無関係な修正を避け維持 |
| `PATH="$PATH:/home/appuser/.cargo/bin" pnpm run prepare-db:check` | 成功。一時DBを削除済み。既存.sqlxの未使用候補warningあり |
| `LVK_TEST_SERVER="$PWD/target/debug/server" node tests/search/isolated.mjs` | 実stdio + 実ブラウザの検索・保持確認に成功。終了時に全fixture processを停止し一時instance削除 |

初回cargo checkは新handlerのDeployment trait import不足で失敗し修正後成功。
初回DB search試験は空白を含む本文への期待値が誤っていたため修正後成功。
web-core単独ESLintは設定なし、testファイルのlocal-web ESLintはtsconfig対象外で実行できず、
既存local-web設定でUI/helperをlint、testはVitestで確認した。
rootの`pnpm exec prettier`は依存がないため失敗し、local-webのPrettierで成功。
これらの失敗を最終成功と取り違えない。

## 受け入れ条件との対応

| 条件 | 確認済みの証拠 |
| --- | --- |
| 両MCPで同条件の同Issue集合 | server real-schema fixtureで全filter・組合せの期待集合を共通helperと外部MCPで比較。実stdioから同helperを呼び、検索・状態・offset/countとタグA/B不一致・A/A一致を確認 |
| 省略時の既存契約 | 元の外部105件fixture・上限/次cursor・metadata/revision/URL等のassertionを維持。stdioのsort/offset/count/limit0を既存unit + 実stdioで確認 |
| TODO search | DB/外部MCP/実stdioで本文またはカテゴリ、完了条件、空白・case・ページングを確認 |
| 設計メモ search | DB/外部MCP/実stdioで現タイトル/現Markdown本文、追記対象外、metadata応答、cursorを確認 |
| ページング前の絞り込み | 疎なIssue一致をpage size1で取得、非一致cursorも許容。TODOは一致結果のoffset、設計メモは一致結果のhas_more/最後の一致cursorを確認 |
| DB書き込みなし | Issue real-schema fixtureでPRAGMA query_only=ON下のhelper/外部一覧、total_changes不変、状態0件Projectで既定状態が増えないことを確認。stdioの状態一覧もINSERTを呼ばないコード経路へ変更 |
| UI検索 | 隔離ChromiumでProject/Workspace両TODOの本文/カテゴリ検索、既存完了表示条件、trim/case、検索API要求なし。設計メモのタイトル/本文検索 |
| 設計メモ選択保持 | 一覧から選択メモを外して詳細が残ること、編集中入力が検索/解除後も残ることを隔離Chromiumで確認 |
| 権限・revision | 既存混在/外Project cursor拒否とTODO/設計メモのscope/revision回帰、HTTP認証roundtrip成功 |
| カンバン非変更 | hook差分なし。実ブラウザでタイトル/simple_id/番号、trim/caseと本文対象外を確認 |

画面はheadless Chromium上の実アプリで確認した。モックだけの確認ではない。
ただし公開preview・実Auth0/ChatGPT接続・稼働dev・本番データは未使用。
Workspace TODO検索はデスクトップ幅で確認。モバイルは横はみ出し確認のみで、既存mobile画面のTODO表示対応を追加したとは扱わない。
画像確認はTODO Project・設計メモ編集中・Workspace TODOを対象とする。

## 判断・残る限界

- MCPは空白を保持し、UIはtrimする仕様を維持。TS/Rustの全Unicodeの厳密同一化は行わない。
- tag_idとtag_nameは同じタグのintersection。Issueに異なるA/Bタグが両方付いていてもA/Bは0件。削除した旧unit testの意味を共通helper/外部/実stdio試験に移した。
- LOCAL-181へ共通helper/APIとworktreeを共有済み。外部の検索条件省略fast pathはhelperを通らないため、archive既定除外を加える際は呼出側にも反映が必要とコメント済み。
- 独立diffレビューは管理側の後続工程。正式Integration・全workspace test・qa-mode全体・配布buildは未実施。今回の変更範囲の検証に限定し、Clippy全体が成功したとは扱わない。
- isolated UIのViteでBrowserslist旧データ/Tailwind既存設定warningとproxy再接続ログが出たが、assertion・pageerrorなし・画像確認は成功。
- `.build/search-validation/` に各コマンドlog、結果JSON、build identity、binary SHA-256、画像を保持する。通常のtargetとnode_modulesは継続開発用に維持。一時DB/instance・fixture repoは削除済み。
- mainへのmerge、正式Integration、deploy、既存preview/稼働devの停止・再起動、他WS変更、Issue本文/状態/TODO変更は行っていない。
