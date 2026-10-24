---
type: architecture
title: Agent Runtime の監査・投影・回復
description: AgentRun を独立プロセスで実行し、Native Audit から永続状態と画面表示へ変換する境界。再接続、DB 競合、投影劣化時の契約を説明する。
tags: [agent-runtime, audit, persistence, recovery]
verified:
  - by: openwiki/0.5.1
    at: 2026-10-24T17:12:45.140Z
sources:
  - id: openwiki-source-9bc71fe4f6cbbaa6fa9168e1
    resource: repo://crates/db/src/models/agent_runtime_kanban_tests.rs
  - id: openwiki-source-d20a82e2192a07c687b838cb
    resource: repo://crates/db/src/models/agent_runtime.rs
  - id: openwiki-source-22b82518475034ade38104e6
    resource: repo://crates/executors/src/executors/codex.rs
  - id: openwiki-source-d0244e39b6cb88790907b9b2
    resource: repo://crates/executors/src/executors/codex/agent_scope.rs
  - id: openwiki-source-c88a7c6fabdac15166554032
    resource: repo://crates/executors/src/executors/codex/client.rs
  - id: openwiki-source-f4e0f9b2ce513d3f8b8b070e
    resource: repo://crates/executors/src/executors/provider_adapter.rs
  - id: openwiki-source-351583963abfe37bbf137f7e
    resource: repo://crates/executors/src/runtime/contracts.rs
  - id: openwiki-source-2312e3967b9fd7f73e349d9a
    resource: repo://crates/executors/src/runtime/native_audit.rs
  - id: openwiki-source-788fa2f6f1a3d449a1a9efe4
    resource: repo://crates/local-deployment/src/agent_run_port.rs
  - id: openwiki-source-8fd7f4fe2d73e778cc63c256
    resource: repo://crates/local-deployment/src/process_host.rs
  - id: openwiki-source-5a8e26da42457b4b47bca3ee
    resource: repo://crates/local-deployment/src/process_host/approvals.rs
  - id: openwiki-source-5d78398344c565f028fc5984
    resource: repo://crates/local-deployment/src/process_host/journal.rs
  - id: openwiki-source-7151ac14cb4570c5c04781dd
    resource: repo://crates/services/src/services/agent_runtime.rs
  - id: openwiki-source-024bbfcff36a8b1f88b9bcf3
    resource: repo://crates/services/src/services/approvals/executor_approvals.rs
  - id: openwiki-source-7d788a7c8b1c5bf5122bab53
    resource: repo://crates/utils/src/assets.rs
  - id: openwiki-source-06fe728b6644f76e4e63a8f4
    resource: repo://crates/utils/src/instance.rs
  - id: openwiki-source-9b65aee0d6accda7142f90a7
    resource: repo://crates/utils/src/native_audit.rs
  - id: openwiki-source-991414672d835fda0eead290
    resource: repo://crates/utils/src/process.rs
  - id: openwiki-source-7272325fd3387a1cecfd5cbc
    resource: repo://crates/utils/src/process/owned_group.rs
  - id: openwiki-source-965b490de6f8364cb7695944
    resource: repo://crates/workspace-manager/src/workspace_manager.rs
  - id: openwiki-source-11be03870aae85a259cab5c1
    resource: repo://docs/future/agent-runtime/architecture.md
  - id: openwiki-source-bb8ec0d0f300d3d2dd8edfc6
    resource: repo://docs/future/agent-runtime/data-planes-and-recovery.md
  - id: openwiki-source-37db6fa2c961da1810d195e7
    resource: repo://docs/future/agent-runtime/README.md
  - id: openwiki-source-141584dac09cf0f93be6bff5
    resource: repo://tests/codex-process-cleanup/README.md
generated: { by: "codex", at: "2026-10-24T17:12:45.140Z" }
---

# Agent Runtime の監査・投影・回復

Agent Runtime は、[Session 内の AgentRun](../concepts/session-and-agent-run.md) を実プロセスとして動かし、その観測結果を製品が判断できる状態へ変換する。provider 固有の通信は adapter が担当し、Workflow は [AgentRunPort を通して実行を依頼](workflow-runtime.md)する。

## 起動と所有者

`LocalAgentRunPort::reserve` は Workspace の lifecycle lock を保持して request と attempt の整合性、Workspace を検証し、実行 ID を DB に保存する。Setup の完了待ちなどでは、予約と起動を分けられる。archive 済み・worktree 削除済みなら明示再開前の起動を拒否し、[手動 archive](../concepts/workspace.md#archive削除期限-cleanup)と予約の競合を防ぐ。[Workspace 検証](../../crates/local-deployment/src/agent_run_port.rs#L734-L753)`launch_reserved` は既に Pending でなくなった実行を重複起動せず、現在の状態を返す。[起動境界](../../crates/local-deployment/src/agent_run_port.rs#L306-L344)

`agent-process-host` は一つの provider process と Native Audit を所有する独立ホストである。アプリ側は認証付き loopback endpoint に再接続し、保存済み host-event cursor の後から観測を取り込む。このため、サーバーとの接続断をそのまま provider の終了と扱ってはいけない。[ホストの契約](../../crates/local-deployment/src/process_host.rs#L1-L5)

Starting の canonical event 保存は provider launch の前提であり、書込失敗時は起動せずに戻る。同一 attempt の再配送では event ID と保存済み時刻を再利用する。初回 input event ID は従来互換を保ち、Retry の input event ID は attempt と message から生成するため、同じ本文でも別 attempt と衝突しない。[起動前の保存](../../crates/local-deployment/src/agent_run_port.rs#L2563-L2616)・[input identity](../../crates/local-deployment/src/agent_run_port.rs#L585-L605)

## Codex の承認待機を所有する場所

Codex の command/file approval waiter は provider と同じ process host に置く。`ProcessHostApprovals` は既存 `ExecutorApprovalBridge` を DB・通知 service なしで再利用し、UI への記録は canonical event 経路に任せる。launch audit の `approval_behavior` も Codex では `host_tool_waiter`、他 provider では `noop` と区別する。人の判断と Plan/質問の違いは [Session の承認契約](../concepts/session-and-agent-run.md#承認質問と現在の接続状態)を正本とする。[host wrapper](../../crates/local-deployment/src/process_host/approvals.rs#L1-L82)・[bridge](../../crates/services/src/services/approvals/executor_approvals.rs#L40-L83)・[launch audit](../../crates/local-deployment/src/agent_run_port.rs#L190-L214)

UI の ResolveApproval は host の waiter を解決する。Codex では port 側で解決 event を先行生成せず、request handler が待機結果（timeout・取消を含む）から `approval_resolved` を出す。`approval_requested` / `approval_resolved` と未解決件数の通知を adapter が canonical に投影し、件数が残る間は AwaitingApproval、0 で Running にする。native request ID への応答と semantic control の監査記録は異なるものであり、UI の回答受付だけを native 操作の実行成功と扱わない。[制御配送](../../crates/local-deployment/src/agent_run_port.rs#L3318-L3367)・[client の結果記録](../../crates/executors/src/executors/codex/client.rs#L971-L1092)・[件数の投影](../../crates/executors/src/executors/provider_adapter.rs#L1522-L1535)

Host の監視 loop を離れた際は、明示 Cancel でなく EOF・crash・protocol failure・通常終了でも cancellation token を cancel して承認 task を解放する。その後に既存 process cleanup、control 解放、出力 drain を行う。waiter の配置はアプリとの再接続を意図したものだが、この変更の fixture 試験はサーバー再起動越しの回復まで証明しない。[終了処理](../../crates/local-deployment/src/process_host.rs#L1061-L1085)・[検証の境界](../concepts/session-and-agent-run.md#承認の検証範囲)

## 四つのデータ経路

| 経路 | 保存先と役割 | 消失・再接続時の扱い |
| --- | --- | --- |
| Native Audit | 生の入出力フレームと manifest | adapter の再解釈や完了証拠の原資料 |
| Live | メモリ上の通知と WebSocket の `live` | 表示中の文字列差分。永続履歴を保証しない |
| Canonical | SQLite の `agent_events` と `agent_run_state` | 完了メッセージ、状態遷移、制御など、製品判断用の永続記録 |
| Snapshot | `agent_run_usage_snapshots` など | 累積使用量等の最新値。通知回数分を加算しない |

この分離の記録された目的は、表示用 token delta によって SQLite を二つ目の完全監査ログにしないことである。[データ経路の設計文書](../../docs/future/agent-runtime/data-planes-and-recovery.md)は現行の batch・snapshot 実装と対応する。`future/` というディレクトリ名だけで未実装文書と判断しない。

stdout は Native Audit へ記録してから分類・投影する。Audit の書き込み失敗、解釈が必須のフレームの失敗は通知して読み取りを打ち切る。既知の audit-only 通知と任意の未知通知には canonical event を作らない。[監査優先の処理](../../crates/local-deployment/src/process_host.rs#L741-L778)、[任意通知の扱い](../../crates/executors/src/executors/provider_adapter.rs#L2041-L2050)

Codex のメッセージ差分と完成メッセージは別の経路を通る。画面は完成した永続メッセージで一時表示を収束させる。再接続を「全 delta の再送」として実装しない。[Web 側の同期](web-and-sync.md)も参照。

## Context と子観測の投影

Codex の最新 context は `tokenUsage.last.totalTokens` と `modelContextWindow`、累積 usage は `total.inputTokens` / `total.outputTokens` から別々に得る。context の任意 field は既存 snapshot JSON に入り、host journal と DB の latest-value snapshot を通って replay される。context 用の専用 table や canonical token 履歴は増やさない。Workspace 累積トークンは別の thread 単位集計として保存する。[分類](../../crates/executors/src/executors/provider_adapter.rs#L1632-L1658)・[JSON 保存](../../crates/db/src/models/agent_runtime.rs#L1861-L1907)・[journal の選別](../../crates/local-deployment/src/process_host/journal.rs#L156-L175)。画面での未知値・新しい attempt の扱いは [Session の context 表示](../concepts/session-and-agent-run.md#context-使用量の意味)を参照する。

現行 Codex mapper は v5 で、`AgentScope` が native thread の親子関係と観測を canonical AgentActivity へ投影する。root は開始・再開応答で確定し、子の通知や scope 不明の usage を親の会話・使用量にしない。旧 v4 mapper は旧 collaboration 投影の replay 用に残す。既存 canonical 履歴を新しい観測で推測補完する仕組みではない。[mapper 選択](../../crates/executors/src/executors/provider_adapter.rs#L532-L553)・[scope](../../crates/executors/src/executors/codex/agent_scope.rs#L24-L200)。子の終端証拠を要求する実行側 guard と表示 lifecycle は [native subagent](../concepts/session-and-agent-run.md#native-subagent-と旧実行モデル)に置く。

## Host の再送記録と有界配信

Host は SQLite と別に `runtime/host-events/<attempt>-<host>.v1.jsonl` を持つ。これは再接続用の意味的 batch journal であって、raw delta の第二監査ではない。文字列 Live delta は記録から外し、usage snapshot は残す。128件・256 KiB・50 ms を契機に grouped flush し、Started / Terminal は直ちに同期保存する。未保存の cursor を consumer へ公開しない。[journal の保存](../../crates/local-deployment/src/process_host/journal.rs#L156-L238)、[周期 flush](../../crates/local-deployment/src/process_host.rs#L566-L584)

再送ページは128件／4 MiBを目安に制限し、直近 Live は256件／4 MiBの ring に置く。producer 側も件数・bytes による backpressure を持つ。単一の過大な意味的 event は黙って切り詰めず失敗させ、原文は Native Audit に残す。一方、journal の batch offset 索引は実行時間とともに増えるため、全メモリが定数という保証ではない。[上限](../../crates/local-deployment/src/process_host/journal.rs#L17-L23)、[producer の byte budget](../../crates/local-deployment/src/process_host.rs#L791-L815)

protocol v2 の Subscribe は cursor から追いつき、通知と heartbeat を契機に journal を読み直す。制御用接続を長い購読と分け、遅い購読先への書込も有限時間に制限する。旧 Host の Attach は互換経路として残り、未知の protocol を推測して replay しない。[Subscribe](../../crates/local-deployment/src/process_host.rs#L691-L720)

## 接続断と確認済み終了を分ける

複数回の通信失敗だけでは死亡としない。Host の開始 identity を照合して終了を確認した後、journal の checksum・連番・run identity と、閉じた Native Audit に対する参照を検証してから再生する。provider / process group が生存中または不明なら終端成功を採用せず、観測・正規 Cancel の対象として残す。代替 provider を勝手に起動しない。[回復入口](../../crates/local-deployment/src/agent_run_port.rs#L1497-L1555)、[子と終端の確認](../../crates/local-deployment/src/agent_run_port.rs#L1580-L1653)

provider の「turn 完了」と Host の終了は別である。成功を最終化する証拠は、出力 drain と Audit close 後の Host Terminal。未完了 journal tail を除いた再生は完全回復とは呼ばず、終端証拠のない死亡を成功へ昇格させない。この順序は [OpenWiki publication](../operations/openwiki-maintenance.md) が閉じた監査を読むためにも必要である。[Audit と terminal](../../crates/local-deployment/src/process_host.rs#L1090-L1166)

### 終了時の stdout 所有権と drain

Codex adapter は実 stdout を JSON-RPC transport に渡し、Host が読む置換 pipe の writer を `LogWriter` に持たせる。`DeferredCodexControl → AppServerClient → LogWriter` という参照が残ると、provider の cleanup 後も Host 自身が writer を保持し、EOF を待てなくなる。`monitor_process` は制御 loop と既存の process cleanup を終えた後、出力 drain の前に `drop(control)` する。この順序は不要になった所有参照を解放するためであり、cleanup を省略するものではない。[pipe と control の生成](../../crates/executors/src/executors/codex.rs#L1342-L1403)・[writer の所有](../../crates/executors/src/executors/codex/client.rs#L2041-L2064)・[解放位置](../../crates/local-deployment/src/process_host.rs#L1061-L1085)

drain の上限は5秒のままで、残った出力の投影・監査失敗を処理してから Audit close と Host Terminal へ進む。drain timeout 自体は警告を記録し provider terminal status を維持するため、timeout と AuditFailure を同一視しない。EOF 所有権の修正は、上限短縮や画面の先行した送信再開とは異なる。Linux の owned group 停止には SIGINT / SIGTERM 後に各最大2秒の猶予があるが、捕捉した全プロセスの終了を確認すれば早く抜ける。固定4秒の待機ではなく、最後の再走査と leader 回収も必要である。詳細は [group 所有権](#linux-の-owned-process-group)を参照する。[drain と終端保存](../../crates/local-deployment/src/process_host.rs#L1085-L1204)・[drain 定数](../../crates/local-deployment/src/process_host.rs#L68)・[停止の猶予](../../crates/utils/src/process/owned_group.rs#L79-L160)

Unix の `terminal_drain_releases_control_owned_output` は、control に実 OS pipe の writer を保持させ、最終 bytes が監査へ入り、2秒未満で終端へ到達し、Audit が Complete になることを検査する。provider 完了証拠のない exit 0 は Crashed、非ゼロ終了は Failed のままであり、所有権解放が成功条件を弱めないことも確認する。[回帰試験](../../crates/local-deployment/src/process_host.rs#L1603-L1717)。実 backend から次の Send までの検証範囲・測定限界は [terminal 回帰試験の運用](../operations/development.md#terminal-から次操作までの回帰試験)に置く。

### OS 終了より遅い provider 終端証拠

高速な CLI では OS の exit が最終 stdout frame より先に監視 loop へ届く。drain 中に `ProviderTerminal` が届いた場合、現在の原因が正常な `Process` 終了のときだけ、その provider status を採用する。exit 0 だけでは成功にせず、既に選んだ取消・監査エラー・非ゼロ終了を後着の成功で上書きしない。drain 内の監査・protocol failure は引き続き終端原因となり、Audit close と Host Terminal の保存順も維持する。[採用条件と最終化](../../crates/local-deployment/src/process_host.rs#L1073-L1183)

`process_exit_before_terminal_output_preserves_provider_evidence` は、process 終了の選択後に通知を送る handshake で順序を固定し、exit 0＋provider success、exit 0＋provider failure、exit 1＋provider success と取消を検査する。Linux では wrapper が残した子の終了と無関係なプロセスの生存も確認する。これは shared host の契約であり、[Claude subscription](../integrations/agent-providers.md#claude-のサブスクリプション専用-profile)だけの別 scheduler ではない。[順序の回帰試験](../../crates/local-deployment/src/process_host.rs#L1433-L1601)

## 制御入力の送信と監査の順序

初回 launch は canonical input と launch payload を Audit に保存してから provider を起動する。一方、活動中の制御は `control_peer.send`、または `stdin.write_all` / `flush` が先で、返された非空 bytes を後から `append_native_input` へ保存する。後段の保存に失敗すると要求へエラーを返し、`AuditFailure` による終了・cleanup へ進む。**AuditFailed や該当フレームの不在は「未送信」「副作用なし」の証明にならない。** [初回起動](../../crates/local-deployment/src/process_host.rs#L333-L360)・[制御と失敗](../../crates/local-deployment/src/process_host.rs#L941-L1011)

耐久 command service は command を DB に enqueue してから配送するが、その記録と provider へ送った生 bytes の監査は別である。再試行時は command の状態に加えて実行状態・作業木も確認する。[command の事前保存](../../crates/services/src/services/agent_runtime.rs#L106-L145)

[Runtime README](../../docs/future/agent-runtime/README.md) は「計画承認済み・実施開始待ち」、[目標設計](../../docs/future/agent-runtime/architecture.md) は Draft と明記する。そこに記録された「native input を保存してから送信する」方針は、この制御経路の現在の順序とは一致しない。設計自身も既発生の作業木変更を rollback する保証はしない。これは静的な配線確認であり、監査ディスク障害の再現試験結果ではない。[記録された順序・失敗方針](../../docs/future/agent-runtime/architecture.md#写入顺序与失败策略)

## Native Audit の保全・エクスポート・寿命

Audit の本体は SQLite の外、`<asset_dir>/runtime/native-audit/v1/sessions/<Session UUID先頭2文字>/<Session UUID>/agent-runs/<AgentRun UUID>/attempts/<RunAttempt UUID>/` にある `manifest.json` と `frames.jsonl` である。`LVK_INSTANCE_DIR` が設定されていれば asset_dir はその `data` 配下を優先する。未設定の場合だけ debug は repository の `dev_assets`、release は OS のアプリデータ領域となる。build profile だけをデータ隔離の境界にしない。SQLite だけの保全では原文を保存できない。[パス構築](../../crates/utils/src/native_audit.rs#L7-L31)・[asset root](../../crates/utils/src/assets.rs#L6-L29)

フレームは payload の無損失 base64 と checksum を保持する。プログラム上の `AuditBundle::export` は bundle を検証してから manifest / frames と、存在すれば fixture の期待イベントをそのままコピーする。脱敏された表示ログではなく、送受信した本文を含む原資料なので、共有する bundle の内容もその単位で確認する。[payload](../../crates/executors/src/runtime/native_audit.rs#L175-L213)・[読取検証](../../crates/executors/src/runtime/native_audit.rs#L849-L910)・[export](../../crates/executors/src/runtime/native_audit.rs#L922-L943)

読取は Complete / Recovered の manifest、連番、frame 数、checksum を検査し、open・不完全な bundle を拒否する。replay は audit schema / runtime / adapter / protocol / mapper の version を照合し、不一致をエラーにする。既存の bundle replay は、下記の将来作業である「製品 DB の完全再投影」と同じ操作ではない。[version 契約](../../crates/executors/src/runtime/native_audit.rs#L945-L964)・[比較](../../crates/executors/src/runtime/native_audit.rs#L1103-L1142)・[不一致テスト](../../crates/executors/src/runtime/native_audit.rs#L1317-L1345)

[Runtime 計画の「周期 cleanup を追加しない」という保存方針](../../docs/future/agent-runtime/README.md#已确认的产品边界)と、利用者の明示削除は分ける。[Workspace 削除](../concepts/workspace.md#archive削除期限-cleanup)では Session ID 群を取得し、DB 削除後の background cleanup が各 Session の Native Audit と process log を削除する。External の作業ディレクトリ保護も、この別保存先の永久保持を意味しない。必要な監査は削除前に保全する。cleanup 失敗はログに残るため、削除 API の受付だけで消去完了とも判定しない。[削除 context](../../crates/workspace-manager/src/workspace_manager.rs#L174-L205)・[cleanup](../../crates/workspace-manager/src/workspace_manager.rs#L436-L515)・[削除と再実行のテスト](../../crates/workspace-manager/src/workspace_manager.rs#L1463-L1492)

## DB の原子性と冪等性

一つの host response の永続イベント、provider session 観測、投影、usage snapshot、cursor 更新を同じ transaction に収める。canonical sequence は密な連番を割り当て、生フレームや host の sequence とは区別する。cursor だけ先へ進めると再接続時に未反映データを失う。[batch の入口](../../crates/db/src/models/agent_runtime.rs#L1479-L1554)、[cursor と commit](../../crates/db/src/models/agent_runtime.rs#L1772-L1793)

RunAttempt の usage snapshot は RunAttempt ごとに native sequence が新しい場合だけ置き換える。同じ sequence に異なる内容が来ると冪等性の衝突として拒否し、古い通知は無視する。[usage 更新](../../crates/db/src/models/agent_runtime.rs#L1861-L1907)

### Workspace 累積値の保存境界

`TokenUsageSnapshot.cumulative_usage` は、Codex native 通知の thread identity と total の整合性を adapter が確認できた場合だけ付く任意 field である。旧 snapshot は欠落したまま deserialize でき、汎用 usage を累積と推定しない。[適合判定](../../crates/executors/src/executors/provider_adapter.rs#L1969-L2025)・[互換 field](../../crates/executors/src/runtime/contracts.rs#L411-L440)

DB はこの情報を再検証し、`workspace_token_usage` の thread 単位最大値を既存 snapshot と host cursor と同一 transaction で保存する。snapshot の同一 sequence 内容衝突や cursor identity 不一致なら commit しない。thread の total 比較と RunAttempt snapshot の sequence 比較は別の更新条件であり、後者の最新値を全 attempt 分加算してはならない。[保存・衝突・commit](../../crates/db/src/models/agent_runtime.rs#L1797-L1928)。集計の意味、欠損内訳・取消・reset の扱いは [Workspace 累積トークン](../concepts/workspace.md#workspace-累積トークン)を正本とする。

### Canonical event から Board への副作用

Kanban 同期は単一 event と host batch の両保存経路で、reducer が `Applied` を返したときにだけ同じ transaction 内で呼ぶ。event・投影・Board 更新（host batch では cursor も）は一緒に commit / rollback する。projection rebuild や OS exit に別の列更新を足すと、この境界と再送保護を壊す。[単一保存](../../crates/db/src/models/agent_runtime.rs#L2037-L2042)・[batch 保存](../../crates/db/src/models/agent_runtime.rs#L1699-L1705)・[rollback / replay 回帰](../../crates/db/src/models/agent_runtime_kanban_tests.rs#L325-L381)

開始の入力根拠は provider spawn 後の Host Started。port はその timestamp を process registry に保存してから、同じ時刻の canonical Running を作る。registry 保存と canonical batch は別段階なので、両者全体を単一 transaction と説明しない。回復テストは再送後も開始時刻を保持することを確認する。[Started の生成](../../crates/local-deployment/src/process_host.rs#L436-L455)・[取り込み](../../crates/local-deployment/src/agent_run_port.rs#L2065-L2122)・[回復の検証](../../crates/local-deployment/src/agent_run_port.rs#L4509-L4537)。列の意味、開始時刻による競合判定、手動変更優先は [Kanban と実行の同期契約](../concepts/project-and-issue.md#kanban-と実行の同期契約)に集約する。

## 障害を区別する

| 障害 | 処理 |
| --- | --- |
| 一時的な SQLite 競合 | 有限回 retry 後に Unavailable。恒久的な投影劣化へ変換しない |
| 決定的な契約・reducer エラー | batch を観測単位に分け、失敗観測を `agent_projection_failures` に記録 |
| DB 基盤の障害 | その失敗を隔離済みとして cursor を進めない。次の attach で再取得する |

隔離時は理由、`projection_degraded`、当該観測の cursor 前進を原子的に保存し、後続観測の取り込みを継続できる。[分離処理](../../crates/local-deployment/src/agent_run_port.rs#L1969-L2051)、[失敗台帳](../../crates/db/src/models/agent_runtime.rs#L234-L310)

投影が Current でない場合、通常制御を拒否するが、Cancel と下記の証拠付き Retry の経路は残す。終端実行の Cancel は冪等な無操作である。[制御ガード](../../crates/local-deployment/src/agent_run_port.rs#L3210-L3255)。この前段の [実行専用 Workspace の所有者検証](../concepts/workspace.md#操作用途と実行所有者)は独立した条件である。

### 明示 Retry に限る startup collision の回復

耐久 command と owner の権限を確認した明示 Retry だけが、既知の startup input ID 衝突を再構築できる。対象は二回目以降の最新 terminal attempt、特定の単一隔離記録、終了確認済み Host/provider、完全な journal と閉じた Native Audit、元の immutable input と一致する証拠を備えるものに限定する。観測処理が自動で degraded flag を消すことはない。[証拠検査](../../crates/local-deployment/src/agent_run_port.rs#L1519-L1710)

欠けた input / Running の事実を冪等に追加し、canonical history 全体を reducer で再生する。再構築後は投影状態と更新時刻以外の意味状態が以前と一致しなければ拒否する。terminal run を Running に戻したり、隔離・監査記録を消したりしない。未知の失敗、改変、未完了 tail、生存不明の process はこの回復の対象外である。[追加と再構築](../../crates/local-deployment/src/agent_run_port.rs#L1710-L1758)・[原子的な照合](../../crates/db/src/models/agent_runtime.rs#L1254-L1340)

この一般 Retry の互換処理と、[正式 Integration の限定 checkpoint adoption](../concepts/formal-integration.md#旧-run-の-checkpoint-adoption)は別である。後者は歴史的 Agent を再実行せず、承認済み Git prefix を引き継ぐ。

## 取消と実プロセスの終了

host は native interrupt の前に子プロセスを捕捉し、送信できた場合は最大2秒、provider 自身による tool cleanup を待ってから transport を閉じる。native interrupt が利用不能でも取消を中断せず、process group と捕捉した子の終了を並行して試みる。両方の cleanup が成功して初めて Cancelled とし、待機失敗は Crashed になる。取消応答も監査の終了と terminal 記録の後で返す。[取消経路](../../crates/local-deployment/src/process_host.rs#L941-L1042)、[完了応答](../../crates/local-deployment/src/process_host.rs#L1090-L1166)、[失敗状態](../../crates/local-deployment/src/process_host.rs#L1227-L1241)

Linux では別 session に移る tool も扱うため `/proc` から子孫を列挙し、親と開始時刻を再照合した pidfd を保持する。PID 再利用で無関係なプロセスを停止しないための仕組みであり、捕捉後の daemon 化まで閉じ込める sandbox ではない。Linux 以外ではこの子孫捕捉は空の処理となり、既存 process group cleanup に依存する。[捕捉の範囲](../../crates/utils/src/process.rs#L8-L83)、[終了確認](../../crates/utils/src/process.rs#L85-L136)。独立 session の子だけを停止する [focused test](../../crates/utils/src/process.rs#L220-L259)と、応答しない native interrupt でも取消が有限時間で進む [test](../../crates/local-deployment/src/process_host.rs#L1863-L1899)がある。

### Linux の owned process group

`OwnedProcessGroup` は provider を起動した所有者が終了まで保持する識別情報であり、独立 supervisor ではない。共通 process host と [Codex discovery の owner task](../integrations/agent-providers.md#codex-discovery-の所有者と終了)で使う。Linux では `waitid(P_PIDFD, WNOWAIT)` で終了を観測して leader を未回収のまま保持し、PID・開始時刻・PGID・session と親を照合する。leader を先に reap してから数値 PGID だけに signal を送ると識別子再利用を防げないため、回収を cleanup の最後に置く。[所有と待機](../../crates/utils/src/process/owned_group.rs#L16-L53)・[identity 検査](../../crates/utils/src/process/owned_group.rs#L226-L290)・[非回収 wait](../../crates/utils/src/process/owned_group.rs#L326-L354)

host は Linux の自然終了でも group cleanup を省略しない。cleanup は anchor を照合して同じ group/session の member を `/proc` から再走査し、開始時刻を照合した pidfd だけへ signal を送る。SIGINT / SIGTERM の各最大2秒の猶予後、最大5秒で SIGKILL・再走査・終了確認を繰り返し、成功後に最大5秒で leader を reap する。観測時点の別 session の子孫も既存 snapshot で捕捉し、group と並行して停止する。これは将来の任意の daemon を閉じ込める保証ではなく、観測前に逃れて reparent された子孫は保証範囲外である。[host の自然終了](../../crates/local-deployment/src/process_host.rs#L888-L923)・[cleanup と回収順](../../crates/utils/src/process/owned_group.rs#L79-L160)

所有権の確立失敗・終了待機失敗は host の `WaitFailure` となり、`Crashed` へ分類する。group と子孫の cleanup が両方失敗した場合は両方の理由を保持する。未確認の所有権を drop しても自動回収は走らず、Linux では未完了のログを残す。OS kill や Tokio runtime 全体の破棄を越える回収保証ではない。非 Linux は従来の group cleanup を使い、自然終了後の追加 cleanup は導入しない。[host の失敗](../../crates/local-deployment/src/process_host.rs#L1040-L1083)・[分類](../../crates/local-deployment/src/process_host.rs#L1275-L1282)・[未完了 drop](../../crates/utils/src/process/owned_group.rs#L164-L173)・[検証限界](../../tests/codex-process-cleanup/README.md#ownership-and-limits)

## 変更時の検証と未解決点

`host_batch_commits_events_projection_and_cursor_atomically` は密な sequence、重複受信、内容衝突時の event・cursor・provider session の rollback を検証する。[テスト](../../crates/db/src/models/agent_runtime.rs#L2392-L2512)を、mapper や永続化単位の変更時に確認する。

完全な canonical projection を Native Audit から再構築する管理操作は、[回復文書](../../docs/future/agent-runtime/data-planes-and-recovery.md#compatibility-and-future-work)では将来作業とされる。失敗台帳が存在することを自動修復の保証と解釈しない。古い mapper と履歴の互換性を含む設計意図は [Runtime 目標設計](../../docs/future/agent-runtime/architecture.md)に記録されており、個別機能の実装有無は現行 adapter と照合する。

### Codex cleanup の検証範囲

[隔離回帰の説明](../../tests/codex-process-cleanup/README.md)は Python による wrapper-only 死亡の機構再現、Linux primitive の確認、実 Rust の owner/RPC/host 経路を区別する。utils の [direct-drop 再現と owned orphan cleanup](../../crates/utils/src/process/owned_group.rs#L358-L470)、host の [自然終了・取消 fixture](../../crates/local-deployment/src/process_host.rs#L1433-L1601)、既存の drain・native interrupt 試験が変更時の入口となる。実 WebSocket router 切断、実 provider/model、非 Linux、runtime 全体の破棄はこの fixture 群の検証結果に含めない。過去に消えた PID の発生源も確定していない。

今回の Sync は凍結入力全4チャンクの hash と統合 source `90a48e66f19509f44aab65900ccbc0462efdef04` を照合した。discovery の WebSocket・scope 解決、model/skill probe、RPC reader、共通 host の自然終了・取消・drain、utils の identity・子孫 snapshot、独立 fixture 定義から対象を確認した。凍結 Change Manifest と上記検証記録は開発時の focused Rust 成功（utils 5、discovery 1試験内4ケース、host 10）を報告するが、最終 Integration は明示的に検証省略されている。この文書保守は静的照合であり、製品テストの再実行や無関係な Wiki 全体の再検証ではない。
