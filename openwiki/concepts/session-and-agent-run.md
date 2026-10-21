---
type: concept
title: Session・AgentRun・RunAttempt
description: 会話・依頼・実行試行の識別、継続と retry、provider binding、Goal と停止の契約。
tags: [session, agent-run, run-attempt, goal, lifecycle]
sources:
  - id: openwiki-source-d20a82e2192a07c687b838cb
    resource: repo://crates/db/src/models/agent_runtime.rs
  - id: openwiki-source-3e5425718d87b437a12edd60
    resource: repo://crates/db/src/models/session.rs
  - id: openwiki-source-13210275fc6cb3d346bc25ae
    resource: repo://crates/executors/src/approvals.rs
  - id: openwiki-source-22b82518475034ade38104e6
    resource: repo://crates/executors/src/executors/codex.rs
  - id: openwiki-source-d0244e39b6cb88790907b9b2
    resource: repo://crates/executors/src/executors/codex/agent_scope.rs
  - id: openwiki-source-c88a7c6fabdac15166554032
    resource: repo://crates/executors/src/executors/codex/client.rs
  - id: openwiki-source-35b6fb5835c638575801a2b6
    resource: repo://crates/executors/src/executors/codex/goal_lifecycle.rs
  - id: openwiki-source-f4e0f9b2ce513d3f8b8b070e
    resource: repo://crates/executors/src/executors/provider_adapter.rs
  - id: openwiki-source-f07f62eba81c4ffbaf0db9d5
    resource: repo://crates/executors/src/profile.rs
  - id: openwiki-source-dc7ce5cfcb903cb959653602
    resource: repo://crates/executors/src/profile/runtime_identity.rs
  - id: openwiki-source-351583963abfe37bbf137f7e
    resource: repo://crates/executors/src/runtime/contracts.rs
  - id: openwiki-source-6e93b2f0738ee5fe4379942f
    resource: repo://crates/executors/src/runtime/reducer.rs
  - id: openwiki-source-788fa2f6f1a3d449a1a9efe4
    resource: repo://crates/local-deployment/src/agent_run_port.rs
  - id: openwiki-source-343f68138e6e2d3312d2774c
    resource: repo://crates/local-deployment/src/container.rs
  - id: openwiki-source-8fd7f4fe2d73e778cc63c256
    resource: repo://crates/local-deployment/src/process_host.rs
  - id: openwiki-source-9d3b3180c15d3125ddbb2675
    resource: repo://crates/server/src/remote_mcp/questions.rs
  - id: openwiki-source-b393e75029374eaf9cba5f7d
    resource: repo://crates/server/src/routes/integrations/runtime.rs
  - id: openwiki-source-8de115a31415ef9c911d6ac0
    resource: repo://crates/server/src/routes/scratch.rs
  - id: openwiki-source-406c792263884bbf462d666c
    resource: repo://crates/server/src/routes/sessions/agent_run.rs
  - id: openwiki-source-a74a882889154c2549e88f41
    resource: repo://crates/server/src/routes/sessions/executor_config.rs
  - id: openwiki-source-3a27c9d52911b8d12051a764
    resource: repo://crates/server/src/routes/sessions/mod.rs
  - id: openwiki-source-4d617b0e2fa188c6ad63ae56
    resource: repo://crates/server/src/routes/sessions/questions.rs
  - id: openwiki-source-7151ac14cb4570c5c04781dd
    resource: repo://crates/services/src/services/agent_runtime.rs
  - id: openwiki-source-9c5d335ecd1178e88c47c0a8
    resource: repo://crates/services/src/services/queued_message.rs
  - id: openwiki-source-28f588bd26222b42bea70c77
    resource: repo://docs/development/codex-execution-observations.md
  - id: openwiki-source-23db569cee004f42315c9f61
    resource: repo://docs/workspaces/sessions.mdx
  - id: openwiki-source-e4d0ff0d72ea9774e1af777c
    resource: repo://packages/ui/src/components/ChatElapsedTime.tsx
  - id: openwiki-source-a38f11830547e0c1d7e425e5
    resource: repo://packages/ui/src/components/ContextUsageGauge.tsx
  - id: openwiki-source-5ac2756548e33cbcc86886e2
    resource: repo://packages/ui/src/components/KeyboardCommandsPlugin.tsx
  - id: openwiki-source-84123960815ca9b2c23d8a0b
    resource: repo://packages/ui/src/components/MarkdownSyncPlugin.tsx
  - id: openwiki-source-583a89a619287948ed95b211
    resource: repo://packages/ui/src/components/SessionChatBox.tsx
  - id: openwiki-source-a46d1ceab41257b518aa5a4a
    resource: repo://packages/web-core/src/features/agent-runtime/model/canonicalAgentConversation.test.ts
  - id: openwiki-source-6f68befb805cadf03e4b569b
    resource: repo://packages/web-core/src/features/agent-runtime/model/canonicalAgentConversation.ts
  - id: openwiki-source-fe9d810dccb0b777dfcb7ae4
    resource: repo://packages/web-core/src/features/agent-runtime/model/canonicalAgentTimeline.ts
  - id: openwiki-source-f133883544522a7fa5fc57b2
    resource: repo://packages/web-core/src/features/workspace-chat/model/canonicalAgentControls.ts
  - id: openwiki-source-dd21d5cad7649302ce861cfc
    resource: repo://packages/web-core/src/features/workspace-chat/model/goalElapsed.ts
  - id: openwiki-source-14cc0373faba797a6e4d1fdd
    resource: repo://packages/web-core/src/features/workspace-chat/model/hooks/useSessionMessageEditor.ts
  - id: openwiki-source-08c30c5f079994cddbe25a3b
    resource: repo://packages/web-core/src/features/workspace-chat/model/hooks/useSessionSend.ts
  - id: openwiki-source-6b4d677fd9667cbdb6924409
    resource: repo://packages/web-core/src/features/workspace-chat/model/pendingSessionDraft.ts
  - id: openwiki-source-e3ad3a00ca485cc7e1958008
    resource: repo://packages/web-core/src/features/workspace-chat/model/sessionDraft.ts
  - id: openwiki-source-c294940cc86e5328c123d83c
    resource: repo://packages/web-core/src/features/workspace-chat/ui/GoalProgressCard.tsx
  - id: openwiki-source-f0616990372eebb94bc69e6f
    resource: repo://packages/web-core/src/features/workspace-chat/ui/SessionChatBoxContainer.tsx
  - id: openwiki-source-649882ab70d5bba1d79459a5
    resource: repo://packages/web-core/src/shared/components/CreateChatBoxContainer.tsx
  - id: openwiki-source-6d0dfae3c1582e1af83405d4
    resource: repo://packages/web-core/src/shared/dialogs/settings/settings/GeneralSettingsSection.tsx
  - id: openwiki-source-43d7a2f6bd06c556e7d48e3c
    resource: repo://packages/web-core/src/shared/hooks/useScratch.ts
  - id: openwiki-source-2e251355ae5f41eedc1e63cc
    resource: repo://packages/web-core/src/shared/lib/executionProcessRuntime.test.ts
  - id: openwiki-source-80f810d80e1c4664d02a9f39
    resource: repo://packages/web-core/src/shared/lib/executionProcessRuntime.ts
  - id: openwiki-source-bfcca8eb0acec96215c9bf83
    resource: repo://packages/web-core/src/shared/lib/executorOverrides.ts
  - id: openwiki-source-53a9c69af17738e7a2b63de9
    resource: repo://tests/chat/editor-display.test.mjs
  - id: openwiki-source-dea540ef1c25316603003cab
    resource: repo://tests/chat/input-operations.isolated.mjs
  - id: openwiki-source-adbcc4477d532869e93682ad
    resource: repo://tests/chat/isolated.spec.ts
  - id: openwiki-source-f2e3f1d703b1fe9ce39ce327
    resource: repo://tests/chat/run-elapsed.test.mjs
  - id: openwiki-source-a41f30f4cf117b514c91218b
    resource: repo://tests/run-elapsed/isolated.mjs
  - id: openwiki-source-854d9f0ac30502f5da093e06
    resource: repo://tests/run-elapsed/README.md
generated: { by: "codex", at: "2026-10-21T03:55:18.362Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-21T03:55:18.362Z
---

# Session・AgentRun・RunAttempt

**Session** は [Workspace](workspace.md) 内の会話単位である。同じ Workspace の Session はファイルを共有する一方、会話履歴は別である。**AgentRun** は一つの実行依頼の正本、**RunAttempt** はその依頼を provider で実行する一回の試行を表す。Session 自体を OS プロセスの寿命と同一視しない。[Session の意図](../../docs/workspaces/sessions.mdx)、[保存モデル](../../crates/db/src/models/agent_runtime.rs#L47-L108)

## 識別と固定するもの

| 単位 | 所有する関係・契約 |
| --- | --- |
| Session | Workspace、executor、会話名・作業ディレクトリ |
| AgentRun | Session、Workspace、依頼入力、provider/profile、相関 ID、冪等キー |
| Turn | run に属する依頼の intent と入力メッセージ |
| RunAttempt | run/turn を参照し、試行番号、transport、capability snapshot、設定、Skill、native session 参照を固定 |
| Provider session | provider 内部の会話識別子。LVK Session と同じ UUID ではない |

[Session モデル](../../crates/db/src/models/session.rs#L22-L37)、[依頼 envelope](../../crates/executors/src/runtime/contracts.rs#L222-L237)、[試行 envelope](../../crates/executors/src/runtime/contracts.rs#L267-L292)

Attempt 検証は run/turn/Session/correlation の一致だけでなく、provider、profile、Workspace、設定と capability snapshot の整合性を要求する。Review に selected Skills を付けることや、FollowUp 以外への reset 指定も拒否する。[契約検証](../../crates/executors/src/runtime/contracts.rs#L294-L351)。これらを永続化してから起動する機構と監査の詳細は [Agent Runtime](../architecture/agent-runtime.md) に置く。

## 実行設定の復元と DEFAULT identity

profile 比較では variant 省略と正確な `DEFAULT` だけを同一視する。保存済み ID の表記は変更せず、`PLAN`・小文字の `default`・異なる provider は混同しない。既存履歴を migration で書き換えるのではなく、binding と request の比較境界で共通規則を使う。[identity](../../crates/executors/src/profile/runtime_identity.rs#L1-L13)

Session の設定復元は最新 RunAttempt の immutable launch request を読み、現在の可変 preset や script 実行から推測しない。起動前の native adoption では保存済み binding を使う。最新の保存値が壊れていたらエラーであり、古い run や別 Session の設定へ黙って戻さない。モデル・reasoning・mode・Goal budget 等の保存値と、provider が実行時に解決した既定値は区別する。[復元](../../crates/server/src/routes/sessions/executor_config.rs)、[順序・破損テスト](../../crates/server/src/routes/sessions/executor_config.rs)

## Follow-up と retry

通常 follow-up 経路は Session に非終端の AgentRun があれば拒否し、queue の使用を案内する。別 Session の会話は分離できても、同じ Workspace のファイルは共有されるので、Session を増やすだけで作業ファイルは隔離されない。[起動 guard](../../crates/server/src/routes/sessions/mod.rs#L314-L348)、[共有の契約](../../docs/workspaces/sessions.mdx)

- **Follow-up** は同じ Session に新しい AgentRun を作る。保存された provider session があれば FollowUp intent、なければ Initial になる。明示的な native resume 指定は Resume mode、通常の続きは Launch mode でも provider session 参照を渡す。
- **Retry** は終端になった既存 AgentRun の同じ turn を維持し、新しい attempt ID と増加した試行番号で再起動する。mode は Resume または Restart。Resume は対応 capability と観測済み provider session を要求し、Restart はその参照を持ち越さない。

[follow-up 選択](../../crates/server/src/routes/sessions/mod.rs#L392-L439)、[retry の契約](../../crates/local-deployment/src/agent_run_port.rs)

Retry は前回の設定・capability snapshot・Skill を引き継ぐ。設定を変えて別の仕事を始める操作と混同しない。制御リクエストも command ID、冪等キー、相関 ID を持って永続 command service を経由する。[retry 固定値](../../crates/local-deployment/src/agent_run_port.rs)、[制御 API](../../crates/server/src/routes/agent_runs.rs#L348-L373)

Starting の保存は provider 起動の前提であり、保存失敗時は provider を起動しない。[起動 identity と再配送](../architecture/agent-runtime.md#起動と所有者)を参照する。[正式 Integration](formal-integration.md#競合だけを有限の-agent-job-にする) の競合処理は source ごとの新規 Create を使い、一般 Retry で次の source を実行する仕組みではない。

## Run の経過時間と Goal 会計

Board の最新 Run とチャットの各 Run footer の経過時間は、AgentRun の `created_at` から現在 attempt の保存済み `finished_at` までの壁時計時間である。活動中は現在時刻との差を表示する。実プロセスの開始時刻や CPU 使用時間ではなく、Pending、入力・承認待ち、Cancelling、同じ Run の retry を含む。Retry では起点をリセットせず、新しい Follow-up の Run とは合算しない。API と画面への経路は [Web の時間表示](../architecture/web-and-sync.md#run-の時間表示を同期状態から分ける)を参照する。[時刻の取得](../../crates/services/src/services/agent_runtime.rs#L201-L237)・[Run ごとの計算](../../packages/web-core/src/features/agent-runtime/model/canonicalAgentConversation.ts#L648-L679)

### 終了の証拠と再接続

永続化では attempt の `finished_at` が未設定のとき、終端 state の時刻を一度だけ保存する。後続イベントが projection の `updated_at` を進めても終了時刻を更新しない。したがって `updated_at` を終了時刻へ流用すると、完了済み Run の表示時間が増える。[batch 保存](../../crates/db/src/models/agent_runtime.rs#L1775-L1794)・[単一 event 保存](../../crates/db/src/models/agent_runtime.rs#L2099-L2119)・[遅延 event の回帰テスト](../../crates/db/src/models/agent_runtime.rs#L3210-L3267)

チャットの時計は summary と stream の state を attempt 番号、event sequence、更新時刻の順で比較する。古い running snapshot で完了時計を再始動せず、新しい retry attempt なら前 attempt の `completed_at` を使わない。終端では一致する attempt の summary 終了時刻を優先し、なければその attempt の terminal lifecycle event の timestamp を使う。どちらもなければ終了は不明のままとする。この比較は時間表示のためであり、新しい実行状態や再送 scheduler を作らない。[新旧比較](../../packages/web-core/src/features/agent-runtime/model/canonicalAgentTimeline.ts#L390-L406)・[attempt の照合](../../packages/web-core/src/features/agent-runtime/model/canonicalAgentConversation.ts#L648-L679)・[再接続と retry のテスト](../../packages/web-core/src/features/agent-runtime/model/canonicalAgentConversation.test.ts#L856-L904)

共通 `ChatElapsedTime` は有効な終了時刻を active flag より優先し、終了後は interval を作らない。終了根拠がない非活動 Run や開始時刻が不正な Run は時間を表示せず、ブラウザーの mount 時刻で補わない。再マウント時も timestamp 差から再計算し、ローカル加算 counter を復元しない。活動中はブラウザーの時計を使い、負の差は0へ丸めるため、精密な provider 稼働時間の測定ではない。[表示と時計](../../packages/ui/src/components/ChatElapsedTime.tsx#L11-L69)

### Goal の時間は別の値

Goal の `time_used_seconds` は provider 報告値であり、Run footer の壁時計時間へ置き換えない。Goal card は Goal と Run がともに active のときだけ報告値へ経過秒を補間し、非活動 Run では `Recorded time` として保存された報告値を表示する。Goal のない Run にも footer があり、二つの値が異なること自体は異常ではない。[Goal 表示](../../packages/web-core/src/features/workspace-chat/ui/GoalProgressCard.tsx#L54-L98)・[補間条件](../../packages/web-core/src/features/workspace-chat/model/goalElapsed.ts#L1-L15)

### 検証の入口と限界

変更時は、DB の遅延イベントテスト、[canonical projection の時間テスト](../../packages/web-core/src/features/agent-runtime/model/canonicalAgentConversation.test.ts#L769-L904)、[制御時計を使う browser component テスト](../../tests/chat/run-elapsed.test.mjs)、[隔離 API・SQLite・WebSocket・画面のテスト](../../tests/run-elapsed/isolated.mjs)を別の境界として読む。隔離テストも provider 記録は合成であり、実モデルの時間精度を証明しない。再現手順は [run-elapsed README](../../tests/run-elapsed/README.md)に置く。

[開発時の検証記録](../../tests/run-elapsed/VALIDATION.md)は成功した試験と、`cargo sqlx` 未導入で metadata 検査前に失敗した `prepare-db:check` を区別している。今回の Sync は凍結 source `2b95298fc4efba657c676644a57bafa620b9e017` の実装・テスト内容を静的に照合したもので、製品テストは再実行していない。凍結入力では正式 Integration の検証は明示的に省略されており、開発時の成功記録を統合候補の検証成功と扱わない。無関係な過去の説明全体を再検証したという意味でもない。

## Provider session の継続と取り込み

Session の executor は最初の指定で設定できるが、以後の別 executor 指定は拒否する。native binding を持った Session では provider、runtime profile、native session ID を別の値へ切り替えられない。別構成を使うときは新しい Session が境界になる。[executor 検証](../../crates/server/src/routes/sessions/agent_run.rs#L154-L171)、[binding 検証](../../crates/server/src/routes/sessions/agent_run.rs#L331-L374)

既存 native 会話を明示的に取り込む場合は正確な作業ディレクトリ scope が必要で、取り込み後は scope と profile fingerprint も維持する。同じ native 会話を別 LVK Session が所有していれば拒否する。履歴を表示できることと、任意のディレクトリ・profile で再開してよいことは別である。[scope 必須](../../crates/server/src/routes/sessions/mod.rs#L354-L381)、[継続検証](../../crates/server/src/routes/sessions/agent_run.rs#L375-L425)

Codex の thread 開始・再開では、app-server が設定から解決した model と reasoning effort を client が採用し、その後の turn 構築に用いる。LVK の依頼で effort override が未指定であることと、実行時の effort が未解決であることは同じではない。設定の継承と OpenWiki writer の起動前確認は [Provider 統合](../integrations/agent-providers.md)を参照する。[解決結果の採用](../../crates/executors/src/executors/codex/client.rs)、[開始・再開・turn](../../crates/executors/src/executors/codex/client.rs)

Codex の resume は同じ native thread ID を要求し、fork しない。再開設定の更新だけでは新しい developer instructions が会話履歴に現れない場合があるため、次の chat・Goal・review・compaction より先に `thread/inject_items` で現在の host context を注入する。空の context では注入せず、注入失敗なら継続を停止する。Goal objective の増量や擬似 user turn で代用しない。[再開処理](../../crates/executors/src/executors/codex/client.rs)、[順序と失敗のテスト](../../crates/executors/src/executors/codex/client.rs)。これにより、[Repository Memory の現在 run identity](repository-memory.md#change-manifest-は変更理由を運ぶイベント)と、[fresh 会話だけへ組み込む保存文脈](card-context-and-llm-wiki.md)を区別できる。

## 状態、停止、送信待ち

正本の状態は Pending / Starting / Running / AwaitingInput / AwaitingApproval / Cancelling と、終端の Succeeded / Failed / Cancelled / Crashed / AuditFailed。入力・承認待ちは終了扱いではない。ProjectionStatus はこの実行状態とは別に、正本の表示状態を安全に再構成できているかを表す。[状態契約](../../crates/executors/src/runtime/contracts.rs#L636-L691)

Cancel は終端 run には無操作で、活動中なら Cancelling を保存して実プロセス側を止める。InterruptTurn は provider の現在 turn への中断要求であり、終端 run には送れない。Steer は活動中の run に空でない追加指示を渡す。projection が degraded の間は通常制御を拒否し、Cancel と既知の startup collision を証拠検証する明示 Retry だけが回復経路を持つ。詳細な対象・拒否条件は [Runtime の限定回復](../architecture/agent-runtime.md#明示-retry-に限る-startup-collision-の回復)に置く。[停止境界](../../crates/local-deployment/src/agent_run_port.rs)、[steer](../../crates/local-deployment/src/agent_run_port.rs)

通常 interactive Workspace の送信待ち queue は **Session ごとのメモリ内 FIFO**。各項目は ID・本文・設定を持ち、本文編集、個別削除、全 ID の正確な並べ替えを Session lock 下で行う。受理済み項目と composer の新しい draft は別物である。成功した最新 run の [Repository Memory](repository-memory.md)完了後に先頭を起動する。元 run の失敗・取消、memory 完了失敗では queue を保持して pause し、次の起動失敗では先頭へ戻して pause する。古い terminal 通知は最新 run の queue を消費しない。[保存](../../crates/services/src/services/queued_message.rs)、[終端処理](../../crates/local-deployment/src/container.rs)。

resume は active run 不在・maintenance / Integration guard を確認して先頭から再開する。古い cancel endpoint は全 queue を消すので、個別 remove と区別する。server restart では失われ、[Workflow outbox](../architecture/workflow-runtime.md)の耐久性は持たない。[queue API](../../crates/server/src/routes/sessions/queue.rs)。

実行専用 Workspace の終端通知では、上記の通常 source 完了・queue 消費を行わない。所有する統合・Wiki 保守が結果を確定し、一般の follow-up や retry を許可するかも [操作用途と実行所有者](workspace.md#操作用途と実行所有者)で検証する。[通常終端処理からの除外](../../crates/local-deployment/src/container.rs)

Cancel の応答は実プロセスの停止確認と監査の永続化を伴う。子プロセスの猶予・停止失敗の扱いは [Agent Runtime の取消](../architecture/agent-runtime.md#取消と実プロセスの終了)を参照する。

Board の列は AgentRun の状態表示と別である。In Progress は着手済み・未完了を示し、失敗や取消後にも残る。開始・成功を列へ反映する条件と複数 Workspace の競合は [Kanban と実行の同期契約](project-and-issue.md#kanban-と実行の同期契約)を参照する。OS の exit 0 だけを成功と扱わず、provider の成功根拠がない正常終了は Crashed にする。[終了判定](../../crates/local-deployment/src/process_host.rs#L1192-L1268)・[回帰テスト](../../crates/local-deployment/src/process_host.rs#L1535-L1562)

## 承認・質問と現在の接続状態

承認は provider の操作や計画を進める判断、質問は provider が求める回答である。UI は未解決イベントから `agentRunId` と `approvalId` または `inputId` を選び、`controlId` で個々の待機を識別する。承認・拒否と質問回答は別の制御であり、質問は質問名と回答の組を JSON にして送る。[待機の選択](../../packages/web-core/src/features/workspace-chat/model/hooks/useWorkspacePendingApproval.ts)、[制御への変換](../../packages/web-core/src/features/workspace-chat/model/canonicalAgentControls.ts)

Plan with Goal の objective 編集・専用承認 UI は退役した。古い draft control は明示的な retirement error になり、通常の Goal 操作とは区別する。[control の拒否](../../crates/executors/src/executors/codex/client.rs#L378-L383)

**この checkout の Codex 起動経路は `NoopExecutorApprovalService` を渡している。** このサービスは tool/plan 承認を即座に Approved とし、質問回答の待機は ServiceUnavailable にする。Codex client は質問待機のエラーを TimedOut として記録する。計画承認後は Default の実装 turn を始めるが、この配線を人の承認による停止点と説明してはいけない。[host の注入](../../crates/local-deployment/src/process_host.rs#L333-L360)、[Noop の実装](../../crates/executors/src/approvals.rs#L69-L103)、[質問と計画処理](../../crates/executors/src/executors/codex/client.rs)

さらに Codex の汎用 DirectControl は Approve/Input を「元の server request ID を使う必要がある」と Unsupported で拒否する。[制御の制約](../../crates/executors/src/executors/codex/client.rs)。廃止表示のない [chat-interface の計画レビュー手順](../../docs/workspaces/chat-interface.mdx) は承認・変更依頼という操作意図を記録するが、現在の host 配線との隔たりがある。ここはソースから確認した制限であり、対話実験の成功を意味しない。[Workflow の HumanGate](workflow-attempt.md) は別の停止・再開契約を持つ。

## 非同期質問への回答と配送証拠

非同期の仕様質問は上記の blocking input / tool approval と別で、Workspace UI と外部 MCP が共通の耐久回答受付を使う。Session lock 下で既存 receipt を照合し、質問が現在の Session に属し未回答であることと、follow-up queue が空であることを確認する。回答を予約してから、質問を出した元の AgentRun に Steer を送る。遅い回答で別 run や新規 follow-up を起動しない。[受付と配送](../../crates/server/src/routes/sessions/questions.rs#L158-L251)

同じ質問の異なる回答は競合となり、配送が不確実なら Session の確認を要求する。command/run の証拠がない予約だけを再試行可能にし、配送後かもしれないエラーでは予約を維持する。新規本文は `LVK question answer: <request_id>` を使い、既存履歴の `Astra operation: <request_id>` も照合・予約解放判定・receipt 回復で認識する。[旧 Astra の廃止](project-chat.md)に伴って旧 marker の認識を削ると二重配送を招く。[互換処理](../../crates/server/src/routes/sessions/questions.rs#L253-L324)・[新旧 marker のテスト](../../crates/server/src/routes/sessions/questions.rs#L486-L592)

外部 MCP の質問参照は Project / Workspace / Session と revision を検証し、実行承認は LVK での判断へ案内する。仕様回答の経路があることは tool approval の代理権限ではない。[MCP の質問分類](../../crates/server/src/remote_mcp/questions.rs#L94-L171)

## 未送信ドラフトと Scratch

**Scratch** は編集状態であって、AgentRun の入力正本や実行待ち queue ではない。composer は本文・executor 設定を500 ms debounce で保存するが、入力時に tab-local な `sessionStorage` に pending backup も置く。サーバー保存の ACK は一致する版だけを消すため、debounce 前の hard reload や遅い保存応答でも新しい入力を失いにくい。初回 snapshot が届く前にユーザーが入力した内容も、遅い復元で上書きしない。[editor](../../packages/web-core/src/features/workspace-chat/model/hooks/useSessionMessageEditor.ts#L64-L179)、[pending backup](../../packages/web-core/src/features/workspace-chat/model/pendingSessionDraft.ts)

| 対象 | 保存キー・保存先 |
| --- | --- |
| 新規 Session の composer | Workspace ID |
| 既存 Session の composer | Session ID |
| 承認・質問の返答 | 待機固有の control ID。通常 follow-up と分離 |
| local runtime | SQLite Scratch と JSON Patch WebSocket |
| 未 ACK の pending backup | runtime・host・scratch ID 別の sessionStorage。別タブ・別Sessionの原本ではない |

保存 mutation は対象 identity ごとに直列化する。画面切替時は編集時に捕捉した旧対象へ flush し、次の Session の callback へ送らない。新規 Session 作成の遅い応答や送信 ACK が別 host / Workspace / Session の現在入力を奪わないよう、scope と編集 revision も照合する。[mutation queue](../../packages/web-core/src/shared/lib/scratchMutationQueue.ts)、[送信 ACK](../../packages/web-core/src/features/workspace-chat/ui/SessionChatBoxContainer.tsx#L717-L762)

送信成功後は送信時の本文・設定 snapshot を条件として Scratch を削除する。新しい draft や既に削除済みの場合は安全な no-op。backend の follow-up 起動や queue 消費から無条件に削除しない。現行 useScratch は server 経路のみである。[DELETE 契約](../../crates/server/src/routes/scratch.rs)、[queue 終端](../../crates/local-deployment/src/container.rs)

queue 済みでも Scratch は別の編集状態として作成・更新できる。旧 queue-ID guard は削除されている。条件付き ACK は送信 snapshot が一致する版だけを消し、queue 消費は新しい draft を削除しない。[Scratch CRUD](../../crates/server/src/routes/scratch.rs)。

保存エラーは editor の console に残り、未 ACK backup を保持する。ただしブラウザー storage が無効・容量不足なら backup 自体が書けない。この場合も入力欄を消さないが、再読み込み後の復元は保証できない。sessionStorage の backup 失敗を、サーバーへの保存成功と取り違えない。[backup の限界](../../packages/web-core/src/features/workspace-chat/model/pendingSessionDraft.ts#L24-L54)。[ボード表示設定](project-and-issue.md#ボード表示と共有状態)は別の Scratch データである。

### 送信中の編集と Markdown 正規化

通常の送信待ち `sending` では送信操作の制約を保ちつつ、本文 editor は編集可能にする。成功 ACK が消費できるのは送信時と同じ scope・revision・content の composer だけである。content は本文だけでなく設定・Skill・添付・review・resume 選択を含む snapshot なので、trim した本文の一致だけへ弱めてはいけない。送信失敗や、待機中に新しく編集した draft は消さない。[送信中の editor](../../packages/ui/src/components/SessionChatBox.tsx#L915-L930)・[snapshot と ACK](../../packages/web-core/src/features/workspace-chat/ui/SessionChatBoxContainer.tsx#L688-L762)・[一致判定のテスト](../../packages/web-core/src/features/workspace-chat/model/sessionDraft.test.ts)

Markdown editor への外部値読み込みは、末尾改行などを正規化しても新規ユーザー編集ではない。`MarkdownSyncPlugin` はその更新に `markdown-sync` tag を付け、serialization の追跡は更新するが親の `onChange` へ返さない。外部値と直近 serialization を別々に追跡し、transformer の変更時には両方をリセットして再解析する。editor hook も同一本文の通知では revision を増やさない。この二つの区別が、送信中の正規化通知だけで ACK の一致条件が崩れることを防ぐ。[外部値と編集通知](../../packages/ui/src/components/MarkdownSyncPlugin.tsx#L30-L97)・[同値通知](../../packages/web-core/src/features/workspace-chat/model/hooks/useSessionMessageEditor.ts#L73-L86)

キー送信は editor の最新 Markdown を `flushSync` で親へ反映した後、ref に保持した現在の送信 callback を呼ぶ。flush 前の closure では更新前 snapshot を送信対象にし得るためである。IME 中の Enter / Ctrl+Enter 抑制は維持する。[flush 後の callback](../../packages/ui/src/components/KeyboardCommandsPlugin.tsx#L150-L171)。これらは表示・draft ACK の変更であり、[永続 steering の保存と表示順](../architecture/web-and-sync.md#永続会話と-live-の順序)とは別の責任を持つ。

変更時の editor fixture は、末尾改行付き外部値の読み込みと送信成功、同値のキー flush、失敗時入力、送信中の追加入力、focus と IME を確認する。built-app 試験でも実 Scratch と制御した follow-up 応答を使うため、実 provider の通常 follow-up 成功を証明するものではない。[editor 試験](../../tests/chat/editor-display.test.mjs#L75-L141)・[built-app 入力試験](../../tests/chat/isolated.spec.ts#L295-L372)。今回の静的照合と開発時の実行記録の区別は [チャット検証境界](../architecture/web-and-sync.md#チャット表示の検証境界)を参照する。

### Enter 操作と重複送信の境界

既存・新規 Session の composer と新規 Workspace の入力は `sendShortcut="Enter"` を明示するため、保存済み `send_message_shortcut` が `ModifierEnter` でも Enter 送信になる。General Settings は選択 UI を持たず、Enter で送信・Ctrl+Enter で改行という案内を表示する。共通 keyboard plugin 自体は `ModifierEnter` を引き続き扱うので、アプリの指定と共通部品の既定値を混同しない。[Session の指定](../../packages/web-core/src/features/workspace-chat/ui/SessionChatBoxContainer.tsx#L1310-L1344)・[Workspace 作成の指定](../../packages/web-core/src/shared/components/CreateChatBoxContainer.tsx#L575-L596)・[設定表示](../../packages/web-core/src/shared/dialogs/settings/settings/GeneralSettingsSection.tsx#L772-L782)・[共通部品の既定値](../../packages/ui/src/components/KeyboardCommandsPlugin.tsx#L22-L42)

Enter mode は Ctrl+Enter を明示的な line break に変換する。IME composition・keyCode 229・typeahead 表示中は送信処理を譲り、修飾キーなしの Enter の repeat は消費するだけで送信しない。Ctrl のない Shift / Meta / Alt 付き Enter はこの送信処理の対象外である。flush 後の callback は現在の画面状態に従い、通常送信だけでなく実行中の queue や質問回答などへ分岐するため、Enter が常に新しい AgentRun を作るわけではない。[キーの分岐](../../packages/ui/src/components/KeyboardCommandsPlugin.tsx#L164-L236)・[状態別操作](../../packages/ui/src/components/SessionChatBox.tsx#L495-L520)。[Todo 追加](project-todos.md#追加-ui-の入力契約)も Enter / Ctrl+Enter を使うが、カテゴリ検査や pending 中の編集禁止は Todo 側の責任である。

`useSessionSend` は host・Workspace・Session・新規 Session mode の組を scope とし、await 前に in-flight set へ登録して同じ scope の重複送信を拒否する。新規 Session 作成と既存 Session follow-up の双方に適用し、finally で解除する。新規 Workspace 作成には別の同期 ref guard があり、作成処理と draft cleanup を囲んで finally で解除する。いずれも hook / component 内の抑止であり、別タブ・別 client を含む永続的な冪等性や backend の実行受付を保証するものではない。[Session の guard](../../packages/web-core/src/features/workspace-chat/model/hooks/useSessionSend.ts#L58-L159)・[Workspace 作成の guard](../../packages/web-core/src/shared/components/CreateChatBoxContainer.tsx#L395-L474)。解除後の再送可否と、成功 ACK で消してよい入力の判定は別であり、後者は上記の scope・revision・content 照合に従う。

### 入力操作の検証範囲と未解決点

[editor fixture](../../tests/chat/editor-display.test.mjs#L127-L150)は Ctrl+Enter の改行・repeat 抑止と共通部品の ModifierEnter mode を検査する。[入力操作の隔離 E2E](../../tests/chat/input-operations.isolated.mjs#L267-L339)は実 Workspace 画面から送る Session ID と複数行 prompt を調べ、follow-up だけを保留後の400応答へ置き換えて再送を確認する。実 agent/provider の成功や OS 日本語 IME の手操作は検証しない。Todo の保存境界は [Todo の検証入口](project-todos.md#変更時の検証入口)に置く。

統合済み source と試験期待には二つの不一致が残る。

- `input-operations.isolated.mjs` は送信保留中の `contenteditable=false` を要求するが、現行 `SessionChatBox` は通常の `sending` 中も本文を編集可能にする。[試験の期待](../../tests/chat/input-operations.isolated.mjs#L310-L330)・[現行 editor](../../packages/ui/src/components/SessionChatBox.tsx#L915-L928)
- 既存 `isolated.spec.ts` は設定を ModifierEnter に変えた後の Ctrl+Enter で追加リクエストを期待するが、現行 Session composer は Enter に固定され、Ctrl+Enter は改行になる。[旧設定に依存する期待](../../tests/chat/isolated.spec.ts#L353-L371)・[固定指定](../../packages/web-core/src/features/workspace-chat/ui/SessionChatBoxContainer.tsx#L1310-L1344)

これらは静的照合で見つけた不整合で、今回実行して失敗を観測したという意味ではない。[LOCAL-26 の開発時検証記録](../../tests/chat/input-operations-validation.md)の成功を、統合後 checkout の試験成功に読み替えない。試験期待と現行契約の調整・再検証は未解決であり、この文書保守では source や tests を変更しない。

今回の通常 Sync は凍結 source `a1c1fd694851904f4f68eddbbc93dfd5de89bd9b` の入力操作変更を対象に、共通 keyboard、各 composer と送信 hook、設定 UI、Todo の REST/MCP・保存モデル、独立した fixture / E2E の入口から照合した。正式 Integration の検証は明示的に省略されている。製品テストは再実行しておらず、無関係な過去の Wiki 全体を再検証したとは扱わない。

## Plan の遷移

Plan は native `collaborationMode` と Codex 内蔵 instructions を使い、prompt に `/plan` を追加しない。legacy `plan: true` preset は独立した Auto / Supervised permission override より先に mode を解決し、明示 mode があればそちらを優先する。承認で Default へ移る際に client の Plan flag を解除するため、その後 reasoning depth を変えても Plan に戻らない。[mode 解決](../../crates/executors/src/executors/codex.rs#L626-L669)・[native mode](../../crates/executors/src/executors/codex/client.rs#L453-L482)・[承認遷移](../../crates/executors/src/executors/codex/client.rs#L1116-L1145)・[設定更新](../../crates/executors/src/executors/codex/client.rs#L714-L742)。CLI と wire 型の組合せは [Provider 互換境界](../integrations/agent-providers.md)に置く。

## Goal の寿命

現在選べる ExecutionMode は Code / Plan / Goal で、PermissionPolicy とは別である。保存済み `PlanWithGoal` の enum/API 表現は互換のため残るが、新規 launch は Plan に正規化し Goal を自動作成しない。composer 復元も旧 Goal option を消す。[起動互換](../../crates/executors/src/executors/codex.rs#L585-L665)・[復元](../../packages/web-core/src/shared/lib/executorOverrides.ts#L20-L29)。Goal は objective と状態、任意 token budget、使用 token・時間を持つ。Codex の objective は 4,000 文字上限を事前検証する。[mode の定義](../../crates/executors/src/profile.rs#L21-L37)、[Goal データ](../../crates/executors/src/runtime/contracts.rs#L199-L219)、[長さ検証](../../crates/executors/src/executors/codex.rs#L1-L23)

Codex Goal の管理開始は対応 RPC の成功応答で確定する。再生された通知や別 thread の完了から、この run が Goal を開始・達成したと判定しない。管理中は active、paused、blocked、usageLimited、budgetLimited でも host を維持し、同じ thread の complete/cleared で維持を解除する。[Goal lifecycle](../../crates/executors/src/executors/codex/goal_lifecycle.rs#L36-L104)、[順序・別 thread テスト](../../crates/executors/src/executors/codex/goal_lifecycle.rs#L111-L142)

Goal の subagent 上限は spawned agent の制約で、0 は無効化、未指定は Codex の既定値を使う。これは [Workflow](workflow-attempt.md) のノード並列数や、[Arena](arena.md) の候補数とは別の範囲である。[設定契約](../../crates/executors/src/profile.rs#L169-L175)、[Codex 設定テスト](../../crates/executors/src/executors/codex.rs#L1615-L1654)

## Native subagent と旧実行モデル

Provider が内部で起動する子 agent の活動は AgentActivity として記録される。子の完了を親の terminal output、status、Goal に適用しない。LVK が所有する [Workflow ノード](workflow-attempt.md) の別 AgentRun と区別する。[reducer と回帰テスト](../../crates/executors/src/runtime/reducer.rs#L278-L298)。表示と互換性の文書は [delegated-agent-display](../../docs/future/agent-runtime/delegated-agent-display.md) を参照し、将来の Audit 再投影構想を現在の自動修復と読まない。

Codex の spawn 観測だけが直接の親子関係を確定し、send/wait 等の相互作用から親を推測しない。観測された子はその Run の折り畳み task card に現れ、利用可能な prompt / message と lifecycle を別に保持する。tool 完了や interrupt 要求は子の終端証拠ではなく、状態のない再開・入力送信は以前の完了を unknown に戻す。親終了後の未完了観測も active 表示を続けず、最後に観測した状態付き unknown になる。[scope](../../crates/executors/src/executors/codex/agent_scope.rs#L58-L178)・[表示](../../packages/web-core/src/features/agent-runtime/model/canonicalAgentConversation.ts#L240-L310)。永続イベントから再構成するため、観測のなかった古い Run に子を補完しない。mapper 互換性は [runtime 投影](../architecture/agent-runtime.md#context-と子観測の投影)を参照する。

親が成功しようとすると client は観測済み全子の終端証拠を要求する。running / pending / unknown が残れば Failure にし、無期限待機や独自継続 scheduler は作らない。子の failed / interrupted / shutdown も終端証拠にはなるため、親が対処済みの子失敗だけで親を失敗にしない。Goal / Plan の成功出口にも同じ guard が掛かる。[成功 guard](../../crates/executors/src/executors/codex/client.rs#L1542-L1662)・[Plan 出口](../../crates/executors/src/executors/codex/client.rs#L1819-L1840)。通知されなかった子は検出できず、terminal 通知のない provider では有用な成果があっても安全側に失敗し得る。[記録された設計と限界](../../docs/development/codex-execution-observations.md#completion-and-integration)。これは [正式 Integration](formal-integration.md)の成功条件や明示 commit 採用を代替しない。

旧 ExecutionProcess は script 実行などに残るが、coding-agent action は明示的に拒否される。現行 coding agent の活動判定・停止・履歴の正本は AgentRun を追う。[旧入口の拒否](../../crates/local-deployment/src/container.rs)、[活動判定のテスト](../../crates/server/src/routes/sessions/agent_run.rs#L873-L910)

## Context 使用量の意味

context gauge は親 thread の最後の測定 `last.totalTokens / modelContextWindow` を示す。累積 input + output（cached input を二重加算しない）とは別で、compaction 後に減少し得る。進行中の要求サイズのリアルタイム推定ではない。欠損・不正 capacity から偽の 0% を作らず unknown とし、モデル名から容量を推測しない。新しい Run / attempt では古い gauge を引き継がず測定を待つ。[native usage](../../crates/executors/src/executors/provider_adapter.rs#L1619-L1645)・[表示値検証](../../packages/ui/src/components/ContextUsageGauge.tsx#L6-L24)・[scope 切替](../../packages/web-core/src/features/agent-runtime/model/canonicalAgentConversation.ts#L664-L678)。保存と replay は [usage snapshot の経路](../architecture/agent-runtime.md#context-と子観測の投影)にまとめる。

blocking 待機や終端の通知・ChatGPT 相談は [Web Push](../integrations/web-push.md)、保存済み commentary の外部読取は [list_run_events](../integrations/remote-access.md#保存済み-codex-進捗の読取)を使う。通知や抜粋から実行成功・承認済み・統合済みを推定しない。

## 会話表示と入力

thinking / tool の表示は会話 timeline の一部であり、表示用の経過時間や折り畳みから実行成功を判定しない。状態は AgentRun の正本を参照する。日本語ラベルは i18n、本文は Markdown renderer が扱う。[thinking 表示](../../packages/ui/src/components/ChatThinkingMessage.tsx)。IME composition 中や keyCode 229 の Enter / Tab 操作は editor command で抑制し、変換確定を送信・一覧操作と取り違えない。[keyboard guard](../../packages/ui/src/components/KeyboardCommandsPlugin.tsx)。

チャットを塞ぐ script 判定は活動中の ScriptRequest のうち `devserver` を除く。Preview server は turn 間にも稼働し専用 Stop を持つため、その存在だけで送信を待たせない。setup や IntegrationValidation 等の活動判定は残る。これは UI の送信可否であり、backend の実行所有・予約 guard を解除するものではない。[判定](../../packages/web-core/src/shared/lib/executionProcessRuntime.ts#L45-L58)、[利用箇所](../../packages/web-core/src/features/workspace-chat/ui/SessionChatBoxContainer.tsx)、[回帰テスト](../../packages/web-core/src/shared/lib/executionProcessRuntime.test.ts)

長い Session は最新の会話と活動中 run を先に表示し、古い履歴の読込中も別の状態として示す。購読 batch と Session 切替時の分離は [Web の履歴同期](../architecture/web-and-sync.md#最近の会話を先に表示する)で説明する。
