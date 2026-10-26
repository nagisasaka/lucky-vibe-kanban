# Generated Workspace archive: lifecycle and review record

2026-10-26、LOCAL-178。実装開始基点は `c8d79127e02da3eee425b4963febcbfb28bf0ea6`、初回実装checkpointは `058325f5e03b3c5ae7cb821646d86c9d8694ee36`。今回の修正で、別系統のmain（観測 `05a959eff7e74ad5cfb65d9b97da11f6cc63d891`）は取り込んでいない。以下はsourceの静的確認であり、実サーバー・実Workspaceの操作結果ではない。

## claim後の介入と削除前の境界

[自動archive](../../crates/server/src/routes/local_remote/issue_archive.rs) は生成元記録のrevisionを読み、同じrevisionかつ未試行の場合だけ、transaction外でstarted intentを永続化する。`settled()`待ち中もSQLite writerを占有しない。

Issue unarchiveのREST/MCP経路は `unarchive()` を呼び、同関数はIssue lockを取得しない。したがってin-process lockだけには依存できない。[migration](../../crates/db/migrations/20261026010000_generated_archive.sql) のtriggerは、startedを上書きせずに、archive state・link・owner/path変更で `eligibility_revision` を進める。自分のlink変更は未試行の資格も消費する。他Workspaceのlink変更はIssueの集合revisionを進め、claim中の試行を失効させるが、後から始まる専用Workspaceの試行までは禁止しない。

`settled()`後、`BEGIN IMMEDIATE` 内でrevision/started、現在owner・path・branch、repository所属と物理path/name、Issue所属link、成果成功、既存busy/process guardを再検証する。同じwrite transactionを物理削除とcommitまで保持するので、この最終検査後に別DB writerがmanual intentやlink ABAを割り込ませることはできない。失効・失敗・中断時は残存を許容し、自動再試行しない。通常の削除エラーは既存 `archive_checked` に従いarchivedを確定する。filesystem変更後のDBエラー/commit不明は結果不明として扱う。

## releaseからcleanupへの経路

共通入口は従来の [OpenWiki recovery monitor](../../crates/server/src/routes/openwiki.rs) の5秒tick末尾。`release_completed()` が `execution_only → interactive` の更新に成功したWorkspaceだけを返し、その回に限りarchiveを試す。新timer、過去のinteractive Workspaceの巡回、再試行frameworkは追加していない。

| owner | 成果と参照の解放 | release後のarchive条件 |
| --- | --- | --- |
| Bootstrap | [bootstrap runtime](../../crates/server/src/workflow_runtime/bootstrap.rs) のchild完了処理で全attempt/process終了確認とinstructions復元。Workflow成功後 `publish()` と `record_reconciliation_result()` を完了し、repository stateのbootstrap/active_runを解除 | [cleanup_allowed](../../crates/services/src/services/workspace_usage.rs) がterminal Workflowとrepository owner解除、共通process idleを確認。archive側はrun/workspace/repository一致とbootstrap Workflow成功を再確認 |
| Sync | [finish_run/recover_repository](../../crates/server/src/routes/openwiki.rs) でprocess終了証拠・instructions復元・出版を確認し、run固有成功結果を保存。`record_reconciliation_result()` がreceipt/stateを確定しactive ownerを解除 | 同じrelease predicateでprocess idle、terminal owner結果、repository owner解除。archive側はAgentRunのworkspace/成功とowner成功結果を照合。`no_op=true` も成功として扱う |
| Formal Integration | [finish](../../crates/server/src/routes/integrations/runtime.rs) は `published=true` が前提。Card完了、semantic outbox記録の安全な試行、product/run成功保存、reservation解放 | [release_completed](../../crates/services/src/services/workspace_usage/lifecycle.rs) はterminal runとreservationなしを確認。archive側はrun/workspace/repository/project/card一致と出版成功を確認。AgentRunを要求しないためmechanical経路も通る |

Integrationのsemantic記録失敗は `wiki_result="semantic outbox pending: ..."` として成功から分離され、downstream Wiki完了を待たない。成功runの `request_recovery()` はrepository本体、凍結payloadのevent/manifest、memory storeからsemantic記録を再構成し、専用Integration worktreeを読まない。参照は [record_memory_integration_at](../../crates/server/src/routes/integrations/runtime.rs) と [既存outbox試験](../../crates/server/src/routes/integrations/runtime_tests.rs)。これは静的根拠であり、今回outbox試験を再実行したものではない。

## attempt_resultと既存receiptの違い・観測

既存 [Operations receipt](../../crates/server/src/remote_mcp/operations.rs) は、認証principal/request_id/input hashに結び付いたUI/MCP要求の受付・再送・結果照会を担う。人間の確認tokenと、そのreceipt/APIは変更しない。

`generated_issue_workspaces` は内部生成元・最初のowner run・失効revision・一度限りの自動後処理intent/結果を保持する専用記録。人間の要求や確認を偽造せず、request_idも作らない。したがって既存 `issue-archive-operations` APIで自動後処理結果を照会できるわけではない。新しい汎用receipt/retry基盤ではない。

既存UI/APIで確認できるのはIssue/Workspaceの現在のarchived状態、Workspaceの `worktree_deleted` など。内部attempt_result専用のUI/GET APIは追加していない。今回、既存server logへWorkspace/Issue ID付きの結果を出すようにした。成功はINFO、guard拒否・結果不明・物理削除未完了はWARNで、Issueを残したかと `deletion_error` も含む。log filterがINFOを抑制していれば正常成功logは見えない。中断でstartedだけが残る場合、内部DB記録は結果不明のままで自動再試行しない。

## 今回の検証と保留

- `python3 tests/generated-archive/test_migration.py`: 隔離SQLite 9試験成功。claim後のmanual unarchive/archive cycle/link ABA/owner ABA/path ABA、および二接続での `BEGIN IMMEDIATE` によるwriter排他を含む。
- Rustの `generated_archive_invalidates_intervention_after_claim_before_deletion` に同じ5種類の介入をsettled callback内で注入する回帰を追加。Rust実行はslot禁止のため未実施。
- 変更Rustファイルのrustfmt check、`git diff --check` を実施。型・SQLx runtime decode・実busy/process guard・実outbox回復はRust slot取得後の検証対象。

最初の最小cargo候補（未実行、既存profile/target設定を変えず単独実行）:

```sh
CARGO_BUILD_JOBS=1 cargo test -p server --lib routes::local_remote::issue_archive::tests::generated_ -- --test-threads=1
```

同じtest binaryが出来た後の既存回帰候補は `cargo test -p server --lib semantic_outbox_uses_only_frozen_events_and_rejects_changed_or_missing_inputs -- --test-threads=1`。全面build/testやmain統合の代替ではない。

読取時のworkspaceは96 MiB、専用 `target/` は存在せず、`CARGO_TARGET_DIR` とuser Cargo build overrideも未設定。`Cargo.lock` は929 packageを含む（このcommandが全packageをcompileするという意味ではない）。空きは約40.25 GiB。fresh server test buildの追加diskは実測不能で、計画用の粗い見積りを20–40 GiB程度（link/tempで上振れあり）とする。専用target/cacheを持たないため、この空きだけで実行可能とは判断しない。177 migrationfix・182/bundleを優先し、slotとdisk予算の確認まで起動しない。

共有sccache用directoryは存在するが、`sccache --show-stats` の稼働serverは `/home/appuser/.cache/sccache` を報告し、推奨 `.lvk-shared/cache/sccache` と不一致。compile requestsは0。server再設定・停止、cache削除、共有targetへの切替は行っていない。実行時wrapper/cache方針はこの不一致を上位担当と調整する必要がある。
