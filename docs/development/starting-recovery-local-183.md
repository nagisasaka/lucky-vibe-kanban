# LOCAL-183 Starting保存失敗の局所修正・統合引継ぎ

## 範囲と根拠

出発点は `45fdda98d586940ef45411ac6fcd2872e9143fad`（開始時HEAD一致、作業ツリー未変更）。最新のroot/UI/docs AGENTS、read-only OpenWiki、LOCAL-197の公式Cargo入口・資源方針を確認した。承認範囲は局所実装・隔離test・commit・Change Manifest。main merge、配備build、publish、live DB操作、過去Runの直接復旧、preview停止、restartは行わない。

当時の直接証拠は依頼文の限定診断と調査コメント `ce969638-d7f4-4425-a6bc-8f95e10d2b7c`。当時のDBロック保持者、競合SQL、DevServerとの因果、当時のbinary/source同一性は未確定。本実装でそれらの特定を主張しない。

元sourceの `crates/local-deployment/src/agent_run_port.rs:2574` はStarting保存失敗をlog+returnし、同 `:995` のterminalize_failureもDB書込に依存する。既存append_recoverableはログ/degraded保存を試みるだけで、provider未起動時には再生元のNative Auditがない。host再照合はendpoint/tokenがある予約を対象とするため、未起動の終端保存失敗を再生する耐久証拠を別途保持する必要があった。

## 変更後の経路・保持する不変条件

- 通常createは既存のidentity保存transaction（BEGIN IMMEDIATE）内でprocess registryへ `launch_requested=1` を記録する。受理後のlaunch側障害では受理済みIDを返す。MCP既存のexecution_acceptedは受理であり、実起動/実装完了を意味しない。上位の状態取得が失敗した場合のMCP既存uncertain応答も再送前のoperation/Session照合を指示する。
- setup用reserveは `launch_requested=0`。既存setup gateが解放してlaunch_reservedを配信した時だけ1にする。正常なsetup待ちをfailureや枠待ちと判定しない。
- Starting書込の直前に同一attemptの `starting_write_attempts` を耐久更新する。最大3回、再照合は1秒間隔（DB busy_timeout等の待機時間は別）。Starting event IDは既存の同一attempt由来IDを維持する。成功済みStartingを先に照合するため再配信時は重複保存しない。単なる予算更新のDB busyはproviderを起動せず、耐久要求を保持する。
- 上限後は既存Error/Failedイベントを固定IDで保存する。Failedだけ保存できた場合も保存済み時刻を使ってErrorを再配信し、両方保存するまでreceiptを保持する。新retryが前attemptの終端状態を継承しないよう、Failedを先に保存する。終端保存も失敗した場合、消費済み予算とstarting_failure_pendingフェンスが再起動後も残るので、DB回復後に同じ未起動失敗を終端へ収束させる。失敗した1件で他のreceiptの再照合を止めない。
- 未起動証明はreservedだけでなくhost endpoint/token/instance、host/provider PID、process group、process開始時刻、host cursor、Native Audit stream、local childの不在を確認する。PID=NULLだけで未起動と断定しない。既存host予約があればattach/observerを使い、置換起動しない。
- host予約の原子的UPDATEは、対象attemptが現在のStartingであることも確認する。Starting未保存・Stop先行・旧attemptではhost予約できない。失敗確定の原子的UPDATEはStarting event/host予約の不在を再確認し、starting_failure_pendingで以後のStarting/host予約も拒否する。Startingイベントtransactionも同一attemptのCancel/terminalからの逆行を拒否する。既存command_lockとhost側Launch冪等性は維持する。
- 共通Cancelは上記の未起動証明がある場合Cancelledにする。provider開始の証拠がある場合のwriter欠落は従来どおりfail-closed/audit_failed。
- pending/startingはUIでPreparingとし、実行時計を表示しない。workspace cardの開始時刻は受理時刻ではなくregistryの実開始時刻。chatは取得済みRunningイベントがあればその時刻を使う。既存のactive集約/Stop可否は維持し、同時実行枠待ちとは表示しない。

## Migration `20261026180000_agent_starting_recovery.sql`

既存 `agent_process_registry` にNOT NULL/DEFAULT 0の `launch_requested` と `starting_write_attempts`、および `starting_failure_pending` の3列を追加する。値のCHECK制約も追加する。新規tableはない。新規indexは `idx_agent_process_registry_prelaunch_recovery` の1件で、起動要求済み・reserved・endpointなしの行だけを対象とする部分index。

このmigrationには既存行へのUPDATE/DELETE/INSERT、Run状態変更、PID/host予約変更、過去Runの再起動指示はない。既存行は全3列とも既定値0として読まれ、過去pendingを自動backfill/復旧しない。index作成時にも既存行はlaunch_requested=0なので回復対象にならない。

統合後の初回起動では既存migration入口がDDLを適用し、その後、通常のhost再接続と本件のreceipt再照合が動く。旧予約はhost再接続等の従来経路だけを使う。既存schema guardによるupdated_at欠損の補修やhost再接続に伴う観測更新は従来どおりの別処理であり、本migrationによる既存行更新ではない。新規受理済みreceiptは再起動を越えて回復対象になる。DDL/index構築にはSQLite writer lockとschema確認が必要で、所要時間は既存DB規模による。本Runは実DBへの適用、旧binaryへの戻し、配備互換性を実行していない。統合担当が承認済みの隔離candidate/DBコピーで確認し、通常の移行・backup方針を適用すること。

## 検証記録

以下はこのcheckoutの隔離fixtureだけで実施。実モデル呼出し、live DB/実Run操作、配備buildは対象外。

| コマンド | 結果・範囲 |
| --- | --- |
| `node scripts/dev-cargo.mjs test -p local-deployment --lib agent_run_port::tests -j 4 -- --test-threads=2` | 27 passed。Starting保存失敗/実SQLite busy、回復後Starting1件、受理済みID保持、重複launch/Stop、未起動Cancel、retry予算/失敗フェンス/終端・診断保存失敗の再照合、前attempt終端の非継承、setup待ち除外、host予約PIDなし時の置換禁止、実開始証拠ありのaudit_failed、既存host/audit回復を検証。 |
| `node scripts/dev-cargo.mjs test -p db --lib models::agent_runtime::tests -j 4 -- --test-threads=2` | 39 passed。旧行migration保存試験、受理とlaunch receiptの原子的保存、setup非対象、host fence、既存Kanban/SQLite busy/host replay回帰。 |
| `pnpm --dir packages/web-core exec vitest run src/features/agent-runtime/model/canonicalAgentConversation.test.ts src/features/workflow/model/taskAttempt.test.ts --maxWorkers=2` | 46 passed。pending/startingのPreparing表示と時計抑止、Running証拠からの時計、既存会話/状態回帰。 |
| `pnpm run web-core:check` / `pnpm run ui:check` / `pnpm run local-web:check` | 全てpassed。 |
| `pnpm --dir packages/ui exec eslint src/components/IssueWorkspaceCard.tsx src/components/WorkspaceSummary.tsx --max-warnings 0` | passed。 |
| `pnpm --dir packages/local-web exec eslint --config .eslintrc.cjs ../web-core/src/features/agent-runtime/model/canonicalAgentConversation.ts ../web-core/src/features/workflow/model/taskAttempt.ts --max-warnings 0` | passed。web-coreにはstandalone lint設定がなく、既存local-web設定でproduction変更箇所を検証。testファイルは当該lint TSConfig対象外なのでVitestで検証。 |
| scoped rustfmt / Prettier check / `git diff --check` | passed。 |

隔離migration試験 `starting_recovery_migration_preserves_legacy_rows_without_launching_them` は実旧schemaを作り、registry-onlyの合成reserved/running/exited行について行数・既存identity/status/endpoint不変、全3列の既定値0、新規tableなし、部分index作成を照合する。この薄い旧行fixtureはFKを無効化して移行対象だけを隔離する。別のfull-chain fixtureでは全migrationを実行し、実FK anchor/Workspace/Sessionと通常create→受理/Stopを検証する。完全な既存データの移行互換性と同一視しない。

初回のfixture検証ではファイルDB作成不備、旧migration自身のFK再有効化・process状態CHECKと合成行の不整合を検出し、fixtureを修正して再検証した。製品修正の未解決失敗として隠さず、最終成功と区別する。

資源snapshotは登録資源/holder/queueなし。公式Cargo入口、checkout標準target、shared build-dirを使用し、Rust検証は逐次、jobs=4・test threads=2。メモリavailable約9–12 GiBとpressureを観測し、既存Cargo lock待ちは待機した。他Runの停止、cache削除、別target/agent追加はしない。node_modules/標準targetは通常の作業用として保持し、試験DB/auditはTempDirで解放する。

server crate全体check/Clippy/全体build・test、SQLx prepare全体再生成、browser/E2E、実provider1回起動のend-to-end照合は未実施。並行作業と本件局所検証の範囲を守り、配備buildを行わないため。追加したSQLはruntime queryで新しいquery macro metadataはない。server側変更は既存summary DTOへの実開始時刻のmapping 1箇所。統合candidateで必要なconsumer/配備互換性を別途検証する。

## 並行変更との交点

- LOCAL-206: `agent_runtime.rs`/`orchestration.rs` 周辺。後者の本件変更は隔離fixtureへのmigration追加のみ。process registryの耐久起動要求/予算、setup待ち非対象、host予約のStarting/attempt照合、共通Cancelの未起動証明と実監査障害のfail-closedを残す。DevServerの停止追跡をagent未起動証明に流用しない。
- LOCAL-81: `IssueWorkspaceCard.tsx`/`WorkspaceSummary.tsx` とworkspace summary/model付近。card sort/realtime配信を維持しつつpending/startingのPreparing表示・時計抑止と、実開始時刻のsummaryを残す。is_runningは既存の複数active Run/scripts集約のままであり、最新Runだけから再定義しない。
- 本件はrun-manager、outbox配信契約、正式Integrationの採用基準やDone条件を変更しない。交点は統合時に最終candidateのsource/testsで再確認する。

## 限界・残存リスク

DBが永久に書込不能なら耐久終端は保証できない。その間はpending/Preparingとログで観測し、受理IDを保持する。budget更新前のDB write不可能期間にも実行時間上限の保証はない。未知のhost予約は既存host回復に委ね、PID不在を理由に再起動しない。既存schema破損やディスク/DBの消失を自動修復する基盤ではない。

実provider起動回数のE2E、実モデル、稼働DBへのmigration、binary/APK等の配備互換性、browserでの見た目、全体build/testは局所fixtureと区別して報告する。chat履歴にRunningイベントが未取得の既存ケースでは従来の受理時刻fallbackが残るが、pending/startingの時計は抑止される。今回の局所変更をmain採用/配備承認と扱わない。
