# LOCAL-206: managed DevServer MCP の実装・検証引継ぎ

基準commit: `45fdda98d586940ef45411ac6fcd2872e9143fad`。
Workspace: `e131bcdc-3b0f-4d4a-9f8f-aca87d15ed33`。
実agentの人間承認後の呼出し成功とbrowser UI表示は未検証として引き継ぐ。
このWorkspaceの範囲は局所検証、source commit、semantic draftまで。
LOCAL-81/204/206の統合・配備・sccache停止・常用dev再起動は別の統合担当が実施する。

## 既存経路と変更

既存UI startはrunning DevServerを停止して起動し直していた。
現在は `routes/workspaces/dev_server.rs` の共通managerをUIとMCPから使用し、
running processがあればその識別子を返す。start/stopは既存のmutation admissionと
Workspace単位の順序制御に参加する。UIのprocess単位stopも同じ順序制御に参加する。
カンバン表示とfrontendは変更しない。

stdioのglobal/orchestrator catalogとremote catalogへ、次を追加する。

- `start_dev_server`
- `stop_dev_server`
- `get_dev_server_status`

Project/Workspaceは必須で、所属を検証する。stdioは既存scope判定を再利用し、
orchestratorの対象差替えを拒否する。remoteは既存の認可・監査・永続receiptを使う。
statusにはread/runtime-read、start/stopにはさらにruntime-executeが必要。
remoteの変更要求にはstableなrequest_idが必要で、同じIDのretryは再実行しない。

remote receiptの受付・実行結果と、managerの観測結果は別である。
stdioの変更応答はmanagerの結果を直接返す。`accepted`、`requested_action`、
`target_state_observed`、`failure_reason`を観測と一緒に返す。
副作用後に観測できなくなったremote operationは`uncertain`とし、
新しいrequest_idでの盲目的な再実行を勧めない。

statusはDBのprocess記録と登録済みOS processのpresenceを読む。
`running`は両方を確認できた状態であり、HTTP health/readinessの保証ではない。
DBがrunningでもpresenceが未確認なら`running_unconfirmed`、
DBがterminalでもcleanup追跡が残れば`stopping_or_unconfirmed`とする。
未解決のcleanupがあれば新しいstartを拒否する。
repositoryごとの最新結果と全running processを返すため、部分的なscript失敗を隠さない。

既存のregistry検索は破損情報を修復することがあった。
statusと個別scriptのsettlement確認には、修復しないstrict readを使う。
破損情報はprocess不在の証明として扱わない。

## 局所検証の再実行

repository rootから現行Cargo入口を使う。
並列度は利用可能memory、他runのpeak、service headroom、pressureを観測して選ぶ。
共有build-dirの通常ロック待ちは異常ではない。他runやsccacheを停止しない。

```sh
node scripts/dev-cargo.mjs test -p server -p mcp -p local-deployment --lib -j 4 -- dev_server stdio_codec public_workspace_and_session_routes_reject_before_handler_side_effects remote_mcp::runtime_write_tests
node scripts/dev-cargo.mjs build -p mcp -p local-deployment --bin vibe-kanban-mcp --bin agent-process-host -j 4
LVK_DEV_SERVER_REAL_AGENT=1 node tests/server/run-managed-dev-server.mjs
```

最後のrunnerはcheckout配下の`.build/devserver-fixture-*`だけを使用する。
本物のUI manager/remote HTTP transport/stdio binaryを使い、認証providerはmock。
managed scriptは隔離repositoryのsleep fixtureであり、preview/dev/stableを使わない。
`LVK_DEV_SERVER_REAL_AGENT=1`では、その隔離instanceのprofile overrideだけで
候補stdio binaryを接続し、実LVK Codex Sessionにstatusを呼ばせる。
実agentによるstart/stopや、常用instanceのagent設定変更は行わない。

runnerはprocessの終了を確認してから成功instanceを削除する。
失敗や残存processがあれば証拠を残して失敗するので、そのinstanceを確認する。
実Auth0/ChatGPT、常用instanceのcatalog/agent設定、HTTP application readinessは
このfixtureの成功とは別に扱う。

## 統合・再起動前の確認

LOCAL-204と共有する `crates/mcp/src/task_server/tools/mod.rs` と
`crates/server/src/remote_mcp/mod.rs` の加算登録を両方保持する。
3件を統合した凍結候補で、適用されるIntegration検証と境界試験を実施する。
backendのSPA fallback試験を含める場合は実frontend distが必要。
server、agent-process-host、MCP、frontendの実artifactとsourceを一致させる。
複数repoの部分失敗表示はDB projection試験であり、実repo群の起動/停止障害を網羅したものではない。
履歴の多いWorkspaceではstatusが過去DevServerのsettlementも読むため、その負荷を確認する。

配備前後の検証では、MCP catalog、remoteの認可・scope、実agentのstdio設定を区別して確認する。
managed start/stop試験は引き続き隔離対象だけで行う。
preview2のDB・Basic認証・session・ユーザー追加タグを保全する。
OS process presenceとHTTP readinessは別であり、利用Projectのscript成功を一括保証しない。

Change Manifestのsemantic入力は、このAgentRunに指定されたshared draftへ保存する。
正式なcommit/path/event identityはLVKがGitから導出するため、この文書で代替生成しない。

## 今回の実agent境界で確認した未達

隔離SessionのAgentRun `0ecc08a9-e220-48da-82e2-e0eb8ae76eb1` は
`lvk_fixture/get_dev_server_status` を要求したが、tool結果はfailedだった。
Native Auditで `mcpServer/elicitation/request`、
`_meta.codex_approval_kind=mcp_tool_call`、対象toolがstatusであることを確認した。
providerの結果は `user cancelled MCP tool call`。
基準commitの `crates/executors/src/executors/codex/client.rs` は
`McpServerElicitationRequest` にCancelを返す実装だった。
実ユーザーがこの試験で取消したことを示すものではない。

したがって、実agentへのcatalog露出と呼出し要求は確認できたが、
実agentによるstatus成功は未達。candidateの実stdio transportによる直接呼出しとは区別する。
この初回試験時点では承認設定の自動許可化、実ユーザーの承認代行、Codex clientの承認UI拡張は行っていない。
統合担当は承認UI対応の責務・方針を判断し、正規の承認経路で実agent受入を再確認する必要がある。
この未達を完了・成功として扱わない。

fixtureの初期化では、正式migration互換選択を使わない既存helperによるchecksum不一致を検出し、
helperを `db::run_migrations` に揃えた。migration SQLや履歴checksumの書換えは行っていない。
fixtureは仮のGitディレクトリ登録をやめ、正規managerで実worktreeを作成する。

共有Cargo中間成果物の再利用時に、sourceに存在するtrait methodが見えないcompile失敗も観測した。
現在checkoutの依存を再コンパイルするとそのエラーは解消した。共有cache原因という一般化は未検証。
cache削除・sccache停止・build policy変更は行っていない。
凍結統合候補のartifact/source一致を別途確認すること。

## 承認Cancelの読取調査（追加判断）

取消した主体はLVKのCodex app-server接続adapterである。
基準commitの `crates/executors/src/executors/codex/client.rs:916–932` は要求の内容・許可結果を照会せず、
`MCP elicitation UI is unavailable; cancelling the request` を理由に、元のJSON-RPC IDへ
`action: Cancel, content: None, meta: None` を返す。
人間が拒否した記録、権限scopeでDevServer APIが拒否された記録とは扱わない。
この分岐は既存の `request_tool_approval`（同:996–1080）を通らない。

保存した秘密値除外の証拠は `managed-dev-server-mcp-evidence.json`。
Native Auditのsequence 55（18:18:52.413038803Z）はrequest ID 0の要求、
56（18:18:52.417351179Z）は同IDの `serverRequest/resolved`、
58に対応するtool結果（18:18:52.424385895Z）はfailedで
`user cancelled MCP tool call` だった。
`send_server_response`（client.rs:1961–1975）は送信内容をlog_rawへ記録しないため、
保存auditにCancel応答の生wire frameはない。Cancelという応答内容と理由は上のsource分岐から確定し、
保存ログの直接観測と区別する。警告文字列もsourceの理由であり、保存済み警告行と偽らない。

`ServerRequest` はCodex app-serverからLVKへのJSON-RPC要求というtransport上の分類である。
それ自体は「上流MCP serverが任意の情報入力を求めた」という意味ではない。
今回の `_meta.codex_approval_kind=mcp_tool_call`、空のobject schema、
`Allow ... to run tool "get_dev_server_status"?` はCodexのtool-call承認要求である。
LVK MCPのstatus実装から発行したelicitationではない。

根拠としてCargo.lockの固定Codex revision `44918ea10c0f99151c6710411b4322c2f5c96bea`
（rust-v0.144.1）のsourceも読取照合した。以下のpathは同revisionの `codex-rs/` 起点。
実行中Codex executableの完全なビルド由来まで証明したものではない。

- `core/src/mcp_tool_call.rs:1681–1720`: Codex自身が空schemaとmcp_tool_call metadataの承認elicitationを生成。
- 同:1821–1857: Acceptは一回の許可、明示したpersist metadataだけがsession/alwaysへ拡張。DeclineとCancelを区別。
- 同:274–285: McpToolApprovalDecision::Cancelの表示文言が `user cancelled MCP tool call`。文言だけで実際の人間操作を推定できない。
- `app-server/src/bespoke_event_handling.rs:763–784`: elicitationをapp-serverのServerRequestへ転送。
- `app-server-protocol/src/protocol/v2/mcp.rs:297–311`: thread/任意turn/server/requestを持つがtool item IDは未提供（TODO）。
  MCP側request IDとapp-server JSON-RPC IDも別である。

**読取判断：今回の一回限りのtool-call承認には独立した汎用承認UIの新設は必須ではなく、
既存の正式承認契約へ接続する限定adapter不具合修正が候補となる。**
既存経路は次のとおりで、MCP要求だけが未接続である。
下のRust pathはrepositoryの `crates/` 起点。

- `local-deployment/src/process_host/approvals.rs:27–60` は任意tool名で既存bridgeのwaiterを使用。
- `services/src/services/approvals/executor_approvals.rs:53–78,121–199` はservice所有のapproval ID、
  run-attempt所有、明示応答、timeout/cancel、二重・期限後応答の拒否を扱う。
- `executors/src/executors/provider_adapter.rs:1458–1488` はapproval requested/resolvedをcanonical eventへ投影。
- `server/src/routes/agent_runs.rs:313–331` と `local-deployment/src/agent_run_port.rs:3325–3364` は
  正式なrun controlを通じて既存provider waiterへ回答する。
- `packages/web-core/src/features/agent-runtime/model/canonicalAgentActions.ts:87–103,160–165` と
  `features/workspace-chat/ui/SessionChatBoxContainer.tsx:1098–1110,1168` は待機承認を表示・許可/拒否する。

限定修正でも、mcp_tool_callかつ対応可能なformを明確に識別し、server/tool/対象引数をユーザーが
確認できる表示にし、service発行approval IDと元のapp-server request IDを正しく対応付ける必要がある。
未提供のtool item IDを推測したり、JSON-RPC IDをservice所有approval IDとして流用してはならない。
明示許可だけをAccept（persist metadataなし）、拒否をDecline、timeout/cancel/不明をCancelへ写像し、
回答待ち中もrun cancelを維持する。tool item関連付けは別途検証が必要である。
未知のmetadataや一般server form/URL elicitationはこの限定修正で許可しない。
任意schema・URL・session/always記憶まで扱うなら別のUI/契約設計が必要となる。

この調査時点では読取判断と証拠整理のみ。後述の追加実装承認前にはadapter/承認UI/permission/security grantを変更していない。
明示的な人間承認を伴う実agent再試験は未実施であり、
実agent呼出し成功という受入条件は引き続き未達である。

失敗試験の原証拠は `.build/devserver-fixture-pcAekf`（約2.4 MiB）と
`.build/devserver-fixture-a8mbJR` に保持する。
これはこのrun所有の隔離DB・native auditであり、公開/commit対象ではない。
終了時に同instanceのprocess残存を確認する。その他の既に説明済みの失敗scratchは
process不在を確認して削除し、通常targetと共有cacheは保持する。


## 追加承認による一回限り承認adapter修正

読取判断の後、ユーザーが206内の限定接続修正を明示承認した。
`client/mcp_approval.rs` に限定して実装し、既存bridge、canonical approval event、
既存run-control入力、既存UIの承認待ち投影を使用する。独立した汎用elicitation UIは追加しない。

- root thread、active turn、実行待ちMCP itemのserverと引数が完全一致し、候補が一つだけの場合に限る。
  tool名はitem由来で、自由文message/description/display metadataから推定しない。
- third-party MCP server/toolとone-call-onlyを明記し、同じtool item IDで既存の引数/scope表示へ結び付ける。
  引数や自由文をラベルへ複写せず、既存の表示・redaction経路を保持する。
- service所有approval IDとnative JSON-RPC IDを別管理する。native IDの再送は新しい承認/応答を作らず、
  一度使用したtool itemへの別ID要求も許可しない。上限1024要求を超えるとfail-closedとなる。
- 自動承認設定を参照しない。Noop serviceも許可へ使わない。明示許可だけAccept、拒否はDecline、
  timeout/cancel/不明/対象変化はCancel。Acceptにsession/always metadataを付けない。
- 空object schemaのmcp_tool_call以外、一般form、URL、invalid schema、未知mode、
  対象不一致・曖昧な対応はCancel。新credential/security grant/allowlistは追加しない。
- 既存command/file承認と共有するpending件数通知も発行し、正式UIのawaiting_approval gateを有効にする。
- 元request IDと応答actionは既存approval_protocol_responseの監査経路にも保存する。

局所protocol harnessのhuman fixtureはfake providerの明示回答だけを扱う。
実agentには代理回答せず、`LVK_DEV_SERVER_REAL_AGENT=approval` は正式な承認待ちへの到達後、
試験runをCancelして終了する。`=1` の呼出し成功試験とは別である。
UI browser描画は未確認。既存canonical UIモデルのVitestも依存未導入（vitest not found）で未実施。

LOCAL-204/203との合成では `codex/client.rs` の既存変更を保持する。
今回の必要差分はmodule/state初期化、MCP elicitation分岐の委譲、invalid elicitationのCancel、
notificationでのMCP item追跡だけである。OpenWiki preflight/allowlist、command/file承認、
設定API、他のserver-request分岐を206の内容で上書きしない。

## 変更path

- `crates/executors/src/executors/codex/client.rs`
- `crates/executors/src/executors/codex/client/mcp_approval.rs`
- `crates/executors/src/executors/codex/client/mcp_approval/tests.rs`
- `crates/local-deployment/src/agent_process_registry.rs`
- `crates/local-deployment/src/container.rs`
- `crates/mcp/src/task_server/tools/dev_server.rs`
- `crates/mcp/src/task_server/tools/mod.rs`
- `crates/mcp/src/task_server/tools/stdio_tests.rs`
- `crates/server/src/middleware/model_loaders_tests.rs`
- `crates/server/src/remote_mcp/dev_server.rs`
- `crates/server/src/remote_mcp/dev_server_tests.rs`
- `crates/server/src/remote_mcp/mod.rs`
- `crates/server/src/remote_mcp/runtime_write.rs`
- `crates/server/src/remote_mcp/runtime_write_tests.rs`
- `crates/server/src/routes/execution_processes.rs`
- `crates/server/src/routes/workspaces/dev_server.rs`
- `crates/server/src/routes/workspaces/execution.rs`
- `crates/server/src/routes/workspaces/mod.rs`
- `crates/services/src/services/container.rs`
- `docs/development/managed-dev-server-mcp-evidence.json`
- `docs/development/managed-dev-server-mcp.md`
- `docs/integrations/vibe-kanban-mcp-server.mdx`
- `tests/server/run-managed-dev-server.mjs`

## 局所結果

- selected lib tests: 18 passed / 0 failed / 1 ignored（隔離fixtureは別実行）。
  新しいprotocol harnessは19分岐を1 testで検証する。承認/拒否/cancel/timeout、
  thread/turn/引数/tool不一致、一般form/URL、invalid schema、曖昧なitem、
  pending/完了後replay、Noop/自動承認設定、失効/対象差替えを含む。
- 修正前の実agent成功試験: failed。理由とeventは上記原証拠参照。
- 既存canonical UI model Vitest: not-run（実行入口が `vitest not found`）。
- 作業途中のfixture checksum/仮worktree/compile不一致、および追加testのcompile誤りは修正済み。
  最終結果と混同しない。lib試験中に出たmust_use警告もfixtureの戻り値を明示破棄して修正した。

承認接続の最初の実agent再試験ではapproval_requested自体とitem対応を観測したが、
pending件数通知が不足しrun状態がrunningのままだった。設定済み180秒上限を延長せず、
既存の `log_tool_approval_count` を共用する最小修正で待機状態の経路を揃えた。

待機状態通知の不足による試験は180秒でfailedとなった。失敗時の旧fixtureはagentを残したため、
正確なLVK_INSTANCE_DIRで所有を確認したprocess-hostだけへSIGTERMを送り、
同instanceの全process不在を `/proc` で再確認した。通常dev/previewへの操作はない。
後続fixtureはtimeout時も正式なrun Cancelを送るようcleanupを補正した。
最小修正後、executor protocol/command-file回帰2件は再度passed。

最小修正後の実agent隔離試験はpassed（19.70秒、AgentRun
`31067fda-b11a-48ac-bd58-0559bd015fe4`）。正式なawaiting_approval、実tool itemへの対応、
承認回答がないことを確認し、正式なAgentRun Cancelで終了した。
同fixtureの実stdio catalog/start/status/stop、remote HTTP scope/target/receipt/concurrent-start、
UI共通状態とcleanupもpassed。process残存なしを確認して成功instanceを削除した。
秘密値除外のevent ID・approval ID・tool item ID・artifact SHAはevidence JSONの
`post_fix_approval_reachability` に保存する。
fixtureの出力 `no permission response sent` は人間の承認回答を送っていないという意味であり、
cleanupに伴うCancelプロトコル応答まで不存在と主張するものではない。
人間承認後のtool-call成功とbrowser描画は未検証のまま。

最終Clippy: `node scripts/dev-cargo.mjs clippy -p server -p mcp -p executors -p local-deployment -p services --all-targets -j 4 -- -D warnings` passed。
`fmt --all -- --check`、Node syntax、`git diff --check` もpassed。
全3 fixture instanceのprocess残存なしを最終 `/proc` 照合で確認した。
semantic draftはAgentRun `9541ad33-4a27-43eb-aa69-3e87918798d1.json` へ保存し、
正式ManifestのGit identity導出はLVKの終了処理へ委ねる。
