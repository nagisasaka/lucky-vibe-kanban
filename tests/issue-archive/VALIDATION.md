# LOCAL-181 実装・検証記録

開始: 2026-10-25 14:24 UTC。実装ベースは HEAD/main とも `271aa60f14dd1ac94b2cf2a9fe0e1878bef3d2ce`、開始時作業ツリーはclean。実装凍結・独立レビュー対象は `492459c3640216d82281df444ba977833c5c2487`。mainの変更、merge/cherry-pick/push、正式Integration、deploy、稼働サーバー再起動、実データarchive/deleteは行っていない。

## 実装の境界

- Issueの`archived`を独立追加。既存Issueはfalseで移行し、Workspaceの`archived`から推定しない。archive/解除はstatusを書き換えず、既存Workspaceの実行による自動status遷移も凍結しない。
- Active/Archived/AllとKanban/Listを独立化。両表示に同じhidden/検索/filterを適用し、選択・一括件数を可視集合へ制限。関連・履歴用の母集団とWorkspace検索は従来どおり保持する。
- 同ProjectのIssue詳細遷移はroute再mountを伴うため、対象/表示を非永続の画面storeに保持。Project切替・再読み込みではActive+Kanban。Scratch/localStorageへ保存しない。
- Issue確認はcheckboxと全リンク集合(ID/path/branch/既存archive状態)、link revisionを束縛。ONの全未archive対象を検査した後にだけ既存Workspace archive primitiveを実行する。OFFとIssue解除ではWorkspace/プロセス/worktreeへ影響しない。
- 排他順序はMUTATIONS→Issue→ID固定順Workspace lifecycle→BEGIN IMMEDIATE。初回作成はAgent/setup受付またはworkflow run/ready bindまでIssue guardを保持。DB triggerで全writerの新規linkを拒否し、unlink/relink ABAも再確認へ戻す。
- 通常の物理削除失敗はlogical archiveを確定し対象別errorを返す。物理処理後のDB失敗・commit不明はuncertainとして、rollback/削除完了を偽らず自動再実行しない。
- UI/stdio/remoteで共通受付と既存receiptを利用。MCP一覧は`archived`省略で全件、UIはActive既定。remoteのcursor/url/revisionとstdioのoffset/応答形式は保持する。

## 実行した検証

ログ・画像・実行時バイナリーSHA/metadata・ソースhash対応・独立レビューは、このWorkspaceの `.build/issue-archive-validation/` に保持する。Rustのtargetと依存は通常のWorkspace出力を再利用し、共有target/cacheの削除・再構成はしていない。Rust build/testはjobs 2で順次実行。後続SQLx確認はjobs 1・nice 10で他作業を優先した。

| コマンド                                                                                                       | 結果 / 対象                                                                                                                                                                                                                                                                                 |
| -------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `python3 tests/issue-archive/migration.py`                                                                     | PASS。移行default、status保持、Workspace archiveとの独立、新規link拒否。最小旧schema fixture。全migrationは下記Rust fixtureでも適用                                                                                                                                                         |
| `cargo test -p server --lib archive`                                                                           | PASS 25件。既存Workspaceの使用中/OS参照/履歴branch保持、0/1/複数対象、OFF、busy一件で全体拒否、確認後link ABA/branch変更、通常削除失敗、物理削除後DB故障、Issueのみ解除、署名JWT remote HTTP確認/権限/本人束縛/receipt/一覧filter                                                           |
| `target/debug/deps/server-4ecb5813dcfd5681 remote_mcp::operations::tests`                                      | PASS 2件。切断後継続、同一requestの単一実行、process instance変更後のuncertainと再実行拒否                                                                                                                                                                                                  |
| `cargo test -p db -p mcp issue_archive`                                                                        | PASS DB 1件、MCP 1件。実canonical eventからのRunning/Succeededでstatus自動遷移を継続しarchive保持。stdio adapterの確認token/request_id/REST path保持                                                                                                                                        |
| `target/debug/deps/mcp-29baaaa5e2aae9d5 stdio_codec`                                                           | PASS 1件。実byte-stream codecのcatalogに新stdio toolsを含み既存API呼出し互換を保持。ビルド追加なし                                                                                                                                                                                          |
| `target/debug/deps/server-4ecb5813dcfd5681 board_reads_use_real_schema_bound_pages_and_reject_mixed_targets`   | PASS 1件。既存remote bounded read/paging/target束縛の互換。ビルド追加なし                                                                                                                                                                                                                   |
| `cargo build -p server --bin server --bin generate_types -p mcp --bin vibe-kanban-mcp`                         | PASS。実UI用の隔離serverと型生成/MCPバイナリー                                                                                                                                                                                                                                              |
| `cargo run --bin generate_types` / `target/debug/generate_types --check`                                       | PASS。生成物を手編集せずRustから生成、整合確認                                                                                                                                                                                                                                              |
| `pnpm run web-core:check` / `pnpm run local-web:check` / `pnpm run ui:check`                                   | PASS。共有型・新画面store・変更コンポーネントとconsumers                                                                                                                                                                                                                                    |
| 変更web-coreへのlocal-web既存ESLint設定 / `pnpm run ui:lint`                                                   | PASS。web-coreの明示tsconfigを指定。型対象外の旧コンポーネントへの不要変更は除去                                                                                                                                                                                                            |
| scoped Prettier / `rustfmt --check --edition 2024 --config skip_children=true <変更Rust>` / `git diff --check` | PASS                                                                                                                                                                                                                                                                                        |
| `node tests/issue-archive/isolated.mjs`                                                                        | PASS。実API・専用instance/SQLite・動的loopback・Chromium。3タブ×2表示、hidden、tab/display独立、selection/search/bulk件数、確認毎回ON、OFF、detail/list解除、新規作成409、confirm応答消失→既存receipt取得(再confirmなし)、SPA Project切替/再読み込みreset、Cancelled表示設定、1440幅と390幅 |
| `CARGO_BUILD_JOBS=1 nice -n 10 pnpm run prepare-db:check`                                                      | PASS。全migration適用・query metadata整合。既存のpotentially unused queries警告あり、終了コード0                                                                                                                                                                                            |

初回Rust試験は新fixtureの必須sort_order不足で失敗し修正後に上記25件PASS。初期ブラウザーでdialog role不足とroute再mountによる対象resetを見つけ修正。続くハーネスの詳細/Workspaceメニュー混同、既存fallback同期を待たないAPI書換え、Projectボタンの複数一致、status更新URLの誤りも修正した。途中失敗はPASSに数えていない。最終実行はfinallyでbrowser/Vite/fixture backend終了を待ち、一時instanceを削除した。

## 独立レビュー

`/root/independent_review` が上記凍結SHAと周辺sourceをread-onlyで評価。実装編集、build/test、共有環境操作は行わず、実装担当の試験報告と自分の静的判断を区別した。blocking/highの具体的不具合はなし。軽微指摘のIssue guard保持期間コメントは実装どおりに修正した。元レビュー文のreview_targetは上書きしない。

## 未確認・限界

- 正式Integrationの全check/lint/backend:testは範囲外。追加の重いRust ClippyはLOCAL-180との資源協調を優先して未実行。
- 実Auth0/ChatGPTアカウント、稼働DBへの移行、実機画面、実モデルによる新規作業は試験していない。remoteは隔離JWT、stdio adapterは隔離mock HTTPと実codec、DB/物理処理は実fixtureを組み合わせた確認。
- 物理削除後DB故障はtrigger注入、再起動はreceiptのprocess instance不一致で検証。OSプロセスを実killしたcrash全点やcommit応答消失の全点を網羅した保証ではない。
- 初回作成とarchiveのguard境界はAPI拒否・DB trigger・独立静的レビューで確認。全入口の決定的並行barrier試験は未追加。既存OS参照観測のPID namespace/権限/観測後外部起動の限界も変えていない。

## 他Issueとの統合接点

- LOCAL-81: `21d9ff05` / `a067059a` のカード本文/Workspace集約・UI fixture。今回のID行メニュー・表示集合変更と共存させる。
- LOCAL-176: `290af7ad` のpriority/tag検証・remote read投影。同じIssue row/read領域で`archived`を落とさず、priority/tag追加も上書きしない。
- LOCAL-177: `08bd7369` の不変Project key・Issue番号/識別子生成。今回のmigration/Issue読取と統合する際に両者の属性を保持する。
- LOCAL-180: 別WS2631d7aeのWIP `issue_filter.rs` を読取のみ確認。後統合では `IssueFilters.archived` → `SearchIssuesRequest.archived` への代入 → 共通`matches`へ今回のarchive predicateを移す。remoteはその共通helperの結果をtransport固有cursor/paging/投影へ渡す。stdioとremoteの応答を同一化せず、url/revision/cursorを失わない。未commit案なので実際の統合時に再確認する。180機能の複製/merge/cherry-pickや本文変更はしていない。

最終差分は独立レビュー指摘のコメント修正と本記録のみ。検証済み実装から実行動作の変更はなく、重い試験は再実行していない。実ブラウザーのserverバイナリーは実装commit前の同等dirty sourceから生成されており、build metadataのrevisionだけでcommit対応を主張しない。実SHAと検証済みsource hashは保持した証拠で対応を確認できる。
