---
type: concept
title: Project・Issue と実施の関係
description: ボード上の仕事と実行環境を結ぶ Project・Issue の意味、ローカル保存、旧 Task との境界。
tags: [project, issue, kanban, task]
sources:
  - id: openwiki-source-11356f66af906ebb06f3ef2e
    resource: repo://crates/api-types/src/issue.rs
  - id: openwiki-source-e4c80fe5a2af8a4abd8a8684
    resource: repo://crates/db/migrations/20260427000000_local_kanban.sql
  - id: openwiki-source-5c1907109b5a72a746e85a0c
    resource: repo://crates/db/migrations/20261020190000_kanban_lifecycle.sql
  - id: openwiki-source-c2fc034438311f3aa02f5ee5
    resource: repo://crates/db/migrations/20261023110000_retain_design_notes_on_project_delete.sql
  - id: openwiki-source-9bc71fe4f6cbbaa6fa9168e1
    resource: repo://crates/db/src/models/agent_runtime_kanban_tests.rs
  - id: openwiki-source-5c693a62e6ccf09ccb60592f
    resource: repo://crates/db/src/models/agent_runtime_kanban.rs
  - id: openwiki-source-830e28055c4ae6f5464df312
    resource: repo://crates/db/src/models/design_note.rs
  - id: openwiki-source-c01fa644be961cb7ee12d2b5
    resource: repo://crates/db/src/models/integration.rs
  - id: openwiki-source-ccd357b1781e6e90e56fd858
    resource: repo://crates/db/src/models/repo.rs
  - id: openwiki-source-a43253b5be9686da077b8067
    resource: repo://crates/server/src/remote_mcp/issue_comments.rs
  - id: openwiki-source-b393e75029374eaf9cba5f7d
    resource: repo://crates/server/src/routes/integrations/runtime.rs
  - id: openwiki-source-1ffb01d1ba3953937e0771bf
    resource: repo://crates/server/src/routes/local_remote.rs
  - id: openwiki-source-a517c0f8e44d2cf0c440fbfc
    resource: repo://crates/server/src/routes/workflows.rs
  - id: openwiki-source-38de6ae4b89edddb12325c18
    resource: repo://crates/server/src/routes/workspaces/links.rs
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
  - id: openwiki-source-5ef837b2bc54d286b3dedd8f
    resource: repo://packages/web-core/src/shared/stores/useUiPreferencesStore.ts
generated: { by: "codex", at: "2026-10-24T02:16:46.128Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-24T02:16:46.128Z
---

# Project・Issue と実施の関係

**Project** は関連する仕事を集める単位で、Issue、ボードの Status、分類用 Tag を持つ。**Issue** は「何を達成するか」を表す仕事である。AI が作業する場所は [Workspace](workspace.md)、会話と実行は [Session・AgentRun](session-and-agent-run.md) が受け持つ。この分離により、一件の仕事に複数の実施や比較候補を関連付けられる。[Issue 契約](../../crates/api-types/src/issue.rs#L20-L40)、[関連表](../../crates/db/migrations/20260427000000_local_kanban.sql#L208-L223)

local Project は固定Organizationと現在actorの [表示モデル](organization-and-membership.md)に結び付く。Issue の担当者・Follower は認証権限とは別である。

Project には Issue と独立した [Todo 共有メモ](project-todos.md)もある。メモの完了は Card の Done や agent 実行を進めず、任意の同一 Project の Card 参照だけを持つ。Board の Not Started 列（旧 Todo）や旧 `tasks` と区別する。

設計の意図と変遷は [設計メモ](design-notes.md) に本文・独立追記として保持する。Project 削除時もその履歴は残るが、既存APIでのアクセスは閉じる。他機能の削除制約と、保持後の閲覧方針は [削除後の契約](design-notes.md#project-を削除した後の保持)を参照する。

## 仕事の識別と分類

Issue の主キーは UUID。表示用 `simple_id` と番号、Project、Status、本文、任意の優先度・期限・完了日時、親 Issue、拡張メタデータを持つ。ローカル作成は Project ごとの最大番号から `LOCAL-N` を採番する。DB の番号・simple ID の一意性も Project 内なので、複数 Project を横断する処理で表示文字列だけをグローバル ID と扱わない。[型](../../crates/api-types/src/issue.rs#L20-L40)、[採番](../../crates/server/src/routes/local_remote.rs)、[一意制約](../../crates/db/migrations/20260427000000_local_kanban.sql#L45-L69)

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

部分更新では「送らなかった」と「null に消した」を区別する。たとえば description や priority は二重 Option であり、未指定なら既存値を保持し、明示 null ならクリアする。フォームや API クライアントの変更でこの違いを失うと、無関係な編集で情報を消し得る。[更新リクエスト](../../crates/api-types/src/issue.rs#L79-L146)、[適用](../../crates/server/src/routes/local_remote.rs)

## Kanban と実行の同期契約

Not Started は未着手、In Progress は着手済み・未完了、In Review は自動同期上のレビュー待ちを表す。In Progress だけから「今も実行中」と判断せず、[AgentRun の状態](session-and-agent-run.md#状態停止送信待ち)を見る。Workspace 作成や Starting だけでは列を動かさず、開始の証拠を持つ Running で移動する。成功でも Done にはせず、[正式 Integration の公開後完了](#正式統合と-card-の完了)と分ける。[同期処理](../../crates/db/src/models/agent_runtime_kanban.rs#L13-L109)

| 観測と条件 | 自動同期の結果 |
| --- | --- |
| 最新の実開始 attempt が初めて Running になる | 可視の Not Started / In Progress / In Review / Done から In Progress へ移し、その attempt を owner とする |
| 最新実開始 attempt が成功し、その owner を保持、他の関連 run も非活動になった | 可視の In Progress から In Review へ移す |
| 開始前の失敗、または最新実開始 attempt の失敗・取消・crash・AuditFailed | 成功として列を進めない。開始後は通常 In Progress を保持する |
| 独自名の列、非表示の現在列、必要な可視の移動先がない | 自動移動しない |

移動先は同じ Project の名前が一致する可視列から sort_order 順の先頭を選ぶ。名前を変更した独自列を推測して対応付けない。追加実行は In Review や Done から再び In Progress に戻れる。Done でも予約・Starting・開始前の失敗では列を変えず、最新の新しい実開始を必要とする。[列の選択と制約](../../crates/db/src/models/agent_runtime_kanban.rs#L63-L109)・[開始と失敗の回帰](../../crates/db/src/models/agent_runtime_kanban_tests.rs#L81-L202)

Done からの再着手は既存の Running 同期で扱い、別の再開処理は持たない。旧 attempt の Running・終端再送では再着手させず、過去の Integration receipt も新しい作業を Done に戻さない。[公開後 receipt の境界](formal-integration.md#公開donewiki-は別の結果)と、両 canonical 保存経路で一巡を確認する [DB 回帰テスト](../../crates/db/src/models/agent_runtime_kanban_tests.rs#L578-L706)を参照する。

### 複数 Workspace と開始順

最新とは Issue に現在 link された全 Workspace の attempt のうち、`agent_process_registry.process_started_at` があるものを全精度の DateTime で比較した最大値である。受付順、event 到着順、完了順は使わない。完全に同時刻なら attempt UUID の大小を決定的な tie-break に使う。これは物理的な先後の証拠ではなく、ホスト時計の逆行時の真の開始順もこの実装からは保証できない。開始根拠が欠けていれば到着時刻で補わない。[比較の実装と理由](../../crates/db/src/models/agent_runtime_kanban.rs#L40-L61)・[1ns 差、同時刻、配送順の回帰](../../crates/db/src/models/agent_runtime_kanban_tests.rs#L474-L575)

終端時は最新 attempt の成功と owner を確認し、別 run に Pending / Starting / Running / AwaitingInput / AwaitingApproval / Cancelling があれば待つ。最新 attempt が既に成功していても古い run が活動中なら移動せず、その古い run の終端（失敗を含む）で条件が揃って In Review へ進む場合がある。したがって「失敗イベントでは絶対に列が変わらない」という一般化はしない。最新実開始 attempt 自身の失敗を、古い成功で隠すことはない。[終端条件](../../crates/db/src/models/agent_runtime_kanban.rs#L86-L102)・[待機の回帰](../../crates/db/src/models/agent_runtime_kanban_tests.rs#L288-L323)

### 手動操作と再送

非公開の `local_issues.agent_status_owner` は自動移動を行った attempt の token である。status_id が変わると DB trigger が解除し、共有更新 API は同じ列の明示指定でも解除する。タイトルだけの編集では保持する。解除後の旧 Running 再送や成功で手動変更・再オープンを取り消さない。ただし、その後の新しい実開始は対象の標準列なら新たな owner を取得できる。これは恒久的な自動同期オフ設定ではない。[trigger](../../crates/db/migrations/20261020190000_kanban_lifecycle.sql#L5-L13)・[共有更新 API](../../crates/server/src/routes/local_remote.rs#L807-L859)・[同列指定とタイトルの回帰](../../crates/server/src/routes/local_remote.rs#L3646-L3682)。Web と外部 MCP はこの更新処理を共有する。[MCP の更新](../../crates/server/src/remote_mcp/issue_write.rs#L213-L233)

同期は [canonical event 保存の原子性](../architecture/agent-runtime.md#canonical-event-から-board-への副作用)に従う。別 scheduler や UI ごとの終了フックは持たず、再送や projection rebuild を新しい開始と扱わない。

### 既存 Todo 列の移行

migration は名前 Todo、色 `210 80% 52%`、sort_order 100、hidden 0 がすべて一致する列だけを Not Started に改名する。列 ID と Issue の参照は保ち、カスタマイズ済み列は残す。これは既定属性一致による判定であり、生成元を証明する仕組みではない。[移行](../../crates/db/migrations/20261020190000_kanban_lifecycle.sql#L1-L3)・[ID とカスタム列の回帰](../../crates/db/src/models/agent_runtime_kanban_tests.rs#L247-L285)。独立した [Project Todo](project-todos.md) と旧 TaskStatus の API 表現は改名の対象ではない。

## ボード表示と共有状態

Team / Personal は同じ Project の Issue 集合を見る filter preset であり、所有権・公開範囲を切り替えるものではない。既定の Team は担当者を絞らず手動順、Personal は現在 user の担当 Issue・優先度順で sub-issue も表示する。Project ごとの active view と、Project / view ごとの検索・sort・表示 option を UI preferences に保存する。[preset](../../packages/web-core/src/shared/stores/useUiPreferencesStore.ts#L82-L209)・[保存キー](../../packages/web-core/src/shared/stores/useUiPreferencesStore.ts#L744-L783)

保存先は [Scratch の runtime 境界](../architecture/web-and-sync.md#ドラフト表示設定通知の保存境界)に従う。現行 local-web は固定の UI preferences レコードを server Scratch に保存する。[Scratch 化](../../packages/web-core/src/shared/hooks/useUiPreferencesScratch.ts#L27-L98)・[保存](../../packages/web-core/src/shared/hooks/useUiPreferencesScratch.ts#L235-L260)

| 操作 | 変更する対象 |
| --- | --- |
| 検索、優先度・担当者・Tag の filter、表示 sort | view の設定。元 Issue を変更しない |
| 同じ列での手動並べ替え | 影響する Issue の共有 `sort_order`。Manual 以外では拒否 |
| 列をまたぐカード移動 | 移動先 Issue 群の `status_id` / `sort_order`、移動元の順序 |
| Status の並べ替え・非表示設定 | Project Status の `sort_order` / `hidden`。下記の blocked 判定にも影響 |

[絞り込み](../../packages/web-core/src/features/kanban/model/hooks/useKanbanFilters.ts#L74-L169)・[カードの mutation](../../packages/web-core/src/features/kanban/ui/KanbanContainer.tsx#L712-L806)・[Status の保存](../../packages/web-core/src/shared/dialogs/settings/settings/RemoteProjectsSettingsSection.tsx#L821-L880)

`hideBlocked` は blocking 関係の元 Issue を blocker として調べる。blocker が **最後の可視 Status（sort_order 順）または任意の hidden Status** にあれば解決済みとして扱い、それ以外なら被ブロック Issue を隠す。参照先 blocker が手元の集合に存在しなければ、ブロック中と判定しない。名前が Done か、`completed_at` が埋まっているかという判定ではない。列の順序・非表示化は見た目だけでなく、この依存表示の意味を変える。[解決済み Status 集合](../../packages/web-core/src/features/kanban/ui/KanbanContainer.tsx#L311-L325)・[blocking の向きと判定](../../packages/web-core/src/features/kanban/model/hooks/useKanbanFilters.ts#L149-L169)

検索は title / simple_id / issue_number の部分一致で、Project / view 別設定を保持する。filter はカテゴリー内 OR、カテゴリー間は順に適用する AND である。[検索・filter](../../packages/web-core/src/features/kanban/model/hooks/useKanbanFilters.ts#L82-L147)・[Project 別設定の復元](../../packages/web-core/src/features/kanban/ui/KanbanContainer.tsx#L244-L285)

### コンパクトカードの表示契約

Kanban のカードは Issue を表し、内側の Workspace はその仕事の実施を表す。カードを小さくしても Issue と Workspace の 1:N 関係や [実行による列の同期](#kanban-と実行の同期契約)は変わらない。可視 Status ごとに列を描画し、その列の実幅が33.5rem以上なら内部のカードを二列、未満なら一列に置く。物理解像度や画面全体の幅で固定せず、三列や masonry にしない。標準 root 16px では閾値536 CSS pxで、二列時の16remカード二枚と gap・padding を確保する設計である。一列時のカード幅を常に16rem以上に制限する CSS ではない。[可視列](../../packages/web-core/src/features/kanban/ui/KanbanContainer.tsx#L468-L476)・[幅と通常 grid](../../packages/web-core/src/features/kanban/ui/kanban-grid.css#L3-L21)。[開発時の判断記録](../../tests/kanban/VALIDATION.md#変更と判断)は、文字縮小や一次元 DnD の CSS だけの grid 化を避けた理由を記録している。

Issue 番号と既存サイズ・省略表示のタイトル、未 archive の関連 Workspace 数を折り畳みの外に残す。native `details` を開くと本文と Workspace 一覧を読める。「Workspace 表示」設定は展開内の一覧に適用し、本文や重要状態を隠す設定ではない。WorkflowAttempt のカードと選択元 Card に対応する Integration の入口も展開の外にある。[タイトル](../../packages/ui/src/components/KanbanCardContent.tsx#L210-L241)・[展開](../../packages/web-core/src/features/kanban/ui/KanbanCardDetails.tsx#L34-L61)・[入口と一覧](../../packages/web-core/src/features/kanban/ui/KanbanContainer.tsx#L1249-L1326)

件数と状態集約は母集団が同じとは限らない。件数は未 archive の関連 Workspace 全体、状態は `local_workspace_id` があり、active Workspace 一覧にも存在するものから作る。Workflow 所属 Workspace も状態集約に含むが、展開内の通常 Workspace 一覧からは除き、WorkflowAttempt の表示と重複させない。[集約対象](../../packages/web-core/src/features/kanban/ui/KanbanContainer.tsx#L649-L721)・[件数と重複除外](../../packages/web-core/src/features/kanban/ui/KanbanContainer.tsx#L1283-L1326)。従って件数は展開後の通常 Workspace カード枚数を保証しない。

承認待ち、入力待ち、実行中、失敗、未確認更新、開発サーバーはそれぞれ独立したフラグとして集約する。別 Workspace が実行中でも承認待ちを隠さない。承認は `hasPendingApproval` または最新 status の `awaiting_approval`、入力は `awaiting_input`、失敗は `failed` / `crashed` / `audit_failed` を見る。実行中・未確認更新・開発サーバーは対応する summary field を使う。これは Issue の Status を再計算する処理ではなく、過去の全 Run の失敗履歴を網羅する表示でもない。[独立した判定](../../packages/web-core/src/features/kanban/ui/KanbanCardDetails.tsx#L5-L49)。取得失敗時の限界、キーボード・drag と既存保存処理の接続、および実 API と fixture の検証範囲は [Web の操作と保存の境界](../architecture/web-and-sync.md#コンパクト-kanban-の操作と保存の境界)を参照する。

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

local Board の [正式 Integration](formal-integration.md)は採用 commit を固定し、一件ずつ merge と必要な review を行い、最終候補を受付時の検証方針に従って処理する。Git 公開後に選択 Card と Integration Card を完了列へ動かす。要件 revision や link 変更の一致は現在の Done 条件ではない。Done はコード到達先や Wiki の新鮮さの証明にもならない。条件・例外・原子的 receipt は [公開・Done・Wiki の境界](formal-integration.md#公開donewiki-は別の結果)を参照する。

通常の link / unlink は interactive 用途を要求し、既存 Workspace の unlink は Integration guard も確認する。execution-only の間は通常作業への付け替えを拒否するが、終了・cleanup 後の用途解除は可能である。[link/unlink](../../crates/server/src/routes/workspaces/links.rs)、[用途と所有者](workspace.md#操作用途と実行所有者)。内部製品が自身の Card を付ける処理と、通常ユーザーの link API は区別する。

## 表示モデルとローカル保存

共通 API の Project は organization ID と表示メタデータを含む。一方、ローカル DB の `projects` は作業ディレクトリ既定値と remote project ID を持ち、`local_project_metadata` を join してボード向けの形にする。同じ名前の Rust 型だけから同じ保存モデルだと判断しない。[共通型](../../crates/api-types/src/project.rs#L8-L17)、[ローカル型](../../crates/db/src/models/project.rs#L7-L17)、[組み立て](../../crates/server/src/routes/local_remote.rs)

ローカル API は固定 Local organization を返すが、公開配置の利用者とroleは認証actorに基づく。承認済みメンバーは全Projectを使い、Projectはtenant隔離を作らない。[メンバーと履歴identity](organization-and-membership.md)。Web は REST snapshot で同期する。[取得と mutation の収束](../architecture/web-and-sync.md)。

Workspace link は現在ローカル表への upsert、unlink はその削除だけであり、Cloud 呼出しとの部分失敗モデルは撤去された。unlink は既に削除済みの Workspace ID も受け付ける。[実装とテスト](../../crates/server/src/routes/workspaces/links.rs)。

## 旧 Task を読むとき

`tasks` と固定 enum の `TaskStatus` はソースに残る。ローカル Kanban 導入 migration は Task ID を保って `local_issues` へコピーし、旧 `workspaces.task_id` からリンク表を初期化した。現在のボード取得はリンク表と `local_issues` を join する。このため、旧 Task の状態や `task_id` だけを追って現行 Issue の挙動を説明すると境界を見失う。[移行](../../crates/db/migrations/20260427000000_local_kanban.sql#L75-L123)、[リンク移行](../../crates/db/migrations/20260427000000_local_kanban.sql#L225-L231)、[現行取得](../../crates/server/src/routes/local_remote.rs)

Repo はパス、対象ブランチ既定値、setup/cleanup/dev script などの実行資源を持つ。[Workspace](workspace.md) がそれを選択して作業環境を構成するので、Project を Git リポジトリ一個と同一視しない。[Repo の責任](../../crates/db/src/models/repo.rs#L36-L54)
