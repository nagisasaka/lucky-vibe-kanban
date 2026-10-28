# Kanbanドロップ保存の調査・検証

対象Workspace: `d4c96fb1-b0c6-4381-8dd4-552dc35d1bae`。調査基点は調査開始時のlocal mainと一致する`a3247dadd5b4385e07a7848fd0009aa07d8ffa71`。mainマージ・デプロイなし。

## 原因

1. `KanbanContainer.tsx`旧実装はbulk保存応答の500ms後に同期保護を解除し、手元の同期配列から表示を再構築した。保存成功と同期到着は別である。現行の`shared/lib/electric/collections.ts`はElectricという名前を残すがRESTスナップショットを30秒間隔で取得している。bulk API直接呼出しは通常mutationの再取得を通らない。500msでは同期を保証できない。
2. 実ブラウザで小数順位`1.5`を送信するとHTTP 422、`updates[0]: invalid type: map, expected f64`。Rustの`BulkUpdateIssueItem`が`UpdateIssueRequest`を`serde(flatten)`し、arbitrary-precision JSON小数の中間表現がf64に渡る。整数順位では成功する。既存の競合テストは整数`99`を使い、この不具合を検出していなかった。
3. 順位競合はHTTPエラーではない。`update_local_issue_with_order_guard`は`expected_sort_order`不一致時に順位変更だけを捨て、成功応答を返す。列間移動はstatus変更を適用し、activityを移動先先頭へ持ち越す。「競合拒否がcatchに入る」という当初仮説は否定。従来UIはこの成功応答内の調整も保存エラーもユーザーに通知しなかった。
4. 連続ドロップは単一booleanと各応答の500ms timerを共有し、旧応答が後続操作の同期保護を解除できた。同期未到着で同一カードを再操作すると古い順位を次の期待値に使う問題もあった。

5. 最終再実行の隔離DBで、通常保存が1回HTTP 500 / SQLite code 5 `database is locked`になった。相手writerは未特定。`update_local_issue_with_order_guard`は読取後に書き込むtransactionであり、DB書込競合の別経路として要調査。画面は保存失敗alertを出して再同期した。小数JSON修正と別で、DB transaction/再試行の方針は変更していない。

根拠の位置: 基点`a3247dad`の`KanbanContainer.tsx:506`（同期配列から再構築）、`:740`（期待順位）、`:756`（500ms解除）。現行`packages/web-core/src/shared/lib/electric/collections.ts:53`と`:360`（30秒pollと通常mutation再取得）、`crates/server/src/routes/local_remote.rs:113`（bulk入力修正）、`:928`（変更していない順位競合判定）、`:3584`（小数入力回帰）。修正版の同期照合は`kanbanDropSync.ts:125`、最新カードIDからの移動元解決は`KanbanContainer.tsx:745`。

## 変更

- `KanbanContainer.tsx` / `kanbanDropSync.ts`: 保存を直列化し、各カードの最新操作を保持。保存後に取得した確定スナップショットのrevisionが表示用同期へ到着するまで楽観表示を維持。旧応答で新操作を解除しない。同一updated_atではstatus/rankの一致も必要（activity promotionはupdated_atを変えないため）。再同期も開始前に保存済みだった操作だけを確認する。画面を離れても受付済み保存を破棄しない。
- 移動元は古いDnD列/indexではなくカードIDと最新の表示配列から特定。関連241の指摘に対応し、リロードなしの列間移動→移動先並べ替え→再移動を実画面で検証。
- 保存失敗・競合調整・同期失敗/15秒タイムアウトを別のalertで表示し、再同期・閉じる操作を提供。保存・確認読取をそれぞれ15秒で制限。保存要求の再送や強制上書きはしない。
- `remoteApi.ts`: bulk保存のAbortSignalを受け取る。EN/JA common文言を追加。
- `local_remote.rs`: bulk itemをJSON Valueから直接デシリアライズすることで、既存の小数f64入力契約を修復。競合条件・更新処理・スキーマ・公開API形式は不変。
- DnDアニメーション・センサー・衝突判定・rank計算・列件数の描画処理は変更しない。

## 再現と検証手段

```sh
node --test tests/kanban/{order,drop-sync}.test.mjs
node scripts/dev-cargo.mjs test -p server bulk_issue_decimal_ranks_deserialize_from_http_json --lib
LVK_DND_REGRESSION=1 LVK_TEST_SERVER="$PWD/target/debug/server" node tests/kanban/isolated.mjs
```

旧500ms挙動は`LVK_DND_BASELINE=1`を追加して再現する。このモードだけViteのload hookで基点commitのKanbanContainerを読み込み、ソースファイル自体は書き換えない。空のDoneへの整数順位ドロップで、保存済みstatusはDoneだがDOMは元列へ戻ることを確認する。旧小数422のログは`.build/drop-run.log`。

ブラウザは実アプリをChromiumで操作し、新規の隔離SQLite、動的loopbackポートのbackend/Viteを使う。既存main/DB/previewへ書き込まない。遅延・エラーだけをPlaywright経路で注入し、正常な保存とactivity promotionは実APIを使う。Running表示はsummary fixtureであり、実provider実行の証拠ではない。通信遅延は900ms保存＋1100ms確認読取、タイムアウトは17秒確認読取。ブラウザの全ネットワーク帯域制限とは異なる。

## 実施結果

- `pnpm run web-core:check` / `pnpm run local-web:check`: PASS。
- 変更したTS/TSX 3ファイルへのlocal-web ESLint、既存Prettier設定、`git diff --check`: PASS。
- `node --test tests/kanban/{order,drop-sync}.test.mjs`: 15件PASS。同期遅延、同一時刻の古いrank/status、旧応答と新操作、競合、保存/確認失敗、再同期の競合、削除、画面離脱時の保存継続を含む。
- Rustの小数入力回帰1件: PASS。生成されたserver lib test binaryを`issue_activity_ --nocapture`で実行し既存のactivity/競合回帰3件もPASS。
- `node scripts/dev-cargo.mjs fmt --check -p server`: PASS。
- `LVK_BUILD_REVISION=a3247dadd5b4385e07a7848fd0009aa07d8ffa71 LVK_BUILD_DIRTY=true LVK_BUILD_KIND=development node scripts/dev-cargo.mjs clippy -p server --lib -- -D warnings`: PASS。
- 同じ明示build identityで`node scripts/dev-cargo.mjs build -p server --bin server`: PASS。Rust全体ビルド/全体テストは行っていない。
- 旧UI baseline Chromium: PASS（不具合の再現を期待する試験）。最終ログ`.build/drop-baseline.log`、証跡`.build/kanban-drop-baseline/`。

最終Chromium: PASS / exit 0。上記`LVK_DND_REGRESSION=1 LVK_TEST_SERVER="$PWD/target/debug/server" node tests/kanban/isolated.mjs`を実行。pointer列内/列間、keyboard、Running summary fixture、900ms保存＋1100ms確認、素早い連続操作、リロードなしの列間移動→移動先並べ替え→再移動、列件数、非手動ソート制限、実API activity競合、500/503注入、15秒同期タイムアウトと再同期を確認。最後の保存成功は実DBを読んで照合した。

証跡: `.build/drop-fixed.log`、`.build/kanban-drop-validation/{results.json,drop-results.json,revision.json,tested-source-hashes.json}`、`continuous-no-reload.png`、`activity-conflict.png`、`save-failure.png`、`sync-failure.png`、`sync-timeout.png`。最終正常系でSQLite busyは再発しなかったが、先の失敗記録は別ファイルへ保持している。テスト用Chromium/Vite/backendは終了、隔離instance DBは削除済み。通常の`target/`ビルド出力は今後の検証用に保持。

ビルドの注意: 初回は共有中間成果物の不整合による「ソースに存在するmethodが見つからない」失敗があった。再ビルド後も自己申告build revisionが他Workspaceの値だったため、最終試験には上記の明示identityで再ビルドした実バイナリを使用。最終`revision.json`は基点SHA/dirty=true、`tested-source-hashes.json`は実バイナリと変更ソースのSHA-256を記録。共有キャッシュの削除や他Workspaceの変更はしていない。

既存の広いKanbanハーネスは古いバイナリのarchivedフィールド欠落や旧期待値で失敗したため、対象の実画面DnD経路を選択する`LVK_DND_REGRESSION=1`を追加した。fixture seedと起動/終了処理は既存のものを再利用。検証中にリロードなし試験がグリッド右端で移動不能なキーを選んだ失敗も修正し、行内の有効な方向を選択して再実行した。これらの失敗は成功件数へ算入していない。

## 未確認・仕様判断

- 実モデルを実行しながらの操作、実スマホの触り心地、人間の見た目確認は未実施。Running表示fixture＋実API本文更新によるactivity競合を区別して報告する。
- 隔離DBのSQLite busyが1回発生。証跡`.build/drop-fixed-sqlite-busy.log`、`.build/drop-sqlite-busy-server.log`、`sqlite-busy.png`。別途writerを特定し、transaction開始/安全な再試行を検討するか判断が必要。現段階では競合保護を維持した通知・再同期まで対応。
- 競合時にもドロップを常に優先するには、イベントの先頭移動を上書きする仕様判断が必要。本修正では行わない。現仕様を維持して通知するか、明示的な再適用操作を別途設計する選択肢がある。
- Rust全体テスト・全体E2E・production配布ビルド・他ブラウザは対象外。
