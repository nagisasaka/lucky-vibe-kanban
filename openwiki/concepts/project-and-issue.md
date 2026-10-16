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
  - id: openwiki-source-c01fa644be961cb7ee12d2b5
    resource: repo://crates/db/src/models/integration.rs
  - id: openwiki-source-ccd357b1781e6e90e56fd858
    resource: repo://crates/db/src/models/repo.rs
  - id: openwiki-source-1ffb01d1ba3953937e0771bf
    resource: repo://crates/server/src/routes/local_remote.rs
  - id: openwiki-source-a517c0f8e44d2cf0c440fbfc
    resource: repo://crates/server/src/routes/workflows.rs
  - id: openwiki-source-38de6ae4b89edddb12325c18
    resource: repo://crates/server/src/routes/workspaces/links.rs
  - id: openwiki-source-36bc16e51274306c238a63a2
    resource: repo://packages/web-core/src/features/kanban/model/hooks/useKanbanFilters.ts
  - id: openwiki-source-e0c2de9e01617a8949b78360
    resource: repo://packages/web-core/src/features/kanban/ui/KanbanContainer.tsx
  - id: openwiki-source-00110cdcfec890895b99690a
    resource: repo://packages/web-core/src/shared/dialogs/settings/settings/RemoteProjectsSettingsSection.tsx
  - id: openwiki-source-b885bc17dfe85c4ca45223e9
    resource: repo://packages/web-core/src/shared/hooks/useUiPreferencesScratch.ts
  - id: openwiki-source-5ef837b2bc54d286b3dedd8f
    resource: repo://packages/web-core/src/shared/stores/useUiPreferencesStore.ts
generated: { by: "codex", at: "2026-10-16T11:50:57.361Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-16T11:50:57.361Z
---

# Project・Issue と実施の関係

**Project** は関連する仕事を集める単位で、Issue、ボードの Status、分類用 Tag を持つ。**Issue** は「何を達成するか」を表す仕事である。AI が作業する場所は [Workspace](workspace.md)、会話と実行は [Session・AgentRun](session-and-agent-run.md) が受け持つ。この分離により、一件の仕事に複数の実施や比較候補を関連付けられる。[Issue 契約](../../crates/api-types/src/issue.rs#L20-L40)、[関連表](../../crates/db/migrations/20260427000000_local_kanban.sql#L208-L223)

local Project は固定 [Organization / user](organization-and-membership.md)の表示モデルに結び付く。Issue の担当者・Follower は認証権限とは別である。

## 仕事の識別と分類

Issue の主キーは UUID。表示用 `simple_id` と番号、Project、Status、本文、任意の優先度・期限・完了日時、親 Issue、拡張メタデータを持つ。ローカル作成は Project ごとの最大番号から `LOCAL-N` を採番する。DB の番号・simple ID の一意性も Project 内なので、複数 Project を横断する処理で表示文字列だけをグローバル ID と扱わない。[型](../../crates/api-types/src/issue.rs#L20-L40)、[採番](../../crates/server/src/routes/local_remote.rs)、[一意制約](../../crates/db/migrations/20260427000000_local_kanban.sql#L45-L69)

Status は Project の列を参照する ID であり、エージェントのプロセス状態ではない。ローカルの初期列は Todo / In Progress / In Review / Done / Cancelled で、Cancelled は初期状態で非表示。更新 API は `status_id` と `completed_at` を別々の入力として保持するため、列を Done に変えるだけで完了日時が自動計算されるとは読めない。[初期列](../../crates/server/src/routes/local_remote.rs)、[更新](../../crates/server/src/routes/local_remote.rs)

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

## Issue の共同作業

担当・Follower・Comment / reaction の保存と、空の local 通知一覧、OS 向け実行通知は [Issue の共同作業](issue-collaboration.md)に集約する。旧 Cloud Attachment の保存・配送契約は現行構成にはない。

## Issue から実施へ

通常の Workspace 作成は `linked_issue` を受け取り、コンテナーと Session を作る前に関連付けを行う。ローカルでは `local_workspace_links.workspace_id` が主キーで、一つの Workspace の関連先を upsert する。一件の Issue は複数 Workspace を持てる。関連解除はリンクの削除であり、Workspace の作業ファイル削除とは別の操作である。[作成順](../../crates/server/src/routes/workspaces/create.rs)、[関連付け・解除](../../crates/server/src/routes/workspaces/links.rs#L24-L59)

「実施」には複数の形がある。

- 単独 Workspace とその Session で進める。
- [Arena](arena.md) により同じ Issue の候補を並べ、設計または実装を比較する。
- [WorkflowAttempt](workflow-attempt.md) により Issue 専用の編集可能なグラフと実施環境を持つ。

WorkflowAttempt は Project と Issue の所属を検証し、通常のテンプレート一覧から隠す専用 Workflow を作って draft として保存する。Issue のタイトル・本文から起動入力を作る処理は、グラフの定義とは分離されている。[作成契約](../../crates/server/src/routes/workflows.rs#L632-L677)、[入力構築とテスト](../../packages/web-core/src/features/workflow/model/issueWorkflow.test.ts#L8-L25)。編集内容の競合や下書き保全は [WorkflowAttempt の保存契約](workflow-attempt.md)を、実際の操作の順序は [依頼から統合まで](../workflows/task-to-integration.md)を参照する。

### 正式統合と Card の完了

local Board の [正式 Integration](formal-integration.md)は採用 commit を固定し、一件ずつ検証・review して Git 公開後に選択 Card と Integration Card を完了列へ動かす。要件 revision や link 変更の一致は現在の Done 条件ではない。Done はコード到達先や Wiki の新鮮さの証明にもならない。条件・例外・原子的 receipt は [公開・Done・Wiki の境界](formal-integration.md#公開donewiki-は別の結果)を参照する。

通常の link / unlink は interactive 用途を要求し、既存 Workspace の unlink は Integration guard も確認する。execution-only の間は通常作業への付け替えを拒否するが、終了・cleanup 後の用途解除は可能である。[link/unlink](../../crates/server/src/routes/workspaces/links.rs)、[用途と所有者](workspace.md#操作用途と実行所有者)。内部製品が自身の Card を付ける処理と、通常ユーザーの link API は区別する。

## 表示モデルとローカル保存

共通 API の Project は organization ID と表示メタデータを含む。一方、ローカル DB の `projects` は作業ディレクトリ既定値と remote project ID を持ち、`local_project_metadata` を join してボード向けの形にする。同じ名前の Rust 型だけから同じ保存モデルだと判断しない。[共通型](../../crates/api-types/src/project.rs#L8-L17)、[ローカル型](../../crates/db/src/models/project.rs#L7-L17)、[組み立て](../../crates/server/src/routes/local_remote.rs)

ローカル API は固定 Local organization と Local User/Admin を返す。これは多人数認可ではない。[固定表示](organization-and-membership.md)。Web は REST snapshot で同期する。[取得と mutation の収束](../architecture/web-and-sync.md)。

Workspace link は現在ローカル表への upsert、unlink はその削除だけであり、Cloud 呼出しとの部分失敗モデルは撤去された。unlink は既に削除済みの Workspace ID も受け付ける。[実装とテスト](../../crates/server/src/routes/workspaces/links.rs)。

## 旧 Task を読むとき

`tasks` と固定 enum の `TaskStatus` はソースに残る。ローカル Kanban 導入 migration は Task ID を保って `local_issues` へコピーし、旧 `workspaces.task_id` からリンク表を初期化した。現在のボード取得はリンク表と `local_issues` を join する。このため、旧 Task の状態や `task_id` だけを追って現行 Issue の挙動を説明すると境界を見失う。[移行](../../crates/db/migrations/20260427000000_local_kanban.sql#L75-L123)、[リンク移行](../../crates/db/migrations/20260427000000_local_kanban.sql#L225-L231)、[現行取得](../../crates/server/src/routes/local_remote.rs)

Repo はパス、対象ブランチ既定値、setup/cleanup/dev script などの実行資源を持つ。[Workspace](workspace.md) がそれを選択して作業環境を構成するので、Project を Git リポジトリ一個と同一視しない。[Repo の責任](../../crates/db/src/models/repo.rs#L36-L54)
