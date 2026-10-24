# 既存GoalのMCP操作

既存UIと同じGoal command／Session follow-upを外部HTTP MCPへ公開する。新規Goal起動、達成の代理宣言、自動承認、Codex native管理への全体委譲は含めない。stdio MCPは既存の独立したcatalogを維持する。

| Tool | 意味 |
| --- | --- |
| `get_goal` | UIと同じ保存projectionの状態、使用量、revision、最新runかどうか、任意commandのdeliveryを読む。providerへの直接問い合わせではない |
| `pause_goal` | 活動中Goalを`paused`にする。実行中turnの終了証拠ではない |
| `resume_goal` | 非活動Goalを`active`に戻す。blockedの原因解消や予算・usage制限の解除を自動では行わない |
| `cancel_goal` | Goalを持つ現在のAgentRunにCancelを送る。Goalはprovider側に保存され得る。達成・解除を意味しない |
| `clear_goal` | UIのClear Goalと同じGoalClear。Goal管理を終了する。成功宣言やprocess停止証拠ではない |

曖昧な`finish_goal`は設けない。`complete`はproviderの達成報告として読み取る。現在turnだけのInterruptTurnも別概念であり、この追加catalogには含めない。

終了済みrunの再開では、保存executor設定を読み、UIと同じGoal mode・`/goal resume`を既存`follow_up_checked`へ渡す。PLAN権限はUIと同じSUPERVISEDに変更し、他の保存設定は保持する。新objectiveや予算上書きを送らない。既存provider session bindingがない場合はUI・MCP共通のfollow-up経路で拒否する。providerが保存Goalの有無と達成状態を確認し、再開失敗をrunへ記録する。

## 受付と読戻し

1. `list_workspace_runs`でProject／Workspace／Session／最新AgentRunを選び、`get_goal`を読む。objective本文は既存`get_execution(section=goal)`で有界取得する。
2. 4つのID、`expected_revision`、UUID `request_id`を指定して操作する。readに加えwrite scopeが必要。既存Project権限・MCP監査を通る。
3. 再送は同じrequest_idと同じ入力を使う。変更した入力の再利用は拒否する。`get_operation`でdurable receiptを読む。accepted/runningは処理中、succeededはdispatch受付・deliveryの結果であり、Goal遷移・run終端の証明ではない。
4. `get_goal`で結果を再読する。command操作はreceiptの`result.command_id`でdeliveryを取得できる。dispatch後に結果取得が失敗したuncertainの場合、操作の`operation_id`がcommand_idに対応する。終了済みrunからの再開は新しい`result.agent_run_id`を読む。これはcommand型の操作ではない。

古いrun、親ID不一致、古いrevision、非current projection、archived／実行専用Workspaceは操作を拒否する。最新runと状態はSession lock内で再確認する。終了済みrunの再開は既存follow-up側の同じlockでlatest/terminal／queue／活動runを再確認し、異なるrequest_idによる競合でも二重起動させない。dispatch後の例外は副作用なしと断定せずuncertainにし、自動再実行しない。Goalがprojectionから消えたことだけでproviderの解除・process停止は断定しない。

LOCAL128のIntegration MCPと同じ`Operations`、`get_operation`、scope routing、監査入口を利用する。Integrationのmerge権限や承認操作は変更しない。

## 初回実装時の検証記録（2026-10-24 JST）

- ベースSHA: `2803307d61079ca7f4ddcfc697d727d13db3a7e7`。開始時のmainと同一で、LOCAL128のIntegration MCPを含む。
- 08:24:02開始。契約・既存処理調査は08:26頃まで（約2分）。実装・fixture追加は08:29頃まで（約3分）。以降、軽量確認・記録・コミット。
- 変更したRustファイルへの `rustfmt --edition 2024 --check --config skip_children=true ...` と `git diff --check` は成功。
- 統括の資源調整指示によりCargo build/check/test/linkは未実行。コンパイル・Rust fixture成功は未確認。実Goal、実DB、実認証設定、デプロイ、再起動、main mergeは行っていない。
- fixtureはin-memory SQLite＋mock runtime。冪等競合、異なるrequestによる再開競合、親ID／revision／最新run拒否、projection／archive／権限拒否、状態遷移、予算・使用量・設定保持、dispatch後uncertain、deliveryと終端の区別を定義。既存HTTP認証fixtureへ4操作のwrite scope拒否を追加。

初回実装時に記録した試験計画は以下（当時は未実行、実行結果は次節）。`RUSTC_WRAPPER`が未設定なら`/usr/local/bin/sccache`を使用し、`SCCACHE_DIR`は`.lvk-shared/cache/sccache`の実pathを設定する。既存serverのcacheを変更・停止せずstatsで確認する。clean禁止。

```sh
CARGO_BUILD_JOBS=2 cargo test -p server remote_mcp::goals::tests --lib
CARGO_BUILD_JOBS=2 cargo test -p server remote_mcp::runtime_write_tests --lib
CARGO_BUILD_JOBS=2 cargo test -p server remote_mcp::tests::http_discovery_authentication_and_tool_call_roundtrip --lib
CARGO_BUILD_JOBS=2 cargo test -p server remote_mcp::tests::plugin_tool_catalog --lib
CARGO_BUILD_JOBS=2 cargo test -p executors resume_goal --lib
```

最初のserver試験で関連crateのコンパイルも確認するため、別の大型checkを先行させない。実Codexとのend-to-endや実Auth0接続はこのfixtureで証明しない。fresh独立レビューは統括が別途依頼する。

## 候補1Bの追検証（2026-10-24 JST）

検証元SHAは `a43dcf13e45dcde3b2ed618a0b8fda8a16af674d`。追加変更は実共有経路fixtureと、`dispatch_control`の返却値を`let _ =`で明示的に破棄する警告修正。後者は受付・状態遷移の意味を変更しない。

[追加fixture](../../crates/server/src/remote_mcp/goals/tests.rs)は実`DeploymentImpl`で`goals::dispatch`を呼び、Session lock解放後に`follow_up_checked`が同じlockを取得し、既存provider binding欠落の具体的なConflictを返すところまで通す。timeoutとその後のlock再取得、新run・attempt・command・process・provider bindingの未生成、元Goal/run/revision保持を検査する。mock runtimeはこのdispatchに使用しない。

instance-global設定とbackground taskを持つため、このfixtureはignoreを付けた単独プロセス試験。空の`goal-mcp-fixture-*`ディレクトリを`LVK_INSTANCE_DIR`に指定し、専用PATH上のCodex拒否stubと`LVK_GOAL_TEST_CODEX_DISABLED=1`でモデル起動を防ぐ。stubの未呼出しも確認する。DB・設定・direct-folderはすべてこの一時instance内に作り、プロセス終了後に削除した。実環境の認証設定は変更していない。

共通実行条件は`CARGO_BUILD_JOBS=2`、`RUSTC_WRAPPER=/usr/local/bin/sccache`、`SQLX_OFFLINE=true`。`SCCACHE_DIR`は既存`.lvk-shared/cache/sccache`の実pathと一致し、既存serverを使用した。profile・incremental設定は変更せず、cleanも実施していない。共有資源snapshotはresources/holders/queueすべて空で、登録済み共用資源は使用していない。

実行コマンドと結果:

| コマンド／filter | 結果 |
| --- | --- |
| `cargo test -p server remote_mcp::goals::tests --lib` | 8成功、単独実行fixture 1件ignore |
| `cargo test -p server remote_mcp::runtime_write_tests --lib` | 4成功 |
| `cargo test -p server remote_mcp::tests::http_discovery_authentication_and_tool_call_roundtrip --lib` | 1成功（Goal 4操作のwrite scope拒否を含む） |
| `cargo test -p server remote_mcp::tests::plugin_tool_catalog --lib` | 1成功 |
| `cargo test -p server --lib remote_mcp::goals::tests::goal_real_dispatch_releases_lock_and_rejects_missing_provider_binding -- --ignored --exact --test-threads=1` | 1成功、Codex stub未呼出し |
| `server_test_binary remote_mcp::tests::issue_writes_reuse_board_services_and_enforce_scope_retries_and_revision` | 1成功（共通receiptの権限・再送・revision） |
| `server_test_binary remote_mcp::issue_write::tests::unreadable_receipts_never_report_success_or_leak_the_damaged_payload` | 1成功（結果読戻しの破損・秘匿） |
| `cargo test -p executors resume_goal --lib` | 2成功（objective・budget・accounting保持、missing/complete拒否） |

`server_test_binary`は実共有経路試験でコンパイルした`target/debug/deps/server-4ecb5813dcfd5681`。ビルドmetadataの毎回再計算による不要な再リンクを避け、警告修正後の同一バイナリで最初の4filterも再確認した（計16件成功）。その後にRust sourceは変更していない。

- 最終server試験バイナリSHA-256: `7ca9c6c8c71764a2c075c9f30a6edb78b06da937c246b8f16983eea9cda708d5`
- 上記baseからの検証済みRust source差分SHA-256: `d4dfac6f5e74713e1aecca25db4bcf80776dcc955a1b2c6b964d48c811daa939`
- 整形・差分チェックも成功。独立ソースレビューは統括から「阻害事項なし」と通知済み。今回の追加fixture・警告修正に対するfresh独立レビューは別途。

時間・資源（JST）:

| 工程 | 開始→終了 | 所要 |
| --- | --- | --- |
| 資源・契約確認、実共有経路fixture追加 | 08:48:13→08:49:49 | 1分36秒 |
| server初回compileと4filter | 08:49:49→09:01:31 | 11分42秒 |
| 実共有経路fixture単独実行 | 09:01:41→09:01:58 | 約18秒 |
| 修正後serverの6filter | 09:02:26→09:02:36 | 約10秒 |
| executors compile＋2試験 | 09:02:36→09:06:45 | 4分9秒 |
| 試験・軽量確認完了 | 08:48:13→09:06:59 | 18分46秒 |

初回は空き84GiB/RAM available12GiB。証拠保存前の最終観測は空き65.58GiB/RAM available13.24GiBで、50GiB/4GiBの抑制基準には達しなかった。通常のWorkspace専用targetは約16GiBで保持する。記録・コミットを含む作業完了時刻、最終commit SHAは保持証拠のvalidation-record.jsonに記録する。

実行script・log・source差分は作業用`.build/goal-mcp-evidence/`から共有persistentの`knowledge/validation/4d790296-860a-4cbd-842a-2c01c30069fc/5a6f6f77-46a9-4f81-86d2-8ff3066b2930/`へ保存し、作業用の重複証拠を削除する。通常のWorkspace専用`target/`は次の検証用に保持し、一時instanceは削除する。ログのcache counterは共用server全体であり、この試験単独のhit率ではない。

実Codex・実Auth0のend-to-end、frontend一式、workspace一式のbuild/lint/testは未実施。実Goal操作・main merge・deploy・再起動は行っていない。初回コンパイルが作ったfrontend placeholderはUI検証の証拠として扱わない。
