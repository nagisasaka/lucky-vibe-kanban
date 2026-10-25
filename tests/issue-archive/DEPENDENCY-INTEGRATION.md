# LOCAL-180 / LOCAL-181 依存統合の局所検証

2026-10-26 04:21 JST開始。自身sourceは `486b0d955b47be62680fa1ffcbc2db332d3343ca`、cleanかつ自身のbuild/test/backendはidleで着手。
指定されたmain `3c0585a56d22b7f3ccad989212e44aecc98cf7a0` と、180 source `28b54be4960cd0aacadd9b7ea361b058b9f31f61` だけを自身branchへ通常mergeした。元181、180、mainのancestryを維持し、main/他worktreeの変更、rebase/rewrite、push、正式Integration、deploy、稼働環境の操作は行わない。

## mergeと判断

- main merge `78863ae959e042d7679fc26b47a6080e8ad09b4e` は競合なし。LOCAL-81のカード本文/Workspace集約と181の対象集合/操作導線を両方保持。LOCAL-4 Session/dispatch guardのsourceは改変していない。
- 180 merge `ad7dd4a131bb3dc6a5c60849755090e3fde95814` の競合は3ファイル。`crates/mcp/src/task_server/tools/remote_issues.rs` は180の共通filter受付を残し、181のarchive操作/状態投影/adapter試験を保持。廃止済みstdio固有filterとその旧unit testは戻さない。
- `crates/server/src/remote_mcp/read.rs` は180の共有filter後UUID cursor処理と既存metadataを維持し、181のarchive状態投影を合成。archive条件だけの場合のbounded SQL fast pathにも省略時の未archive条件を適用する。
- `crates/server/src/routes/local_remote/board_api/mod.rs` は180の共有`matches`呼出しを残し、181のarchive predicateを `board_api/issue_filter.rs` の`matches`へ移す。
- 共通 `api-types/src/issue_filter.rs` へ `archived` を追加し、`q.archived`へ代入する。新合成仕様では省略/falseが未archive、trueがarchive済、`"all"`が両方。既存boolean入力は保持し、以前の181の「省略は全件」はこのMCP一覧契約に限り置き換える。内部board一覧/関連読取の母集団は変更しない。
- 検索・ID・status・priority・parent・assignee・tagはarchiveとAND。tag IDと名前は同じtagへのintersection。stdioはsort/offset/count、外部はUUID cursor/limit/URL/revision/観測metadataを維持する。UIのhidden設定でMCP読取を削らない。
- Issue archive受付、OFF/解除のIssue限定、ONの都度確認/全子busy preflight、既存WS継続/新規link拒否、MUTATIONS→Issue guard→固定順WS lifecycle→BEGIN IMMEDIATE、DB trigger/link revision ABA保護は変更していない。

## 局所検証と証拠

動作source凍結は `0dba921a`、tree `7f7f04885436a1c60fe96381f21a9932510bbccc`。実行結果、ソースhash、画像は `.build/issue-archive-validation/` の `merge-*` と `dependency-merge/` に保存する。以前の181の証拠・レビュー対象は上書きしない。

- `pnpm run web-core:check`、`pnpm run local-web:check`、`pnpm run ui:check`: PASS。
- 変更Rustへのscoped rustfmt、`node --check tests/issue-archive/isolated.mjs`、`git diff --check`: PASS。
- `cargo test -p server -p mcp -p api-types archive`: PASS (api-types 1、stdio MCP 2、server 25)。0/1/複数対象、busy全体拒否、OFF、ABA/branch変更、通常削除失敗/物理処理後DB失敗、scope/receipt/再実行拒否を含む。
- 再生成済み `target/debug/deps/server-ca8556db2c6e6caa` の `board_reads_use_real_schema_bound_pages_and_reject_mixed_targets`: PASS 1。4 archive指定×16 filter、共有helperと外部cursorを比較。同名tagとID/name intersection、hiddenをMCPから除外しないこと、limit1疎な一致/全件paging、URL/revision/metadataを検証。
- 同バイナリーの `search_tool_schemas_preserve_external_paging`: PASS 1、`remote_mcp::operations::tests`: PASS 2。既存cursor schemaとrequest単一実行/切断後継続/再起動後uncertainを確認。
- `target/debug/deps/mcp-9d803b30b982aa21 stdio_codec`: PASS 1。既存byte-stream codec/catalog確認。
- stdio追加adapter試験では省略/true/allを共通RESTへ渡し、searchの空白・tag ID/name・sort/offset/limitを保持、total_count/returned_count/offset応答を保持することを検証。
- `LVK_TEST_OUTPUT=.build/issue-archive-validation/dependency-merge node tests/issue-archive/isolated.mjs`: PASS (8項目)。実APIのarchive条件/検索/ID/status/offset/count、3タブ×2表示、hidden/selection/bulk、ON初期値/OFF、status保持、新規作成拒否、詳細/一覧から解除、応答消失→receipt照会(再confirmなし)、Project/reload reset。1440幅dark/390幅list画像を保持。fixture backend/Vite/browserの終了待ちとDB削除まで完了。
- ブラウザーのserverは今回のCargo testが生成した `target/debug/server` を使用し追加buildを回避。実SHA-256は `d48d0d52ab770d256ae14393f42cdc0f1bd0bd8a81fec7a11218022bca9aca21`、build metadata/source対応は `dependency-merge/revision.json` と `source.json`。以前のバイナリーと混同しない。
- 最後の変更は本記録のみ。検証済み動作sourceからの差分を確認し、文書追加のためだけのbuild/testは繰り返さない。

共有sccacheの既存canonical pathとWorkspace targetを使用。重コマンドはjobs2。177の処理終了が観測できるまでコード/軽検査を先行した。局所Rust開始直後、179正式full開始の追加連絡を受けたため次の重いジョブは待機し、他processの停止/変更は行わない。

## 上位への境界

独立レビューと正式Integration/full validationは上位担当。こちらの局所試験は正式full成功を代替しない。177/32/179等の進行中の未指定sourceを取り込んでいない。上位がmain再同期を必要と判断した場合は、その追加差分と競合に応じて検証を計画する。実運用archive、live DB、実アカウント、稼働preview/process操作は対象外。

終了: 2026-10-26 04:36 JST頃。局所検証は上記sourceで通過し、新しい重いジョブの予定はない。SQLは動的query/既存schemaのfilter条件だけで新規migration/SQLx macro/type生成登録の変更なし。今回SQLx全チェック・Clippy・無関係な全件試験は再実行していない。実認証/実モデル/live DB移行/crash全点/決定的な全入口並行barrierは未確認。新規archive安全境界の再設計はせず既存局所回帰を確認した。
