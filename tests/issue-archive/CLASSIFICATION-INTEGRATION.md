# 179・176/177とarchive/searchのsource合成

2026-10-26 04:51 JST開始。元HEAD `4cf38eda1636b936991dc2871b36f76db7de3aaa` はclean、自身build/testはidle。
現main `ed1dcba8730abc6440902333f766697753db2c81` は出版済179 `39b2fe9976efeed810bad80dc38d01e4123bc4c3` にWikiのみを追加した状態だった。これと固定176/177候補 `fab2f57695b4969e0e297d0974bad02a8203340d` (tree `335a8ad022ffabf72dda098ec00fea7ce2a53024`) だけをown branchへ通常mergeした。動く177/32 branchは参照・mergeしていない。main/他worktree、稼働環境、実データへの変更・archiveは行っていない。

## 競合解消・最小修正

- main merge `62b9ea4c`: `packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx` のimport競合。`useId`/`Textarea`/plain text編集とpreview、`matchesListSearch`/一覧検索をすべて保持。
- 固定候補merge `8cb64198`: `crates/db/src/models/mod.rs` では `text_search` と `issue_number`/`board_tag` を併存。`crates/server/src/remote_mcp/read.rs` はarchive/filter/cursorと一覧・詳細の `coalesce(priority,'medium')` 投影を合成。URL/revision、bounded no-filter SQL、shared matches、既存transport別pagingは維持。
- `crates/server/src/routes/local_remote/issue_archive.rs` はtest fixtureのProject keyのみ変更。複数Projectに一意keyを付ける。archive実処理・排他・transactionは元4cfと同一。
- `crates/server/src/remote_mcp/tests.rs` のmatrixをmedium検索とlegacy NULL投影に拡張。NULLは隔離fixtureでのみtriggerを外して注入し、migrationを書き換えない。
- `tests/issue-archive/isolated.mjs` はProject keyを明示し、設計メモの検索・下書き保持とMarkdown編集/preview/原文保存の相互作用を追加。
- 元classification・Project key・archive migrationは各元sourceと同一。LOCAL-4 Session/dispatch、Workspace作成時Issue guard、共通Workspace archiveも元4cfから変更なし。Issueのstatus分離、ON全子busy、OFF Issueのみ、新規link拒否、link revision ABAと既存WS継続を維持。

## 実行結果

動作sourceはcleanな `84424cc07e2828445645aaa7044fb2d20ae85486`、tree `b01cbb50620e7f354fc21b8b942cfb0441e81684`。以後の差分は本記録のみ。証拠は `.build/issue-archive-validation/classification-merge/` に別保存し、前回の結果・review_targetは上書きしていない。

| 検証 | 結果 |
| --- | --- |
| `cargo test -p server --lib issue_archive` | PASS 6。新schemaでstatus保持、OFF、全子busy拒否、ABA、新規link拒否、削除/DB故障境界 |
| 同生成済みserver試験バイナリーの `board_reads_use_real_schema_bound_pages_and_reject_mixed_targets` | PASS 1。4 archive指定×17 filter、medium/旧NULL、同名tag intersection、hidden、cursorとmetadata |
| `classification_mcp_priority_tags_receipts_scope_and_atomic_create` | PASS 1。priority/tag、receipt/scope、atomic create |
| `project_creation_rolls_back_key_metadata_and_statuses_on_seed_failure` | PASS 1。key/metadata/status/tag seed rollback |
| `project_key_tagged_create_rolls_back_issue_tags_and_number_together` | PASS 1。key採番とtag生成のtransaction |
| `remote_archive_http_confirmation_authorization_races_and_results` | PASS 1。署名JWT fixtureで確認/scope/競合/receiptと状態読取 |
| `pnpm run web-core:check` / `local-web:check` / `ui:check` | PASS 3pkg |
| scoped DesignNotePanel ESLint / Prettier、変更Rust rustfmt、node構文確認、`git diff --check` | PASS |
| `cargo build -p server --bin server --bin generate_types` / `target/debug/generate_types --check` | PASS。自動merge後の生成型整合も確認 |
| `LVK_TEST_OUTPUT=.build/issue-archive-validation/classification-merge node tests/issue-archive/isolated.mjs` | PASS 9項目。共通REST archive/filter/offset、3タブ×2表示、hidden/selection、確認/OFF、解除/作成拒否、receipt復旧、Project/resetに加え、設計メモ検索とraw Markdown編集/preview/保存を確認 |

実UI用serverは上記clean source、build metadataも同SHA/dirty=false。SHA-256 `820779488db079df58d6eda6422aba239cdcb9d5994f05018418d71eb4c8b5d6`。画像・build metadata・source hashを保持。backend/Vite/browser終了を待ちfixture DBを削除済み。前回33件全体や無関係suiteは再実行していない。

## 資源・残る検証

重コマンドはjobs2で順次、他の正式validationと合計2以内で実施。共有target化・cache削除・他process操作なし。sccacheの指定環境はcanonical共有pathだが、実サーバー保存先は `/home/appuser/.cache/sccache` と観測された。不一致を報告し、既存サーバーを停止/再設定していない。通常targetと依存は継続利用用に保持する。

独立レビューと正式full 5件は後段の上位担当で必須。こちらではClippy/SQLx全体/正式full、実認証・実モデル・稼働DB移行、crash全点や全入口並行barrierは再実行していない。最後のmain再同期は上位からの指示待ちとし、追加の未指定sourceは取り込まない。終了は05:03 JST頃、約12分（待機含む）。
