---
type: concept
title: Project・Issue と実施の関係
description: ボード上の仕事と実行環境を結ぶ Project・Issue の意味、ローカル保存、旧 Task との境界。
tags: [project, issue, kanban, task]
sources:
  - id: openwiki-source-11356f66af906ebb06f3ef2e
    resource: repo://crates/api-types/src/issue.rs
  - id: openwiki-source-98b2b381d94f0ceabce67262
    resource: repo://crates/api-types/src/project.rs
  - id: openwiki-source-0afb1e125b616abe2a64c15f
    resource: repo://crates/db/migration_compat/project_keys_multi.sql
  - id: openwiki-source-e4c80fe5a2af8a4abd8a8684
    resource: repo://crates/db/migrations/20260427000000_local_kanban.sql
  - id: openwiki-source-5c1907109b5a72a746e85a0c
    resource: repo://crates/db/migrations/20261020190000_kanban_lifecycle.sql
  - id: openwiki-source-c2fc034438311f3aa02f5ee5
    resource: repo://crates/db/migrations/20261023110000_retain_design_notes_on_project_delete.sql
  - id: openwiki-source-11bd8cfafd506215798951a6
    resource: repo://crates/db/migrations/20261025000000_issue_classification.sql
  - id: openwiki-source-84bf5b13c3c1bf55a6f0a317
    resource: repo://crates/db/migrations/20261025150000_issue_archive.sql
  - id: openwiki-source-bac2c1bba2c5a35274ebfe6b
    resource: repo://crates/db/migrations/20261026010000_generated_archive.sql
  - id: openwiki-source-3c57aab40b425fc8b5d0d5c5
    resource: repo://crates/db/migrations/20261027090000_generated_issue_status.sql
  - id: openwiki-source-4fe58481730f861df4628448
    resource: repo://crates/db/migrations/20261027090100_issue_number_high_water.sql
  - id: openwiki-source-9bc71fe4f6cbbaa6fa9168e1
    resource: repo://crates/db/src/models/agent_runtime_kanban_tests.rs
  - id: openwiki-source-5c693a62e6ccf09ccb60592f
    resource: repo://crates/db/src/models/agent_runtime_kanban.rs
  - id: openwiki-source-672c8da0d18c56b60df745a2
    resource: repo://crates/db/src/models/board_tag.rs
  - id: openwiki-source-830e28055c4ae6f5464df312
    resource: repo://crates/db/src/models/design_note.rs
  - id: openwiki-source-c01fa644be961cb7ee12d2b5
    resource: repo://crates/db/src/models/integration.rs
  - id: openwiki-source-2e4f4c7654ac4a8e4d26283a
    resource: repo://crates/db/src/models/issue_number_tests.rs
  - id: openwiki-source-6fe5fbd6df406f973641b771
    resource: repo://crates/db/src/models/issue_number.rs
  - id: openwiki-source-0f48367503e71396b298f71a
    resource: repo://crates/db/src/models/issue_order.rs
  - id: openwiki-source-ccd357b1781e6e90e56fd858
    resource: repo://crates/db/src/models/repo.rs
  - id: openwiki-source-4f59e07f8850eb04105d5cb8
    resource: repo://crates/db/tests/generated_issue_status.rs
  - id: openwiki-source-a43253b5be9686da077b8067
    resource: repo://crates/server/src/remote_mcp/issue_comments.rs
  - id: openwiki-source-5720d865d1c837f0e6b7dab7
    resource: repo://crates/server/src/routes/integrations/conversation.rs
  - id: openwiki-source-1ffb01d1ba3953937e0771bf
    resource: repo://crates/server/src/routes/local_remote.rs
  - id: openwiki-source-24838d7108e2888a7fdc2714
    resource: repo://crates/server/src/routes/local_remote/issue_archive.rs
  - id: openwiki-source-a517c0f8e44d2cf0c440fbfc
    resource: repo://crates/server/src/routes/workflows.rs
  - id: openwiki-source-38de6ae4b89edddb12325c18
    resource: repo://crates/server/src/routes/workspaces/links.rs
  - id: openwiki-source-04d5adb51b5929dacce999bc
    resource: repo://crates/server/src/workflow_runtime/bootstrap.rs
  - id: openwiki-source-15dea6acc3c44eb46d7d11d9
    resource: repo://docs/development/issue-numbering.md
  - id: openwiki-source-5e4c1802d2c1ee81cad7ac45
    resource: repo://packages/ui/src/components/IssueWorkspaceCard.tsx
  - id: openwiki-source-ff91ec3e5a5f406a698e52e6
    resource: repo://packages/ui/src/components/KanbanCardContent.tsx
  - id: openwiki-source-36bc16e51274306c238a63a2
    resource: repo://packages/web-core/src/features/kanban/model/hooks/useKanbanFilters.ts
  - id: openwiki-source-cae8a3c125596fe3e7ed7b37
    resource: repo://packages/web-core/src/features/kanban/ui/kanban-grid.css
  - id: openwiki-source-b39952c69795059582eecfdf
    resource: repo://packages/web-core/src/features/kanban/ui/KanbanCardDetails.tsx
  - id: openwiki-source-e0c2de9e01617a8949b78360
    resource: repo://packages/web-core/src/features/kanban/ui/KanbanContainer.tsx
  - id: openwiki-source-00110cdcfec890895b99690a
    resource: repo://packages/web-core/src/shared/dialogs/settings/settings/RemoteProjectsSettingsSection.tsx
  - id: openwiki-source-b885bc17dfe85c4ca45223e9
    resource: repo://packages/web-core/src/shared/hooks/useUiPreferencesScratch.ts
  - id: openwiki-source-44d596f923fda7620210296d
    resource: repo://packages/web-core/src/shared/stores/useKanbanDisplayStore.ts
  - id: openwiki-source-5ef837b2bc54d286b3dedd8f
    resource: repo://packages/web-core/src/shared/stores/useUiPreferencesStore.ts
generated: { by: "codex", at: "2026-10-29T08:49:34.152Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-29T08:49:34.152Z
---

# Project・Issue と実施の関係

**Project** は関連する仕事を集める単位で、Issue、ボードの Status、分類用 Tag を持つ。**Issue** は「何を達成するか」を表す仕事である。AI が作業する場所は [Workspace](workspace.md)、会話と実行は [Session・AgentRun](session-and-agent-run.md) が受け持つ。この分離により、一件の仕事に複数の実施や比較候補を関連付けられる。[Issue 契約](../../crates/api-types/src/issue.rs#L20-L40)、[関連表](../../crates/db/migrations/20260427000000_local_kanban.sql#L208-L223)

local Project は固定Organizationと現在actorの [表示モデル](organization-and-membership.md)に結び付く。Issue の担当者・Follower は認証権限とは別である。

Project には Issue と独立した [Todo 共有メモ](project-todos.md)もある。メモの完了は Card の Done や agent 実行を進めず、任意の同一 Project の Card 参照だけを持つ。Board の Not Started 列（旧 Todo）や旧 `tasks` と区別する。

設計の意図と変遷は [設計メモ](design-notes.md) に本文・独立追記として保持する。Project 削除時もその履歴は残るが、既存APIでのアクセスは閉じる。他機能の削除制約と、保持後の閲覧方針は [削除後の契約](design-notes.md#project-を削除した後の保持)を参照する。

## 仕事の識別と分類

Issue の主キーは UUID。表示用 `simple_id` は保存済み Project key と Project UUID ごとの永続高水位 `issue_number_high_water` の次番号から作る。Project 作成時の key は必須で、前後空白除去・ASCII大文字化後に英字先頭の英数字2〜10文字、現存 Project 間で一意である。API 更新では変更できない。UI の名前由来候補は編集可能で、非ASCII語はローマ字化せず空候補とする。重複はサーバーで409となり、UI候補は予約ではない。[作成](../../crates/server/src/routes/local_remote.rs#L440-L489)・[正規化](../../crates/api-types/src/project.rs#L70-L83)・[候補](../../packages/web-core/src/shared/lib/projectKey.ts)。

Project key 未適用の既存 DB は、互換 migration が Project UUID 順で先頭を `LOCAL`、以降を `LOCAL2`〜`LOCAL99999` にする。既存 Issue UUID・番号・simple_id は変更しないため、過去の表示接頭辞と現在の Project key が異なる場合がある。新規 Issue は現在の key を使う。履歴選択・上限失敗・旧 tool の制約は [Project key migration の互換運用](../operations/development.md#project-key-migration-の互換運用)を参照する。通常 Issue・自動マージ Card・OpenWiki保守Cardは同じ allocator に caller の `BEGIN IMMEDIATE` transaction を渡し、Issue と関連 Tag を一緒に commit/rollback する。番号・simple_id のDB一意制約はProject内であり、高水位導入後に commit した番号は最高行・全行を削除しても再利用しない。Project削除後のkey再利用も禁止する履歴予約はない。永続参照はUUIDを使う。[allocator](../../crates/db/src/models/issue_number.rs#L23-L46)・[互換 SQL](../../crates/db/migration_compat/project_keys_multi.sql#L1-L21)・[通常作成](../../crates/server/src/routes/local_remote.rs#L789-L855)。

priority は urgent / high / medium / low の四値で、作成時省略と legacy の明示 null は medium に正規化する。更新時の省略は保持するが明示 null は medium になる。nullable wire型は旧client互換のため残り、migration と trigger は直接生成・旧行の NULL も補正する。優先度は実行の許可や順序を変更する仕組みではない。[共有更新](../../crates/server/src/routes/local_remote.rs#L879-L963)・[NULL補正](../../crates/db/migrations/20261025000000_issue_classification.sql#L1-L11)。

Tag は Project 内の分類であり、system はLVK管理の予約名である。通常のrename・削除・手動付与/解除を拒否するがcolor変更は許す。新規Projectと一度限りのmigrationで不足する system / survey / bug / feature を用意し、読取は削除済み通常Tagを再作成しない。既存同名定義は残し、自動 system 関連は最小IDを選ぶ。自動マージ/OpenWikiの生成transactionと、記録されたownerのstartup補完がsystemを付ける。ラベルはownerの証拠や承認に代わらない。[初期化と既存owner](../../crates/db/migrations/20261025000000_issue_classification.sql#L13-L39)・[同一Projectと予約名](../../crates/db/src/models/board_tag.rs#L14-L72)。外部MCPのadd/removeは独立した一件の変更で、他のTagを置換しない。[MCP](../integrations/remote-access.md)。

Status は Project の列を参照する ID であり、エージェントのプロセス状態ではない。ローカルの初期列は Not Started / In Progress / In Review / Done / Cancelled で、Cancelled は初期状態で非表示。更新 API は `status_id` と `completed_at` を別々の入力として保持するため、列を Done に変えるだけで完了日時が自動計算されるとは読めない。[初期列](../../crates/server/src/routes/local_remote.rs)、[更新](../../crates/server/src/routes/local_remote.rs)

Issue の分類と関係はそれぞれ別の意味を持つ。

| 情報 | 意味と保存上の関係 |
| --- | --- |
| Parent / sub-issue | 仕事の分解。親削除時は子の親参照が null になる |
| Assignee | 担当者。Issue と user の複数関連を保存する |
| Follower | 追跡する利用者。担当者とは別の関連 |
| Tag | Project 内の分類ラベル。Issue と多対多で結ぶ |
| Relationship | blocking / related / has_duplicate を保存する |

この関係を [Workflow の制御エッジ](workflow-attempt.md) と同じ依存グラフとして扱う根拠はない。表の保存契約は [スキーマ](../../crates/db/migrations/20260427000000_local_kanban.sql#L125-L179) と [親参照](../../crates/db/migrations/20260427000000_local_kanban.sql#L58-L68) を参照。

部分更新では「送らなかった」と「null に消した」を区別する。description などは二重 Option であり、未指定なら保持し、明示 null ならクリアする。priority の明示 null は上記の medium 正規化が例外となる。フォームや API クライアントの変更でこの違いを失うと、無関係な編集で情報を消し得る。[更新リクエスト](../../crates/api-types/src/issue.rs#L79-L146)、[適用](../../crates/server/src/routes/local_remote.rs)

### 採番の寿命と移行の限界

高水位は表示 key や Issue 行ではなく Project UUID に所属する。archive / unarchive、key の変更、DB 接続の再生成でも低下せず、Project 自体の削除でその寿命を終える。通常 API は key 更新を公開していないが、DB 上の key が変わっても既存 Issue の UUID・番号・simple_id は変えない。直接 INSERT や番号 / Project の更新も trigger が高水位へ記録し、高水位の低下は拒否する。ただし直接 SQL による過去番号の指定まで禁止する allocator ではない。[前方 migration](../../crates/db/migrations/20261027090100_issue_number_high_water.sql#L1-L26)・[削除・再接続・key の回帰](../../crates/db/src/models/issue_number_tests.rs#L63-L123)

allocator は caller の `BEGIN IMMEDIATE` 内で `UPDATE … RETURNING` し、Issue INSERT・Tag 等と一緒に commit する。予約だけを commit したり、未 commit 番号を外部公開したりしない。失敗時は transaction 全体を rollback するので、未公開予約は次の作成で使用できる。上限は i32::MAX であり、到達後は明示エラーとなる。通常作成 API は BadRequest、DB-only caller は既存 SQLx エラー境界へ変換し、wrap や現存 MAX への fallback はしない。[予約とエラー](../../crates/db/src/models/issue_number.rs#L4-L46)・[通常作成の変換](../../crates/server/src/routes/local_remote.rs#L797-L817)・[並列・rollback・上限の回帰](../../crates/db/src/models/issue_number_tests.rs#L126-L173)

migration は archive を含む現存最大値（空なら0）を初期値にし、既存 Issue の識別子を変更しない。導入前に削除済みの最大番号は、番号を持たない remote create receipt や不完全な audit から復元できないため、過去全体の非再利用は保証しない。導入時は Project UUID と対応する信頼できる保持履歴を確認し、必要なら確認済み最大値以上の floor を計画する。履歴不足なら初期値・根拠・保証不能期間を記録する。今回の実装は live DB 修復や既存番号の付替えを行わない。[運用上の根拠と未解決条件](../../docs/development/issue-numbering.md#migration-と過去履歴の限界)・[履歴不足を含む移行回帰](../../crates/db/src/models/issue_number_tests.rs#L176-L222)

## Kanban と実行の同期契約

以下は通常の Issue に対する一般 Agent 同期である。[生成専用 Issue](#生成-issue-の状態契約)は別の product 投影に従う。Not Started は未着手、In Progress は着手済み・未完了、In Review は自動同期上のレビュー待ちを表す。In Progress だけから「今も実行中」と判断せず、[AgentRun の状態](session-and-agent-run.md#状態停止送信待ち)を見る。Workspace 作成や Starting だけでは列を動かさず、開始の証拠を持つ Running で移動する。成功でも Done にはせず、[自動マージ の公開後完了](#正式統合と-card-の完了)と分ける。[同期処理](../../crates/db/src/models/agent_runtime_kanban.rs#L13-L109)

| 観測と条件 | 自動同期の結果 |
| --- | --- |
| 最新の実開始 attempt が初めて Running になる | 可視の Not Started / In Progress / In Review / Done から In Progress へ移し、その attempt を owner とする |
| 最新実開始 attempt が成功し、その owner を保持、他の関連 run も非活動になった | 可視の In Progress から In Review へ移す |
| 開始前の失敗、または最新実開始 attempt の失敗・取消・crash・AuditFailed | 成功として列を進めない。開始後は通常 In Progress を保持する |
| 独自名の列、非表示の現在列、必要な可視の移動先がない | 自動移動しない |

移動先は同じ Project の名前が一致する可視列から sort_order 順の先頭を選ぶ。名前を変更した独自列を推測して対応付けない。追加実行は In Review や Done から再び In Progress に戻れる。Done でも予約・Starting・開始前の失敗では列を変えず、最新の新しい実開始を必要とする。[列の選択と制約](../../crates/db/src/models/agent_runtime_kanban.rs#L63-L109)・[開始と失敗の回帰](../../crates/db/src/models/agent_runtime_kanban_tests.rs#L81-L202)

Done からの再着手は既存の Running 同期で扱い、別の再開処理は持たない。旧 attempt の Running・終端再送では再着手させず、過去の 自動マージ receipt も新しい作業を Done に戻さない。[公開後 receipt の境界](formal-integration.md#公開donewiki-は別の結果)と、両 canonical 保存経路で一巡を確認する [DB 回帰テスト](../../crates/db/src/models/agent_runtime_kanban_tests.rs#L578-L706)を参照する。

### 複数 Workspace と開始順

最新とは Issue に現在 link された全 Workspace の attempt のうち、`agent_process_registry.process_started_at` があるものを全精度の DateTime で比較した最大値である。受付順、event 到着順、完了順は使わない。完全に同時刻なら attempt UUID の大小を決定的な tie-break に使う。これは物理的な先後の証拠ではなく、ホスト時計の逆行時の真の開始順もこの実装からは保証できない。開始根拠が欠けていれば到着時刻で補わない。[比較の実装と理由](../../crates/db/src/models/agent_runtime_kanban.rs#L40-L61)・[1ns 差、同時刻、配送順の回帰](../../crates/db/src/models/agent_runtime_kanban_tests.rs#L474-L575)

終端時は最新 attempt の成功と owner を確認し、別 run に Pending / Starting / Running / AwaitingInput / AwaitingApproval / Cancelling があれば待つ。最新 attempt が既に成功していても古い run が活動中なら移動せず、その古い run の終端（失敗を含む）で条件が揃って In Review へ進む場合がある。したがって「失敗イベントでは絶対に列が変わらない」という一般化はしない。最新実開始 attempt 自身の失敗を、古い成功で隠すことはない。[終端条件](../../crates/db/src/models/agent_runtime_kanban.rs#L86-L102)・[待機の回帰](../../crates/db/src/models/agent_runtime_kanban_tests.rs#L288-L323)

### 手動操作と再送

非公開の `local_issues.agent_status_owner` は自動移動を行った attempt の token である。status_id が変わると DB trigger が解除し、共有更新 API は同じ列の明示指定でも解除する。タイトルだけの編集では保持する。解除後の旧 Running 再送や成功で手動変更・再オープンを取り消さない。ただし、その後の新しい実開始は対象の標準列なら新たな owner を取得できる。これは恒久的な自動同期オフ設定ではない。[trigger](../../crates/db/migrations/20261020190000_kanban_lifecycle.sql#L5-L13)・[共有更新 API](../../crates/server/src/routes/local_remote.rs#L807-L859)・[同列指定とタイトルの回帰](../../crates/server/src/routes/local_remote.rs#L3646-L3682)。Web と外部 MCP はこの更新処理を共有する。[MCP の更新](../../crates/server/src/remote_mcp/issue_write.rs#L213-L233)

同期は [canonical event 保存の原子性](../architecture/agent-runtime.md#canonical-event-から-board-への副作用)に従う。別 scheduler や UI ごとの終了フックは持たず、再送や projection rebuild を新しい開始と扱わない。

### 既存 Todo 列の移行

migration は名前 Todo、色 `210 80% 52%`、sort_order 100、hidden 0 がすべて一致する列だけを Not Started に改名する。列 ID と Issue の参照は保ち、カスタマイズ済み列は残す。これは既定属性一致による判定であり、生成元を証明する仕組みではない。[移行](../../crates/db/migrations/20261020190000_kanban_lifecycle.sql#L1-L3)・[ID とカスタム列の回帰](../../crates/db/src/models/agent_runtime_kanban_tests.rs#L247-L285)。独立した [Project Todo](project-todos.md) と旧 TaskStatus の API 表現は改名の対象ではない。

## 生成 Issue の状態契約

自動マージ・OpenWiki Bootstrap / Sync が内部生成した専用 Issue は、製品実行の進行と結果を表す。`system` Tag や名前による判別ではなく、`generated_issue_workspaces` の生成時 provenance と `generated_issue_status` の永続 receipt を使う。現在の Workspace owner kind / run / repository、Issue / Project link と製品 run の対応を検査して投影する。一般 Agent 同期は receipt のある生成 Issue を除外し、無関係な Agent の終了で専用 Card を戻さない。[投影の identity](../../crates/db/migrations/20261027090000_generated_issue_status.sql#L1-L55)・[一般経路の除外](../../crates/db/src/models/agent_runtime_kanban.rs#L41-L54)

| 製品 | In Progress / In Review の根拠 | Done の根拠 |
| --- | --- | --- |
| 自動マージ | 通常進行・再開は In Progress。blocked / failed / cancelled / recovery_required と未公開 succeeded は In Review。integrating 中のみ現在の担当 Agent の待機・失敗も反映 | payload の `published=true`。後続 Wiki や後着 Agent の失敗とは分離 |
| Bootstrap | Workflow の awaiting_human / awaiting_arena / failed / canceled、または実行中 node の child Agent の入力・承認待ちは In Review。それ以外の進行は In Progress | host の publication End を通った Workflow succeeded |
| Sync | 現在の Agent の入力・承認待ちや失敗・取消・crash・audit_failed、owner result の failed / cancelled は In Review。再開は In Progress | 対応 Agent が存在し、owner result が succeeded。公開検証済み no-op も含む |

Agent Succeeded 単独では公開成功としない。公開済みの自動マージは後着 Agent の失敗で引き戻されず、Done receipt は後着・重複の成功 / 失敗で再消費しない。DB の product 状態・owner・Agent / node 更新に伴う trigger で同じ transaction 内に列と完了日時を反映し、過去 AgentRun の監査・失敗記録は書き換えない。[状態と更新](../../crates/db/migrations/20261027090000_generated_issue_status.sql#L8-L183)・[製品別の回帰](../../crates/db/tests/generated_issue_status.rs#L144-L261)

移動先は同じ Project の可視列を名前の小文字値で照合し、sort_order / ID 順で選ぶ。必要な列がない場合は無関係な末尾列へ代替しない。Issue 全体の Cancelled / Canceled は operator の判断として保ち、公開後の手動再オープンも Done receipt の再送で消さない。一方、公開前の通常の手動列変更を永久 freeze する契約ではない。archive 属性は独立し、手動 archive 中でも対象の product 状態は投影される。[適用条件と receipt](../../crates/db/migrations/20261027090000_generated_issue_status.sql#L52-L103)・[取消・再オープン・列欠落・archive の回帰](../../crates/db/tests/generated_issue_status.rs#L264-L419)

upgrade はまだ有効な execution-only owner だけを登録し、Issue 行を一括更新しない。公開済みの継続 owner は done receipt のみ引き継いで手動再オープンを守り、解放済みの過去残留は登録・修復しない。次の通常イベントから現在の製品状態を反映する。[upgrade 条件](../../crates/db/migrations/20261027090000_generated_issue_status.sql#L57-L76)・[継続と履歴保持の回帰](../../crates/db/tests/generated_issue_status.rs#L422-L454)

公開成功・Issue の Done・[専用 Workspace の安全 archive](workspace.md#生成された専用-workspace-の自動-archive)は別の結果である。自動 archive は Cancelled を拒否し、状態 receipt がある対象は receipt と現在列の両方が Done であることを要する。停止や取消だけでは archive せず、process / cwd / lease、owner 解放、他 Workspace の既存 guard も維持する。[最終 guard](../../crates/server/src/routes/local_remote/issue_archive.rs#L450-L522)

## ボード表示と共有状態

Team / Personal は同じ Project の Issue 集合を見る filter preset であり、所有権・公開範囲を切り替えるものではない。既定の Team は担当者を絞らず手動順、Personal は現在 user の担当 Issue・優先度順で sub-issue も表示する。Project ごとの active view と、Project / view ごとの検索・sort・表示 option を UI preferences に保存する。[preset](../../packages/web-core/src/shared/stores/useUiPreferencesStore.ts#L82-L209)・[保存キー](../../packages/web-core/src/shared/stores/useUiPreferencesStore.ts#L744-L783)

保存先は [Scratch の runtime 境界](../architecture/web-and-sync.md#ドラフト表示設定通知の保存境界)に従う。現行 local-web は固定の UI preferences レコードを server Scratch に保存する。[Scratch 化](../../packages/web-core/src/shared/hooks/useUiPreferencesScratch.ts#L27-L98)・[保存](../../packages/web-core/src/shared/hooks/useUiPreferencesScratch.ts#L235-L260)

| 操作 | 変更する対象 |
| --- | --- |
| 検索、優先度・担当者・Tag の filter、表示 sort | view の設定。元 Issue を変更しない |
| 同じ列での手動並べ替え | 影響する Issue の共有 `sort_order`。Manual 以外では拒否 |
| 列をまたぐカード移動 | 移動した1件の `status_id` / `sort_order`。他 Issue の rank は維持 |
| Status の並べ替え・非表示設定 | Project Status の `sort_order` / `hidden`。下記の blocked 判定にも影響 |

[絞り込み](../../packages/web-core/src/features/kanban/model/hooks/useKanbanFilters.ts#L74-L169)・[カードの mutation](../../packages/web-core/src/features/kanban/ui/KanbanContainer.tsx#L684-L770)・[Status の保存](../../packages/web-core/src/shared/dialogs/settings/settings/RemoteProjectsSettingsSection.tsx#L821-L880)

`hideBlocked` は blocking 関係の元 Issue を blocker として調べる。blocker が **最後の可視 Status（sort_order 順）または任意の hidden Status** にあれば解決済みとして扱い、それ以外なら被ブロック Issue を隠す。参照先 blocker が手元の集合に存在しなければ、ブロック中と判定しない。名前が Done か、`completed_at` が埋まっているかという判定ではない。列の順序・非表示化は見た目だけでなく、この依存表示の意味を変える。[解決済み Status 集合](../../packages/web-core/src/features/kanban/ui/KanbanContainer.tsx#L311-L325)・[blocking の向きと判定](../../packages/web-core/src/features/kanban/model/hooks/useKanbanFilters.ts#L149-L169)

検索は title / simple_id / issue_number の部分一致で、Project / view 別設定を保持する。filter はカテゴリー内 OR、カテゴリー間は順に適用する AND である。[検索・filter](../../packages/web-core/src/features/kanban/model/hooks/useKanbanFilters.ts#L82-L147)・[Project 別設定の復元](../../packages/web-core/src/features/kanban/ui/KanbanContainer.tsx#L244-L285)

### コンパクトカードの表示契約

Kanban のカードは Issue を表し、内側の Workspace はその仕事の実施を表す。カードを小さくしても Issue と Workspace の 1:N 関係や [実行による列の同期](#kanban-と実行の同期契約)は変わらない。可視 Status ごとに列を描画し、その列の実幅が33.5rem以上なら内部のカードを二列、未満なら一列に置く。物理解像度や画面全体の幅で固定せず、三列や masonry にしない。標準 root 16px では閾値536 CSS pxで、二列時の16remカード二枚と gap・padding を確保する設計である。一列時のカード幅を常に16rem以上に制限する CSS ではない。[可視列](../../packages/web-core/src/features/kanban/ui/KanbanContainer.tsx#L468-L476)・[幅と通常 grid](../../packages/web-core/src/features/kanban/ui/kanban-grid.css#L3-L21)。[開発時の判断記録](../../tests/kanban/VALIDATION.md#変更と判断)は、文字縮小や一次元 DnD の CSS だけの grid 化を避けた理由を記録している。

Issue 番号・省略表示のタイトル・未 archive の関連 Workspace 数に加え、本文プレビューをタグ群の後へ最大2視覚行で表示する。native `details` は本文全文を開くために残す。`showWorkspaces` が有効なら通常 Workspace 一覧を折り畳みの外に表示し、無効なら一覧を隠す。WorkflowAttempt カードは従来どおり別に描画する。カードごとの 自動マージ 行は廃止し、自動マージ は Navbar の 自動マージ から開く。[本文](../../packages/ui/src/components/KanbanCardContent.tsx#L318-L329)・[全文](../../packages/web-core/src/features/kanban/ui/KanbanCardDetails.tsx#L34-L60)・[Workspace 一覧](../../packages/web-core/src/features/kanban/ui/KanbanContainer.tsx#L1300-L1420)・[自動マージ 入口](../../packages/web-core/src/features/kanban/ui/KanbanContainer.tsx#L1015-L1032)

タグは固定二件ではなく利用可能幅で可視数を決め、隠れた件数を `+N` で残す。長い先頭タグは省略して全文を title に保持し、既存のタグ編集操作を維持する。計測機構は [Web の描画責任](../architecture/web-and-sync.md#コンパクト-kanban-の操作と保存の境界)を参照する。

件数は未 archive の関連 Workspace 全体で、通常一覧は Workflow 所属 Workspace を除くため、表示枚数と一致を保証しない。列見出しの括弧件数は別の値で、filter 後の描画用 Issue ID 数を0も含めて示す。[件数と重複除外](../../packages/web-core/src/features/kanban/ui/KanbanContainer.tsx#L1230-L1270)・[列見出し](../../packages/web-core/src/features/kanban/ui/KanbanContainer.tsx#L1235-L1270)

Issue 集約の承認・入力・実行・失敗・未確認・DevServer 文字行は、Workspace 表示 ON/OFF とも描画しない。Priority と担当者は LOCAL-ID 行に置き、Workspace ごとの注意状態、読取状態、DevServer をアイコンで示す。注意状態は承認、入力、実行、失敗の順に選び、既読/未読は準備中・実行中等には出さず、DevServer の Play は独立する。pending / starting は Preparing とし実行時計を抑止する。非表示は状態がない証明ではなく、過去の全失敗を網羅する表示でもない。[details の責任](../../packages/web-core/src/features/kanban/ui/KanbanCardDetails.tsx)・[Workspace 判定](../../packages/ui/src/components/IssueWorkspaceCard.tsx#L125-L170)・[描画](../../packages/ui/src/components/IssueWorkspaceCard.tsx#L250-L320)。操作と snapshot の収束、ghost、fixture の限界は [Web の操作と保存](../architecture/web-and-sync.md#コンパクト-kanban-の操作と保存の境界)を参照する。

### Activity による手動順序の更新

本文保存、新規コメント、canonical の Succeeded / Failed / Cancelled / Crashed / AuditFailed の初回終端は、対象 Issue だけを現在列の先頭へ移す。stream の文字追加、単なる Running、同じ終端の再送では順位を変えない。全カードを更新日時で並べ替えたり、他カードを renumber したりしない。列の status 同期とは独立で、失敗・取消でも activity rank は更新できる。[共通順位処理](../../crates/db/src/models/issue_order.rs#L1-L83)・[本文保存](../../crates/server/src/routes/local_remote.rs#L879-L963)・[全終端の条件](../../crates/db/src/models/agent_runtime_kanban.rs#L7-L23)・[両保存経路の回帰](../../crates/db/src/models/agent_runtime_kanban_tests.rs#L750-L865)

共通の global UI_PREFERENCES Scratch に保存された Project / active view / sort をイベントの transaction 内で読む。Manual (`sort_order`) のときだけ rank を変え、降順では最大側を先頭にする。title / priority / created_at / updated_at の明示 sort 中は、裏の手動 rank も更新しない。Board 非表示中も保存済み値が基準となり、複数 browser の違う選択や設定保存失敗時には、各画面の未保存の意図まで保証しない。[保存済み sort の解釈](../../crates/db/src/models/issue_order.rs#L5-L83)

順位だけの promotion は Issue `updated_at` を変えず、コメントの review target を順位更新だけで古くしない。本文・新規コメントや canonical event の transaction と一緒に commit / rollback する。drag 中の昇格は [Web の rank 計算](../architecture/web-and-sync.md#コンパクト-kanban-の操作と保存の境界)で扱い、DB は `expected_sort_order` と現在値が違えば古い drop rank を捨てる。列間移動は新しい activity を移動先の先頭へ持ち越す。これも明示 sort 中には promotion を停止する。[書込境界](../../crates/server/src/routes/local_remote.rs#L879-L963)・[遅延昇格の回帰](../../crates/server/src/routes/local_remote.rs#L3630-L3730)

## Issue のアーカイブ

`archived` は仕事を通常の表示対象から外す属性で、Status・Workspace archive・Arenaの候補状態とは独立する。archive/解除は status を変えず、既存関連 Workspace の実行と通常のKanban自動同期も継続する。新規 Workspace・候補・Workflow主Workspaceの作成および新しいlinkは拒否し、解除後に再び可能となる。APIのguardに加えDB triggerで全writerを保護する。[schemaとリンク境界](../../crates/db/migrations/20261025150000_issue_archive.sql)・[既存実施との境界](../../crates/server/src/routes/workspaces/create.rs#L234-L248)。

UIのActive / Archived / AllとKanban / Listは独立した表示軸である。hidden StatusのIssueは両形式とも表示せず、検索・filter・選択も同じ表示集合に従う。対象tabと形式は非永続storeにあり、Issue詳細のroute remountを越えて保持するが、Projectを離れる・切り替える・reloadで既定Active/Kanbanへ戻る。Team/PersonalのScratch設定とは寿命が違う。[集合](../../packages/web-core/src/features/kanban/ui/KanbanContainer.tsx#L335-L364)・[画面state](../../packages/web-core/src/shared/stores/useKanbanDisplayStore.ts)。MCP一覧は省略/falseが未archive、trueがarchive済み、`"all"` が両方であり、UIのhiddenとは別条件である。[共通入力](../../crates/api-types/src/issue_filter.rs#L7-L56)。

一回の確認でIssueと全リンク集合、Workspaceのpath/branch/archive状態、checkboxを固定する。checkboxは毎回ONを既定にし、ONなら未archiveの配下Workspaceも既存の[手動archive](workspace.md#archive削除期限-cleanup)で処理する。OFFとIssueの解除はWorkspace・process・fileを変更しない。tokenはcaller/targetに束縛された10分の一回用で、リンクrevisionも比較するためunlink/relinkのABAも再確認となる。[prepare](../../crates/server/src/routes/local_remote/issue_archive.rs#L170-L223)・[リンクrevision](../../crates/db/migrations/20261025150000_issue_archive.sql#L16-L25)。

実行順はMUTATIONS → Issue作成guard → UUID順のWorkspace lifecycle lock → SQLiteのBEGIN IMMEDIATE。全対象のscript・DB idle・実processの再確認が済む前には論理/物理変更しない。その後にIssue flagとWorkspace archiveを更新する。DB履歴とGit branchは保持するがworktreeは削除対象となる。普通の削除失敗はWorkspaceごとのdeletion_errorとして返し、物理変更後のDB/commit失敗はrollbackでファイルが戻ったと証明できないため `uncertain` とする。[確認と結果](../../crates/server/src/routes/local_remote/issue_archive.rs#L223-L318)。

UI/stdio/外部HTTPは共通primitiveと耐久receiptを使う。応答喪失・再起動・uncertainでは同じrequest_idの結果と現在値を読み、別IDで盲目的に再実行しない。UIは結果確認を提供する。[UI回復](../../packages/web-core/src/shared/dialogs/kanban/IssueArchiveDialog.tsx#L65-L113)・[receipt入口](../../crates/server/src/routes/local_remote/issue_archive.rs#L349-L436)。変更時は `tests/issue-archive/migration.py`、serverの全対象busy/ABA/DB fault fixture、DB自動status同期、両MCPと `tests/issue-archive/isolated.mjs` を照合する。今回の保守は試験定義の静的照合であり、稼働DBや物理削除の再実行ではない。

## Issue の共同作業

担当・Follower・Comment / reaction の保存と、空の local 通知一覧、OS 向け実行通知は [Issue の共同作業](issue-collaboration.md)に集約する。旧 Cloud Attachment の保存・配送契約は現行構成にはない。

外部 AI の仕様レビューも同じコメントに追記する。レビュー対象の Issue revision を保持するが、投稿自体で要求・status を更新したり実装を始めたりはしない。来歴と編集、全文取得の契約は [共有レビュー](issue-collaboration.md#共有レビューは通常コメントとして残す)を参照する。[投稿処理](../../crates/server/src/remote_mcp/issue_comments.rs#L135-L153)

## Issue から実施へ

通常の Workspace 作成は `linked_issue` を受け取り、コンテナーと Session を作る前に関連付けを行う。ローカルでは `local_workspace_links.workspace_id` が主キーで、一つの Workspace の関連先を upsert する。一件の Issue は複数 Workspace を持てる。関連解除はリンクの削除であり、Workspace の作業ファイル削除とは別の操作である。[作成順](../../crates/server/src/routes/workspaces/create.rs)、[関連付け・解除](../../crates/server/src/routes/workspaces/links.rs#L24-L59)

「実施」には複数の形がある。

- 単独 Workspace とその Session で進める。
- [Arena](arena.md) により同じ Issue の候補を並べ、設計または実装を比較する。
- [WorkflowAttempt](workflow-attempt.md) により Issue 専用の編集可能なグラフと実施環境を持つ。

WorkflowAttempt は Project と Issue の所属を検証し、通常のテンプレート一覧から隠す専用 Workflow を作って draft として保存する。Issue のタイトル・本文から起動入力を作る処理は、グラフの定義とは分離されている。[作成契約](../../crates/server/src/routes/workflows.rs#L632-L677)、[入力構築とテスト](../../packages/web-core/src/features/workflow/model/issueWorkflow.test.ts#L8-L25)。編集内容の競合や下書き保全は [WorkflowAttempt の保存契約](workflow-attempt.md)を、実際の操作の順序は [依頼から統合まで](../workflows/task-to-integration.md)を参照する。

### 正式統合と Card の完了

local Board の [自動マージ](formal-integration.md)は採用 commit を固定し、同じ専用会話で候補作成・競合修復と必要な検証判断を行い、有効なWiki更新後にhostが公開する。Git 公開後に採用元の選択 Card を完了列へ動かし、専用 自動マージ Card は [生成状態投影](#生成-issue-の状態契約)で Done を記録する。要件 revision や link 変更の一致は現在の Done 条件ではない。Done はコード到達先や Wiki の新鮮さの証明にもならない。条件・例外・原子的 receipt は [公開・Done・Wiki の境界](formal-integration.md#公開donewiki-は別の結果)を参照する。

通常の link / unlink は interactive 用途を要求し、既存 Workspace の unlink は 自動マージ guard も確認する。execution-only の間は通常作業への付け替えを拒否するが、終了・cleanup 後の用途解除は可能である。[link/unlink](../../crates/server/src/routes/workspaces/links.rs)、[用途と所有者](workspace.md#操作用途と実行所有者)。内部製品が自身の Card を付ける処理と、通常ユーザーの link API は区別する。

## 表示モデルとローカル保存

共通 API の Project は organization ID と表示メタデータを含む。一方、ローカル DB の `projects` は作業ディレクトリ既定値と remote project ID を持ち、`local_project_metadata` を join してボード向けの形にする。同じ名前の Rust 型だけから同じ保存モデルだと判断しない。[共通型](../../crates/api-types/src/project.rs#L8-L17)、[ローカル型](../../crates/db/src/models/project.rs#L7-L17)、[組み立て](../../crates/server/src/routes/local_remote.rs)

ローカル API は固定 Local organization を返すが、公開配置の利用者とroleは認証actorに基づく。承認済みメンバーは全Projectを使い、Projectはtenant隔離を作らない。[メンバーと履歴identity](organization-and-membership.md)。Web は REST snapshot で同期する。[取得と mutation の収束](../architecture/web-and-sync.md)。

Workspace link は現在ローカル表への upsert、unlink はその削除だけであり、Cloud 呼出しとの部分失敗モデルは撤去された。unlink は既に削除済みの Workspace ID も受け付ける。[実装とテスト](../../crates/server/src/routes/workspaces/links.rs)。

## 旧 Task を読むとき

`tasks` と固定 enum の `TaskStatus` はソースに残る。ローカル Kanban 導入 migration は Task ID を保って `local_issues` へコピーし、旧 `workspaces.task_id` からリンク表を初期化した。現在のボード取得はリンク表と `local_issues` を join する。このため、旧 Task の状態や `task_id` だけを追って現行 Issue の挙動を説明すると境界を見失う。[移行](../../crates/db/migrations/20260427000000_local_kanban.sql#L75-L123)、[リンク移行](../../crates/db/migrations/20260427000000_local_kanban.sql#L225-L231)、[現行取得](../../crates/server/src/routes/local_remote.rs)

Repo はパス、対象ブランチ既定値、setup/cleanup/dev script などの実行資源を持つ。[Workspace](workspace.md) がそれを選択して作業環境を構成するので、Project を Git リポジトリ一個と同一視しない。[Repo の責任](../../crates/db/src/models/repo.rs#L36-L54)

### 内部生成 Card の成功後 archive

新規の OpenWiki Bootstrap / Sync と自動マージ が生成した専用 Card は、生成元と最初の owner run の保存記録を根拠に、成功・owner 解放後の専用 Workspace archive に伴って非表示になり得る。Cancelled や公開後に再オープンした生成 Issue は自動 archive しない。同じ Issue に未 archive の別 Workspace があれば Issue は残し、その別 Workspace は変更しない。archive 自体は Status・本文・履歴・Git branch を保持する。生成状態の投影は別に継続する。system Tag や名称だけでは資格にならず、過去の生成 Card を一括回収する仕組みでもない。[自動 archive の資格と失敗契約](workspace.md#生成された専用-workspace-の自動-archive)・[生成元記録](../../crates/db/migrations/20261026010000_generated_archive.sql#L1-L11)・[Issue の条件付き更新](../../crates/server/src/routes/local_remote/issue_archive.rs#L506-L513)

手動 archive / unarchive は自動後処理の資格も消費するため、解除した Card を古い完了通知が再び archive しない。人間の確認 token / Operations receipt と内部生成元の一度限りの結果は別の記録である。[失効 trigger](../../crates/db/migrations/20261026010000_generated_archive.sql#L12-L24)・[内部 attempt](../../crates/server/src/routes/local_remote/issue_archive.rs#L450-L470)

今回の採番・生成状態の Sync は凍結 source `7d3bd3e637580c18d5311ec7a5b7466d203683ef` の migration、通常 / system 作成 caller、canonical 同期、公開後処理、archive guard と独立した回帰定義を静的照合した。供給 Manifest の最終候補検証成功は過去の実行証拠であり、この保守で製品 suite や live DB 操作を再実行していない。無関係な既存内容全体の再検証は主張しない。
