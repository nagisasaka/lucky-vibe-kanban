---
type: guide
title: クイックスタートと調査案内
description: lucky-vibe-kanban の目的、主要概念の説明先、変更時の調査経路、既存文書の適用範囲を案内する。
tags: [overview, onboarding, concepts, navigation]
sources:
  - id: openwiki-source-4e5231907394b801763cfa97
    resource: repo://crates/api-types/src/issue_tag.rs
  - id: openwiki-source-d6072e60b63fe041de50f89e
    resource: repo://crates/api-types/src/tag.rs
  - id: openwiki-source-b50ba7975e31fb9a56845b2c
    resource: repo://crates/db/migrations/20261016000000_remove_project_chat.sql
  - id: openwiki-source-2f63ca1582805a46da98d522
    resource: repo://crates/db/migrations/20261018180000_web_push.sql
  - id: openwiki-source-c2fc034438311f3aa02f5ee5
    resource: repo://crates/db/migrations/20261023110000_retain_design_notes_on_project_delete.sql
  - id: openwiki-source-830e28055c4ae6f5464df312
    resource: repo://crates/db/src/models/design_note.rs
  - id: openwiki-source-5144fccee237bf35dd134ccb
    resource: repo://crates/db/src/models/repository_state.rs
  - id: openwiki-source-3e5425718d87b437a12edd60
    resource: repo://crates/db/src/models/session.rs
  - id: openwiki-source-0ba09128382dd913fe9a4c4a
    resource: repo://crates/db/src/models/tag.rs
  - id: openwiki-source-c30cc36c4bbe315c59b8e38f
    resource: repo://crates/db/src/models/workspace_tokens.rs
  - id: openwiki-source-a3956271cbf62ae0786e7fc7
    resource: repo://crates/executors/src/env.rs
  - id: openwiki-source-f4e0f9b2ce513d3f8b8b070e
    resource: repo://crates/executors/src/executors/provider_adapter.rs
  - id: openwiki-source-5a8e26da42457b4b47bca3ee
    resource: repo://crates/local-deployment/src/process_host/approvals.rs
  - id: openwiki-source-0a300791bc056c6eee6a577a
    resource: repo://crates/mcp/src/bin/vibe_kanban_mcp.rs
  - id: openwiki-source-076036ed7bdac82d628d39d1
    resource: repo://crates/server/src/remote_mcp/environment_process.rs
  - id: openwiki-source-0801a7e5bbcc064c12effc80
    resource: repo://crates/server/src/remote_mcp/environment.rs
  - id: openwiki-source-235c61e44e26d792cb34ab1d
    resource: repo://crates/server/src/remote_mcp/execution_approvals.rs
  - id: openwiki-source-cb711830e5a5d11f35f1d1c7
    resource: repo://crates/server/src/remote_mcp/help.rs
  - id: openwiki-source-a43253b5be9686da077b8067
    resource: repo://crates/server/src/remote_mcp/issue_comments.rs
  - id: openwiki-source-9e15762bf7a432b7605f8466
    resource: repo://crates/server/src/remote_mcp/issue_write.rs
  - id: openwiki-source-71f837a8aa36443ad9688945
    resource: repo://crates/server/src/remote_mcp/mod.rs
  - id: openwiki-source-2ac65265d4518e3d0e1d338e
    resource: repo://crates/server/src/remote_mcp/repository_read.rs
  - id: openwiki-source-0545bff1ae2507a92137efaf
    resource: repo://crates/server/src/remote_mcp/repository_wiki.rs
  - id: openwiki-source-0987a696c656ec0d4f318f44
    resource: repo://crates/server/src/remote_mcp/run_events.rs
  - id: openwiki-source-4936f3b3e77c32b1d6e30fb0
    resource: repo://crates/server/src/repository_read.rs
  - id: openwiki-source-d40201a53108a0c612f229ef
    resource: repo://crates/server/src/routes/help.rs
  - id: openwiki-source-595d5d0308d670e38ef27fbf
    resource: repo://crates/server/src/routes/integrations/admission.rs
  - id: openwiki-source-5720d865d1c837f0e6b7dab7
    resource: repo://crates/server/src/routes/integrations/conversation.rs
  - id: openwiki-source-b393e75029374eaf9cba5f7d
    resource: repo://crates/server/src/routes/integrations/runtime.rs
  - id: openwiki-source-1ffb01d1ba3953937e0771bf
    resource: repo://crates/server/src/routes/local_remote.rs
  - id: openwiki-source-db663c02905b9785eff4f4af
    resource: repo://crates/server/src/routes/local_remote/issue_versions.rs
  - id: openwiki-source-231241ac3c842dcd67525890
    resource: repo://crates/server/src/routes/openwiki.rs
  - id: openwiki-source-3b873a675f85754e9183d162
    resource: repo://crates/server/src/routes/openwiki/management.rs
  - id: openwiki-source-4d617b0e2fa188c6ad63ae56
    resource: repo://crates/server/src/routes/sessions/questions.rs
  - id: openwiki-source-a517c0f8e44d2cf0c440fbfc
    resource: repo://crates/server/src/routes/workflows.rs
  - id: openwiki-source-6c4d6d22a9d50e0def61c50d
    resource: repo://crates/server/src/routes/workspaces/integration.rs
  - id: openwiki-source-5590f47e4cee001a35a42117
    resource: repo://crates/server/src/routes/workspaces/wiki.rs
  - id: openwiki-source-ae8c631b5b4385c934e47d60
    resource: repo://crates/services/src/services/config/versions/v9.rs
  - id: openwiki-source-6067be8d6f85caa8ac03638f
    resource: repo://crates/services/src/services/repository_memory.rs
  - id: openwiki-source-8dd059d50335f535b6d49e83
    resource: repo://crates/utils/src/agent_guides.rs
  - id: openwiki-source-00394af5c4b9d56480285bf6
    resource: repo://crates/utils/src/lvk_directory.rs
  - id: openwiki-source-19419edea1d312cc9f07ef2d
    resource: repo://crates/utils/src/path.rs
  - id: openwiki-source-63d2b16e8e208dce3e2032ac
    resource: repo://crates/workflow/src/presets.rs
  - id: openwiki-source-df6b7819295f53fc2381f23d
    resource: repo://deploy/server/compose.yaml
  - id: openwiki-source-89d0fb22262f0a0ac1df7d1e
    resource: repo://deploy/server/server.mjs
  - id: openwiki-source-5b54a58d1b51cd490b0e7162
    resource: repo://package.json
  - id: openwiki-source-a4deecf361e70bd3b2703d13
    resource: repo://packages/local-web/src/app/auth/serverAccess.ts
  - id: openwiki-source-f5c8f8cc45fdf36d98b214ce
    resource: repo://packages/local-web/src/app/entry/Bootstrap.tsx
  - id: openwiki-source-8e332afc7e2a06fab23f3226
    resource: repo://packages/ui/src/components/FileTagTypeaheadPlugin.tsx
  - id: openwiki-source-cbfe71b82135aa5496989f7e
    resource: repo://packages/web-core/src/features/pets/SidebarPet.tsx
  - id: openwiki-source-39fe27949ccee837db47d1b9
    resource: repo://packages/web-core/src/features/workflow/ui/WorkflowTemplateEditorPage.tsx
  - id: openwiki-source-b8d01ec27647e9c180f0e8bc
    resource: repo://packages/web-core/src/pages/kanban/IssueVersionEditor.tsx
  - id: openwiki-source-5b956d3d9d7f17c236d42ebb
    resource: repo://packages/web-core/src/pages/kanban/ProjectUtilities.tsx
  - id: openwiki-source-235a3eb75f22fdecdbd66a8f
    resource: repo://packages/web-core/src/shared/components/EnvironmentPanel.tsx
  - id: openwiki-source-867b74d482ff6cc365176503
    resource: repo://packages/web-core/src/shared/components/TerminalSection.tsx
  - id: openwiki-source-7e0da77c31130e66a4b635a5
    resource: repo://packages/web-core/src/shared/dialogs/command-bar/EditorSelectionDialog.tsx
  - id: openwiki-source-6ffd002a557f8328086cf8ea
    resource: repo://packages/web-core/src/shared/dialogs/shared/WorkspacesGuideDialog.tsx
  - id: openwiki-source-5d2382a0f231c830f95f103c
    resource: repo://packages/web-core/src/shared/dialogs/shared/WorkspaceTargetDialog.tsx
  - id: openwiki-source-ab4a5800fcfc16d8494e494a
    resource: repo://packages/web-core/src/shared/lib/localIdentity.ts
  - id: openwiki-source-d4726fdd33da0f7c2f1ae1ee
    resource: repo://packages/web-core/src/shared/lib/remoteApi.ts
  - id: openwiki-source-09147e83e2e70cb032ece9e7
    resource: repo://packages/web-core/src/shared/providers/TerminalProvider.tsx
  - id: openwiki-source-23775c3de52f3ab95a13cb8b
    resource: repo://README.md
generated: { by: "codex", at: "2026-11-12T04:13:28.445Z" }
verified:
  - by: openwiki/0.7.2
    at: 2026-11-12T04:13:28.445Z
---

# クイックスタートと調査案内

lucky-vibe-kanban（LVK）は、Issue で仕事を整理し、Workspace 内で coding agent を動かし、成果物をレビューして Git に統合する製品である。README はカンバンでの計画、agent 実行と変更のレビューを目的として掲げ、Git worktree・複数Session・visual agent workflow・local integration・OpenWikiを紹介する。[製品の目的](../README.md) 実装の境界は [システム構成](architecture/system.md)、一連の操作は [Issue から統合まで](workflows/task-to-integration.md) から読む。

稼働版の使い方を質問する場合は [対話ヘルプ](concepts/interactive-help.md) を読む。既存ガイドUIまたはremote MCPから既定coding agentへ質問でき、上位AIは `query_repository` の `source:"lvk"` でProjectを作らず同梱公開sourceを直接調べることもできる。開発開始・配布・稼働反映は別の指示で扱う。

## 最初の作業

1. [サーバーコンテナー](operations/server-container.md)の手順で LVK の Docker image を起動し、公開配置では [LVKの初回認証設定](concepts/server-access.md#初回設定と方式変更)、続いて実行環境内で coding agent の認証を済ませる。LVK は npm パッケージを公開しない。ソースから開発する場合は [開発環境と検証範囲](operations/development.md) を参照する。[配布版の案内](../README.md)
2. [Project / Issue](concepts/project-and-issue.md) と、変更先の [Repo / Workspace](concepts/workspace.md) を確認する。仕事の分類と、実ファイルを置く場所は別の設定である。
3. 単一Agent（通常Session）を既定とし、工程の引渡しが必要なら[固定Workflow preset](concepts/workflow-attempt.md)を選び、[Setup → 実行 → レビュー → 統合](workflows/task-to-integration.md) を進める。local Board で複数 Card の採用成果を統合する場合は [自動マージ](concepts/formal-integration.md)を使う。同じ割当Issue / Workspace / Sessionの担当が候補・必要な検証/根拠付き省略・同候補Wiki・公開要求まで進める。Wiki有効時はbegin前に正式prepareで原本を保存し、生成setup差分は未stageで残す。担当Run終了後にhostがwriter収束・限定復元・cleanを含む安全条件を検査してlocal targetへ公開する。検証省略は成功したテストを意味しない。
4. canonical repository memory は [OpenWiki 初期生成・Sync](operations/openwiki-maintenance.md)を Repo ごとに設定する。保存Issue本文とは独立しており、Card Context専用UI・Pipeline供給・旧Wiki互換は廃止した。[本文保護と必要時ガイド](concepts/card-context-and-llm-wiki.md)を参照する。成果物は [OpenWiki Viewer の表示元](operations/workspace-inspection.md#wiki-の本文ツリーと表示元を確認する)を確認し、閲覧可能と publication 成功を区別する。

サーバーに常駐させる場合は [単一サーバーコンテナー](operations/server-container.md)を参照する。信頼された一チーム向けHTTPS配布であり、データ永続化と実行プロセスの継続を区別する。旧 Cloud / relay は退役している。

旧 LVK npx ランチャーと Tauri desktop shell は撤去済みである。現在はブラウザーを使う。以前の server・MCP・desktop の起動設定を使っている場合は、[移行先と既存データの保持範囲](operations/server-container.md#旧-npx-ランチャーの撤去と移行)を確認する。[配布の移行案内](../README.md#distribution)

初回の準備と通常作業は上記の「最初の作業」と [Issueから統合まで](workflows/task-to-integration.md) を入口にする。現行Boardはlocal APIを使い、公開配置ではBasic/Auth0の認証actorを使う。旧Cloud sign-inやremote API baseは要求しない。入口認証と外部 MCP の認可は [接続境界](integrations/remote-access.md)を参照する。[local API](../packages/web-core/src/shared/lib/remoteApi.ts)

LVK独自のrepo内管理領域は原則 `.lvk/` に集約する。共有設定と実行時データの違い、標準位置を維持するAGENTS/CLAUDE/OpenWikiなどの例外、旧配置を移行しない方針は [Workspaceの所有境界](concepts/workspace.md#lvkの所有境界)を参照する。同一登録RepoのWorkspace間の一時受け渡しは `.lvk/shared/tmp/`、再生成可能な再利用はcache、継続保存はpersistentを使う。tmpはRun終了で消さず、受け渡し後の不要・非使用時だけ整理する。[共有領域](concepts/workspace.md#共有ディレクトリの意味)・[製品概要](../README.ja.md)

## 主要概念の説明先

| 知りたいこと | 説明の本拠地 |
| --- | --- |
| local Board の identity と Organization は何か | [Organization・Member・Invitation](concepts/organization-and-membership.md)。固定Organization、承認制メンバー、現在actorと過去ownerの区別 |
| LVKへ入る認証と設定の寿命は何か | [サーバー認証・設定・セッション](concepts/server-access.md)。Local/Public、初回setup、Basic/Auth0切替、失効、内部agentと回復 |
| 何を作るか、誰の仕事か、status や親子関係は何か | [Project・Issue](concepts/project-and-issue.md)。ボード表示と共有 status の差、Workspace との link と旧 Task の違いもここ |
| Pet の版・ZIP・保存・帰属を調べる | [装飾Pet](concepts/pets.md)。v1/v2、16方向視線、取込安全境界、添付保持、素材帰属、fixtureと実環境の区別 |
| 小さな作業メモと Card の要求をどう使い分けるか | [Project Todo](concepts/project-todos.md)。Project 所属、任意の Card link、完了・削除、revision 競合と操作 receipt |
| 設計意図と判断の変遷をどう残すか | [設計メモ](concepts/design-notes.md)。本文と独立追記、初版・復元、出典、競合、MCPの固定版読取、Project削除後の保持 |
| Issue の担当者・コメント・file 参照はどう保存するか | [Issue の共同作業](concepts/issue-collaboration.md)。共有レビューの来歴・対象版・全文読取、local コメント・reaction・follower と通知の実装範囲 |
| どこで変更するか、何を共有し、何を削除できるか | [Workspace・Repo](concepts/workspace.md)。Worktree / DirectFolder、物理所有権と interactive/execution_only の用途、共有領域、手動 archive の確認・物理削除・履歴保持 |
| 会話、実行、再試行をどう識別するか | [Session・AgentRun・RunAttempt](concepts/session-and-agent-run.md)。native session、現在の承認配線、未送信ドラフト、Goal、queue、子 agent の境界 |
| 外部会話から LVK を操作するには | [外部 MCP](integrations/remote-access.md)。認証・scope・操作 receipt。内蔵機能の履歴は [Astra 廃止と移行境界](concepts/project-chat.md) |
| 外部会話からログを読み既存アプリを動かすには | [実行環境へのアクセス](concepts/runtime-access.md)。files/execute設定・file roots、runtime scope、明示Project contextと直接processの寿命とAgentRun・共有資源との違い |
| タブを閉じた後の Codex 待機・終了を知りたい | [Web Push・通知同意・会話への復帰](integrations/web-push.md)。Project/browser 登録、logout と別の同意寿命、配送失敗、参照先への復帰 |
| 複数段階の仕事をどう定義して動かすか | [WorkflowAttempt](concepts/workflow-attempt.md)。template と Run、分岐・合流、人の判断待ち |
| 候補をどう比較し、採用後の実装へ移るか | [Arena](concepts/arena.md)。Design / Implementation、候補、promotion と Workflow 内の選択 |
| prompt の再利用文脈と並列 source をどう扱うか | [保存本文・共有領域・並列参照](concepts/card-context-and-llm-wiki.md)。既知5hash限定の送信時除去と引用保護、必要時ガイド、固定snapshotと旧機能撤去の境界 |
| 採用した並列成果を安全に local target へ反映するには | [自動マージ](concepts/formal-integration.md)。固定source/base、同じ会話の候補・検証/省略・Wiki、hostの公開と取消 |
| 並列作業の知識を統合済み repository にどう反映するか | [Repository memory](concepts/repository-memory.md)。canonical Wiki、ファイルChange Manifest、正常Run自動commit、DB運用状態とsource checkpoint |

名称だけを確認する場合は [自動マージの名称と互換境界](concepts/formal-integration.md#名称と互換境界)を読む。API・tool の識別子と画面名を区別し、マージ済みコードと候補内Wikiの完了と公開の結果を別々に確認する。

## 変更内容から調査する

| 変更・障害 | 調査経路 |
| --- | --- |
| 共有cache/tmpの容量を確認し手動で掃除したい | [容量と削除手順](operations/workspace-inspection.md#共有cachetmpの容量と手動削除)。Repository横断の影響、保守ownerの利用中拒否、失敗時の再計測と排他保証の限界を確認する |
| 起動、DB、standalone と残存 startup API の差 | [システム境界と起動順](architecture/system.md) → [開発・検証](operations/development.md) |
| Agent が止まった、表示と監査が合わない、再接続したい | [Session の状態](concepts/session-and-agent-run.md) → [Agent Runtime の監査・投影・回復](architecture/agent-runtime.md) |
| Workspace の累積トークン、内訳、不明・一部のみの意味 | [Workspace 累積トークン](concepts/workspace.md#workspace-累積トークン) → [画面で確認](operations/workspace-inspection.md#累積トークンを確認する)。context gauge・各 run 時間との違い、thread 単位の保持、MCP の同じ集計を確認する |
| Starting のまま進まない、準備中なのに時計が動く | [Starting の耐久回復](architecture/agent-runtime.md#starting-保存失敗と未起動の回復) → [Preparing と実開始時計](concepts/session-and-agent-run.md#run-の経過時間と-goal-会計)。有限保存予算・失敗フェンス、setup除外とhost ACK不確実性を確認する |
| account 利用枠や provider usage をモデルを動かさず読みたい | [受動quota/usageの正本](integrations/agent-providers.md#受動的な-account-quotausage-参照) → [外部MCPの認可](integrations/remote-access.md#provider-account-の利用枠を参照する)。server_default・DEFAULT所属・未知値とWorkspace tokensの違いを読む |
| Codex resume の固定指示が重複する | [履歴保持の証拠と全文fallback](integrations/agent-providers.md#codex-resume-の固定指示と現在-identity)。現在identityは毎回更新し、未知版・compaction等で固定文を省かない |
| DevServer を開始・停止・観測したい | [共通managerの運用](operations/workspace-inspection.md#preview-と開発サーバー) → [MCPのscopeとreceipt](integrations/remote-access.md#managed-devserver-を操作する)。再利用、部分起動、cleanup不確実性、readinessの違いを確認する |
| MCP tool 一回だけの権限要求が届いた | [Codexの正式承認](concepts/session-and-agent-run.md#codex-の一回限り-mcp-tool-承認)。元toolとの一意相関・明示回答・取消・永続grantなしの境界を確認する |
| コメント本人編集、activity順序、raw保存名を直したい | [本人編集](concepts/issue-collaboration.md#自分の既存コメント本文を編集する)、[手動rankと明示sort](concepts/project-and-issue.md#activity-による手動順序の更新)、[file配信](operations/workspace-inspection.md#repo-内ファイルを読む)へ進む。要求revision、順位、配信bytesを別に扱う |
| provider や model 設定、MCP、Skill の追加 | [Provider integration](integrations/agent-providers.md)。effort override と解決済み値、外部 MCP を使う方向と LVK MCP を公開する方向を分ける |
| Workflow の分岐、取消、outbox、再起動回復 | [Graph の契約](concepts/workflow-attempt.md) → [Workflow Runtime](architecture/workflow-runtime.md) |
| 定期的に作業を実行したい | [ScheduledTask の起動・skip 条件](workflows/task-to-integration.md#定時に-workflow-を起動する) |
| Card の列が動かない、失敗後も In Progress、追加実行で戻る | [Kanban と実行の同期契約](concepts/project-and-issue.md#kanban-と実行の同期契約)。実開始時刻、手動変更優先、他 run の待機と自動マージ の Done の違い |
| カード本文・タグ・Workspace 状況の見え方、差分の0と不明 | [コンパクトカードの契約](concepts/project-and-issue.md#コンパクトカードの表示契約) → [summary の差分集計](operations/workspace-inspection.md#workspace-summary-の差分集計)。描画設定・本文全文・正常0・部分取得失敗を区別する |
| 自動マージの停止後に同じ会話を続けたい | [同じ会話の候補と再開](concepts/formal-integration.md)。候補と証拠を保持し、停止理由を確認して同じ割当Sessionへ追加指示を送る |
| image の Claude Code と認証の準備 | [同梱 CLI と永続 home](operations/server-container.md#claude-code-の同梱と認証) → [Subscription profile の確認](integrations/agent-providers.md#claude-のサブスクリプション専用-profile)。実行ファイルの存在と account readiness を分ける |
| frontend、local Board の取得と更新 | [Web と同期](architecture/web-and-sync.md) → [外部 MCP と認証](integrations/remote-access.md) |
| ファイル、Session 添付、diff、preview、terminal、editor | [Workspace inspection](operations/workspace-inspection.md)。Board の参照は [Issue の共同作業](concepts/issue-collaboration.md) |
| 外部 MCP が認証・認可で失敗する | [Auth0 JWTとscope](integrations/remote-access.md)。active membership、read/write/merge と runtime専用scope、stateless transportの境界 |
| 実行環境の許可を保存・取り消す、旧形式・legacy設定を復旧する | [Runtime Access の管理設定](concepts/runtime-access.md#管理設定の保存適用取消)。DB優先・revision競合・明示復旧と、取消が停止を意味しない契約。旧設定の移行範囲とfiles/executeの現在値を正本で確認する |
| 外部会話から保存済み Codex 進捗を読みたい | [list_run_events](integrations/remote-access.md#保存済み-codex-進捗の読取)。明示 Run 参照、beginning/latest の初回窓、除外を含む cursor、全文credential検査と4096バイト短縮、最終回答との分離 |
| 既存 Goal を外部会話から一時停止・再開・取消・解除する | [Goal の寿命と保存再開](concepts/session-and-agent-run.md#goal-の寿命) → [MCP の対象固定と結果確認](integrations/remote-access.md#既存-goal-を-mcp-から操作する)。達成・解除・Run 終了の違いと、revision・receipt・不確実な配送を確認する |
| Plan が戻る、旧 Plan with Goal、context や子の表示が違う | [Session の Plan 遷移・context・子 lifecycle](concepts/session-and-agent-run.md#plan-の遷移) → [CLI / wire 型と mapper 互換](integrations/agent-providers.md#cli-検出と-provider-互換性) |
| GitHub の認証、push 先、PR 結果や merge 観測が合わない | [GitHub 認証と PR](integrations/github-review.md)。リポジトリ別 PAT、agent shell との適用差、push/base と部分成功 |
| サーバーの CPU・RAM・DISK と制限を読む | [画面の共通観測](architecture/web-and-sync.md#両ビューの共通右サイドバーと環境情報)と [外部 MCP](integrations/remote-access.md#オンデマンド資源観測)は同じ collector を使う。system-visible と cgroup、保存先の filesystem、unknown・部分失敗を区別する。資源予約の代わりにはならない |
| 開発 instance の成果物を切り替える、失敗後に復旧する | [切替 engine の運用](operations/development.md#開発用成果物切替と復旧)。専用 MCP の [自己再起動](concepts/runtime-access.md#開発専用-self_restart)からの受付、排他、backup、停止証拠、durable 結果を区別する |
| 終了した Workspace のディスクを解放する | [手動 archive](concepts/workspace.md#archive削除期限-cleanup)。busy 検査、一度の確認、削除失敗と履歴保持。外部会話からは [MCP の準備・確認と結果読戻し](integrations/remote-access.md#workspace-を外部-mcp-から-archive-する)へ進む。通常の merge・PR・Arena の終了だけでは自動 archive しない |
| Wiki / 自動マージ の専用環境が成功後に消えた、または残っている | [生成 Workspace の自動 archive](concepts/workspace.md#生成された専用-workspace-の自動-archive)。生成元とowner解放、一度限りの試行、手動介入・削除失敗、専用Issueと採用元の保持を確認する |
| 外部マシンの実行を予約・回復する | [共有資源と Execution Bridge](concepts/shared-resource-coordination.md)。snapshot、Runner、settlement と operator 回復 |
| 実行の元データを保全したい | [Native Audit の保存・エクスポート・削除](architecture/agent-runtime.md#native-audit-の保全エクスポート寿命) |
| Merge / Rebase / PR 後の状態が合わない | [統合までの順序と部分失敗](workflows/task-to-integration.md) → [memory の統合契約](concepts/repository-memory.md) |
| Wiki を読む、表示対象が違う | [Wiki の本文・ツリーと表示元](operations/workspace-inspection.md#wiki-の本文ツリーと表示元を確認する) → [共有 reader とチャット保持](architecture/web-and-sync.md#wiki-のナビゲーションと本文を一つの状態で動かす) |
| 外部会話から既存 Wiki を同期・再同期する | [MCP Sync の固定対象・再送・結果読戻し](operations/openwiki-maintenance.md#mcp-から既存-wiki-を同期する)。受付成功、公開・照合完了、現在の鮮度を分け、初期生成は Repository settings で行う |
| Wiki 保守の Card がない、紐付け先 Project が違う | [開始元 Project と保守 Issue](operations/openwiki-maintenance.md#開始元-project-と保守-issue)。Project 設定/MCP の引継ぎ、関連検証と未指定時の互換動作を確認する |
| Wiki が stale/error、Bootstrap が途中で止まる | [OpenWiki の phase・共通索引・レポート・完了証拠・回復](operations/openwiki-maintenance.md)。MCP 起動前確認と公開条件を分けて診断する |
| 自動マージ の説明不足、古い候補、反映結果の照合中 | [保存済み解消説明と承認条件](concepts/formal-integration.md#保存済み説明から候補をレビューする) → [公開・Done・Wiki の区別](concepts/formal-integration.md#公開donewiki-は別の結果)。操作は [統合までの手順](workflows/task-to-integration.md#並列変更を試してから正式統合する) |
| 外部会話から自動マージを続ける・取り消す | [MCPの担当会話・証拠・取消receipt](integrations/remote-access.md#mcp-から-integration-の会話と取消を扱う)。割当Sessionへの指示、コードSHAとWiki SHA、取消受付と予約解放を区別する |
| 統合・Wiki 実行のログを見る、所有者を止める | [Execution の閲覧と停止](operations/workspace-inspection.md#実行専用-workspace-の確認)。通常開発用 Workspace と操作入口を分ける |
| local LVK を常駐サーバーへ置く | [単一サーバーコンテナー](operations/server-container.md)。HTTPSとbackend認証、独立アプリの認証、volume、停止と rollback |
| 同じコンテナーの独立アプリを DNS 名ごとに公開する | [独立アプリの CSV ルート](operations/server-container.md#独立アプリの-csv-ルート)。アプリ自身の認証、旧previewの廃止、証明書と再作成の順序を確認する |
| モバイル表示の検証 | [built preview と検証](operations/development.md)。稼働 API と分離した frontend bundle |

## 編集・接続の不調から調べる

ブラウザーの Markdown / HTML / raw paste、音の試聴は [Web の操作境界](architecture/web-and-sync.md#ブラウザー操作と廃止された-native-bridge)を確認する。native shell の廃止と共有機能の存続を分け、貼り付け fixture と実 OS clipboard・live 通知の検証範囲は [開発・検証](operations/development.md#ブラウザー貼り付けの回帰)で確認する。

- 外部エディターは server で起動せず、設定した URI の候補を明示クリックして browser / OS へ渡す。[共有設定・パス解決・非復元・適用不可の契約](operations/workspace-inspection.md#外部エディターの-uri-連携)から調べ、[選択 dialog の取得と表示](architecture/web-and-sync.md#通知音エディターの接続先)を確認する。
- 未送信本文が戻らない、前回の実行設定が違う場合は [Session の設定復元](concepts/session-and-agent-run.md#実行設定の復元と-default-identity)と [Scratch の ACK・pending backup](concepts/session-and-agent-run.md#未送信ドラフトと-scratch)を分けて確認する。
- Workflow の保存競合、再読込後の draft、Undo/Redo は [編集契約](concepts/workflow-attempt.md#編集保存競合undoredo)へ進む。Undo は実行や Git を戻す操作ではない。
- 設定値が非表示なのは未設定と同義ではない。[機密設定の明示読取・保持/置換/削除](integrations/agent-providers.md#設定の公開と変更の安全境界)を確認する。
- Agent 表示が止まっても、接続断を死亡と即断しない。[Host の再送と終了証拠](architecture/agent-runtime.md#接続断と確認済み終了を分ける)を確認する。差分の取得失敗も [正常な変更ゼロとは別](operations/workspace-inspection.md#差分の未取得エラー空を区別する)である。


## 小さな用語の境界

### Prompt 用 Tag と Issue の分類 Tag

Settings の Tag は tag_name/content を持つ、再利用する prompt text の保存モデルである。SettingsのTag管理で名前とcontentを保存し、promptに `@` と名前を入力して候補を選ぶと、contentがcursor位置へ挿入される。[入力処理](../packages/ui/src/components/FileTagTypeaheadPlugin.tsx#L387-L411) Issue の分類 Tag は project_id/name/color を持ち、IssueTag が issue_id と tag_id を関連づける。名前が同じでも保存対象と利用目的が異なる。[prompt Tag](../crates/db/src/models/tag.rs)・[分類 Tag](../crates/api-types/src/tag.rs)・[IssueTag](../crates/api-types/src/issue_tag.rs)

### Review の違い

Codexの **REVIEW execution mode** はCODE基盤の通常会話で調査・レビュー意図を保存し、報告して停止する。Workspace内の再現・テストを許すが製品実装には別指示が必要で、PLAN・Goal・native `/review` やOpenWikiの独立Reviewerとは別である。[実行境界](integrations/agent-providers.md#review-の実行境界)・[設定保存と続行](concepts/session-and-agent-run.md#review-は通常会話で報告して止まる)

Issue の [共有レビュー](concepts/issue-collaboration.md#共有レビューは通常コメントとして残す)は、外部 AI が通常コメントへ意見を残す仕組みであり、要求変更・実装開始・承認を意味しない。[投稿境界](../crates/server/src/remote_mcp/issue_comments.rs#L135-L153)

通常 Workspace の [Agent review](workflows/task-to-integration.md#3-会話を続け成果物を確認する) と [OpenWiki の独立 Review](operations/openwiki-maintenance.md) は別 lifecycle である。旧 Cloud Review の upload / GitHub App worker は現行 local 配布にない。

## 既存文書を使うとき

操作の詳細はWikiの説明先に集約する。Workspaceのファイル・差分・端末は [確認と実行](operations/workspace-inspection.md)、Boardsは [Project・Issue](concepts/project-and-issue.md)、agent/MCP設定は [provider連携](integrations/agent-providers.md)、Git認証とPRは [GitHub連携](integrations/github-review.md)、配置と復旧は [server運用](operations/server-container.md) を読む。

実装の確認には source・tests・実際の設定を使い、文書は目的・契約・過去の判断の証拠として状態を区別する。特に次の適用差は各説明先で扱う。

- Workflow の graph と prompt 展開は [WorkflowAttempt](concepts/workflow-attempt.md) の現行 validator を確認する。
- Bootstrap v2 の一部は後続 implementation record が更新している：[OpenWiki 設計文書](operations/openwiki-maintenance.md#設計文書の読み方)。
- [内蔵 Astra は廃止済み](concepts/project-chat.md)。旧モデル設定や専用会話履歴の説明を現行機能と扱わず、残る質問回答と外部 MCP の契約を確認する。
- 自己開発の旧 development container と固定 staging 候補は区別する：[開発・検証](operations/development.md)・[サーバー運用](operations/server-container.md)。

## カバー範囲と限界

この構成は、README と文書 index に加え、local server の router/startup、local Board と外部 MCP の router、Workflow/Agent runtime、Git と memory の境界から照合している。文書の章構成や直近差分だけを説明範囲にはしていない。[local API の入口](../crates/server/src/routes/mod.rs)・[外部 MCP](../crates/server/src/remote_mcp/mod.rs)・[MCP の入口](../crates/mcp/src/task_server/tools/mod.rs)

この Wiki は実装理解のための要約であり、すべての UI 項目や依存 package の一覧ではない。外部サービス、配布済み binary、IDE extension、model の意味的な出力品質は、この source 調査だけでは確定しない。製品 test の実行結果については [検証記録の限界](operations/development.md) を参照する。

過去の Sync は凍結された pending changes と統合済み source `066b2b4bd93d06d046865d66ae16bf1537e8b184` に沿い、Workspace 累積トークンを照合した。native adapter と親 thread scope、migration と snapshot/host cursor の保存、UI の query と HTTP router、stdio/外部 MCP、独立した Rust fixture と Chromium spec を入口として確認した。集計は取得済み Codex 親 thread の報告値であり、未取得は unknown、報告ありも partial で全量を保証しない。[集計契約](concepts/workspace.md#workspace-累積トークン)・[根拠となる応答](../crates/db/src/models/workspace_tokens.rs#L42-L70)・[検証の分担](operations/development.md#workspace-トークン集計の検証)。無関係な過去の説明全体を再検証したという意味ではない。

前回の通常 Sync は凍結された pending changes と統合済み source `e378eb281dde23cd9283c96e6db3cfae3379518b` を対象とし、[Runtime Access](concepts/runtime-access.md) の grant 簡素化を照合した。外部HTTPのscope入口、dispatchとProject照合、設定のDB/legacy読取とrevision付き保存、Linux process registryと再起動receipt、設定UI・生成型・日本語ヘルプ、HTTP/設定/exec/restartの各fixtureを独立した入口として確認した。subject／実行ファイル許可リストの撤去と、残る所有者・cwd・Workspace境界を正本ページへ反映した。関連する外部MCP・システム構成の説明も照合し、無関係な過去の内容全体を再検証したとは扱わない。[現行grantとdispatch](../crates/server/src/remote_mcp/environment.rs#L309-L403)・[exec遷移の回帰](../crates/server/src/remote_mcp/environment_process.rs#L640-L700)・[設定互換性の回帰](../crates/server/src/remote_mcp/environment_settings_tests.rs#L275-L275)

前回の Runtime Access の凍結入力は、開発時のテスト成功報告と自動マージ の検証省略を区別していた。これはその対象範囲の履歴であり、後続 自動マージ の検証結果を示すものではない。


### 共有レビュー Sync の照合範囲

統合済み source `be97e258698174883272eadcd325ae743d30b7d8` と凍結された pending changes を対象に、[Issue の共有レビュー](concepts/issue-collaboration.md#共有レビューは通常コメントとして残す)を照合した。独立した入口として外部 HTTP の認証・catalog・dispatch、投稿 receipt と actor 継承、migration と通常 Web 編集、Rust API 型・生成型・Web 表示、Unicode decoder・SQL・handler 回帰、隔離 JWT bridge と Chromium spec を確認した。対象 Issue revision と本文 revision、追記と要求変更、来歴と監査証明を分けて正本へ反映した。[投稿・読取の根拠](../crates/server/src/remote_mcp/issue_comments.rs#L85-L279)・[UI の根拠](../packages/ui/src/components/IssueCommentsSection.tsx#L319-L365)・[検証入口](concepts/issue-collaboration.md#変更時の検証入口)

同じ範囲で、[自動マージ検証設定の歴史的更新](concepts/formal-integration.md#検証方針は受付時に固定する)に依存準備 `pnpm install --frozen-lockfile` が先頭追加されたことを確認した。[設定](../.lvk/integration-validation.json)

凍結入力には、初回の型生成・Rust/browser 検証の保留、その後の開発検証成功、最後の自動マージ の install / frontend build / check / lint / backend:test 成功が順に記録されている。これらは供給された過去の実行記録であり、今回の文書保守による再実行結果ではない。今回行ったのは source・テスト内容・文書差分の静的照合で、実 Auth0/ChatGPT、配布サーバー、実DB移行は確認していない。無関係な過去の Wiki 内容全体を再検証したとは扱わない。


### 今回の設計メモ Sync の照合範囲

凍結入力全9チャンクと統合済み source `b7371bd196c9113a16e1efc8116bcf480f588d09` を対象に、[設計メモ](concepts/design-notes.md)を照合した。独立した入口として Project 表示と編集state、RESTとactor、stdio/HTTP MCPのcatalog・scope・receipt、DBの版管理・有界読取、保持migrationとProject削除、DB/server/隔離ブラウザの回帰定義を確認した。共有routerの変更で影響した既存Claimsも再照合した。無関係な過去のWiki内容全体を再検証したとは扱わない。

途中のProject削除409方針は撤回済みで、最終実装は履歴を保持し既存アクセスを閉じる。保持後の閲覧・持出し・再関連付け・期限は未決である。[現行保持契約](concepts/design-notes.md#project-を削除した後の保持)。メモの記録を実行承認とせず、独立追記と本文への明示統合を区別する。

凍結入力は開発検証の保留・後続成功と、最終自動マージの install / frontend build / check / lint / backend:test 成功を記録している。欠落したsemantic draftのcheckpointから動機は推定していない。これらは供給された過去の実行記録であり、今回の文書保守によるテスト再実行ではない。今回の検証はソース・テスト内容・リンク・文書差分とOpenWiki lifecycleの検証で、実Auth0/ChatGPTや稼働DBへのmigration適用は含まない。[変更時の検証入口](concepts/design-notes.md#変更時の検証入口)

### Repository Wiki MCP 同期の照合範囲

今回の通常 Sync は凍結 manifest と全4チャンク（SHA-256照合済み）、統合済み source `de20c156dc28ead686f5f1322ae787b1abd6ff42` の pending changes を対象とした。UI設定からRESTへの入口、外部MCPのcatalog・scope・adapter、共通maintenance受付とscope解決、operation receipt、Workspace owner結果保存、Git公開と回復、独立した対象拒否・再送・公開判定・HTTP scopeのfixtureを照合した。詳細契約は [OpenWiki保守](operations/openwiki-maintenance.md#mcp-から既存-wiki-を同期する)、権限境界は [外部MCP](integrations/remote-access.md#repository-wiki-の同期を依頼する)、鮮度の意味は [Repository Memory](concepts/repository-memory.md#並行性receipt失敗)に置く。無関係な過去の説明全体を再検証したとは扱わない。

凍結Change Manifestでは開発時の型検査成功と実行試験の保留、自動マージの明示的な検証省略が記録されている。付随するWorkspace Memoryは後続の限定試験成功も報告するが、これは供給された履歴であり、今回の実行結果ではない。今回の検証は統合source・試験定義・リンク・文書差分とOpenWiki lifecycleの照合で、製品試験、実Auth0 / ChatGPT接続、実モデルによる同期は再実行していない。[試験と限界の入口](operations/openwiki-maintenance.md#不確実な結果と検証の境界)

### 保守開始元 Project の照合範囲

今回の Sync は、凍結 manifest・全4チャンクの SHA-256 と source `7297e1148324cb8a2b0c03c29b23d4a74dd4092d` を照合した。Project 設定 UI、MCP の認可・adapter、共通受付、ProjectRepoDefaults を含む関連集合、Issue 作成 transaction、過去補完、repository Workflow の所有形式と各 fixture から、[開始元 Project と保守 Issue](operations/openwiki-maintenance.md#開始元-project-と保守-issue)の契約を更新した。共有ファイルの変更で stale となった既存 Claims も該当部分を確認したが、無関係な過去の Wiki 全体を再検証したものではない。

凍結入力の最初の checkpoint には意味 draft がなく、意図・理由・試験結果は不明である。後続 Manifest の開発試験成功は履歴として扱い、自動マージ の検証省略を合格へ読み替えない。この保守で行ったのは source・試験定義・文書差分・リンク・OpenWiki lifecycle の検証であり、製品試験、実モデルによる製品の同期処理、実DB・外部認証は再実行していない。

### 外部 MCP archive の照合範囲

今回の Sync は凍結 manifest と全1チャンクの SHA-256、統合済み source `505b8f9cac53b26c9609068672fa18a87b7b58a8` を照合した。HTTP 認証・catalog・scope、外部 adapter、UI/stdio の共通 archive、DB 所属・busy 検査、strict cleanup、operation receipt と現在値の読取、署名 JWT を使う HTTP fixture を独立した入口として確認した。詳細は [Workspace の確認と回復](concepts/workspace.md#外部-mcp-の確認と結果の回復)、利用導線は [外部 MCP](integrations/remote-access.md#workspace-を外部-mcp-から-archive-する)に集約した。共有入口に関係する既存 Claims は再確認したが、無関係な過去の説明全体を再検証したものではない。

凍結 Change Manifest は自動マージ の検証を明示的に省略したと記録し、意味的な変更理由や開発テスト結果は供給していない。この保守は実装・試験定義・文書差分の静的照合であり、製品テストや実 Auth0/ChatGPT 接続の再実行ではない。fixture の script liveness と実環境の検証範囲も区別する。

### 独立アプリ経路の照合範囲

今回の Sync は凍結 manifest と全3チャンクの SHA-256、統合済み source `fc29341d2862a46d4a78273ea0662a3eec19e899` を照合した。Compose 両サービスの環境配線、JS の起動・nginx 生成・TLS reload、Python の ACME 注文、legacy preview の UI URL と backend 認可、共通 parser fixture と native nginx の転送試験を独立した入口として確認した。詳細契約は [サーバー運用の独立経路](operations/server-container.md#独立アプリの-csv-ルート)、変更時の検証は [配布 kit の CSV 回帰](operations/development.md#独立アプリの-csv-ルートを変更するとき)に集約した。無関係な過去の説明全体を再検証したものではない。

CSV 経路は LVK コンテナー内の HTTP アプリへ HTTPS を転送し、LVK login/handoff による保護を追加しない。既存の wildcard preview とは認証責任が異なるため、アプリ自身の認証と cookie の運用を確認する。[転送と資格情報](../deploy/server/server.mjs#L248-L276)

凍結 Change Manifest は開発時の局所・native 試験成功と自動マージ の検証省略を区別している。移行したCSV経路の開発履歴も実 ACME・DNS・VPS・アプリ・配布 image を未確認としている。今回の検証は統合 source・試験定義・文書差分・リンクと OpenWiki lifecycle の静的照合であり、製品試験や証明書発行の再実行ではない。

### カンバン・差分・競合 Session の照合範囲

今回の Sync は frozen manifest と全11チャンクの SHA-256、統合 source `c8ccba168fdc2c06dae69f0b92a1cb70f1d80267` を照合した。カードの呼出側・部品・summary hook・API・Git 集計・独立した service/Git/browser fixture、自動マージ の admission / immutable request / transaction / native session projection の回帰試験を別の入口として調べた。配布側の変更は Dockerfile、運用文書と image smoke 定義で確認した。説明の正本は上記の概念・操作ページに置き、共有参照で stale となったページは必要箇所だけを再確認した。

供給された最終 自動マージ event は install・frontend build・check・lint・backend:test の成功を記録する。この保守自体は静的な source・試験定義・リンク・OpenWiki lifecycle の検証であり、製品試験、実 Codex / Claude、browser からの実 Resume、Docker image の実行、外部 preview 認証を再実行していない。意味 draft が欠けた launcher checkpoint の意図は不明のままとし、統合ツリーにない machine 固有 launcher や Workspace Memory の運用状態を製品契約へ昇格させていない。無関係な歴史的説明全体を再検証したとはしない。

### Issue分類・archive・検索と会話表示のSync

今回の通常Syncは凍結manifestと全23チャンクのSHA-256を照合し、統合source `c8d79127e02da3eee425b4963febcbfb28bf0ea6` のpending changesを対象にした。入口はProject作成APIと採番/移行、Tag共通CRUDとowner補完、Issue archiveの確認/DB trigger/Workspace cleanup、両MCPのscope/filter/paging、Todo/設計メモのUIとDB検索、provider adapterとcanonical/live投影、composer/送信store/仮想リスト、独立したDB/HTTP/stdio/browser試験定義である。無関係な歴史的Wiki内容全体を再検証したとは扱わない。

- Project key・priority・system Tagは [仕事の識別と分類](concepts/project-and-issue.md#仕事の識別と分類)、Statusと別の属性・全対象確認・不確実な削除は [Issue archive](concepts/project-and-issue.md#issue-のアーカイブ)を読む。
- 両MCPの検索はfilter解釈を共有し応答/pagingは別である。[MCPの検索と操作](integrations/remote-access.md#issueの分類検索archiveを操作する)、[Todo検索](concepts/project-todos.md#一覧を検索する)、[設計メモ検索](concepts/design-notes.md#一覧検索と下書きの関係)が説明先となる。
- literal入力・送信直後のpendingとACK/event照合は [Sessionの入力](concepts/session-and-agent-run.md#送信中の編集と-markdown-正規化)、公開summary/plan・操作結果・追従意図と読書anchorは [Webの会話同期](architecture/web-and-sync.md#履歴の閲覧位置と末尾追従)を読む。

凍結入力には開発中の保留・失敗・修復と、最終自動マージのinstall / frontend build / check / lint / backend:test成功が記録される。これらは供給された実行履歴であり、この文書保守の再実行結果ではない。未統合Workspace Memoryの運用指示やmachine固有preview launcherは製品契約へ昇格させていない。今回の検証はソース・試験定義・差分・リンクとOpenWiki lifecycleであり、製品suite、実Auth0/ChatGPT・Codex/Claude接続、稼働DB移行、preview切替は再実行していない。

### 生成環境の成功後 archive と保存進捗の Sync

今回の通常 Sync は凍結 manifest と全10チャンクの SHA-256、統合済み source `c4d2f6b730900dfc0ba05f213fa795a135ce33db` の pending changes を対象に照合した。生成元登録の transaction / migration trigger、owner 解放の caller と cleanup predicate、archive の最終 write transaction・共通削除・server/SQL fixture、自動マージ の成功と semantic outbox 回復を独立した入口から確認した。説明の正本は [Workspace の生成archive契約](concepts/workspace.md#生成された専用-workspace-の自動-archive)に置き、[保守の解放](operations/openwiki-maintenance.md#成功後の保守環境の解放)と [統合後の作業領域](concepts/formal-integration.md#成功後の専用作業領域)、Issue・Memory・横断workflowの関係を整合した。

保存進捗は外部 MCP のcatalog・read scope、所属とcursorのsnapshot、canonical末尾選択、privacy投影・全文形状検査、pagingとHTTP再接続の回帰定義を照合した。[最新窓とcommentaryの契約](integrations/remote-access.md#保存済み-codex-進捗の読取)を参照する。既存共有ファイルでstale/unresolvedとなったClaimsは該当契約を再確認したが、無関係な過去のWiki全体を再検証したものではない。取り込まれたProject-key互換処理は前回までの説明と最終source差分を確認し、変更のない契約を再生成していない。

凍結Change Manifestには初期の重検証保留、fixture修復後の限定試験・lint/backend検証成功、最終凍結候補のinstall / frontend build / check / lint / backend:test成功が記録されている。これらは供給された履歴であり今回の試験実行結果ではない。今回の検証はsource・試験定義・文書差分・リンクとOpenWiki lifecycleの照合で、製品suite、実Auth0/ChatGPT、実モデルのBootstrap/Syncからprocess/cwd参照までのE2E、稼働DB移行、実ユーザーWorkspaceの削除は再実行していない。開発文書の古い「未実行」記録を現在の失敗とせず、Workspace Memoryの未統合運用状態も製品契約へ昇格させていない。

### 起動回復・provider参照・MCP操作のSync

今回の通常Syncは凍結manifestと全17チャンクのSHA-256を照合し、統合source `b9fd2194154bfff52fe362eb9253037521f91963` のpending changesを対象とした。独立入口として、server起動/registryとmigration、create/Starting/Cancelとhost予約、canonical reducer/summaryとUI、本文/コメント/終端のtransaction、DnD helperとDB競合、stdio/HTTPのcatalog・認可・receipt、公式CLIの受動transport、resume前後履歴証拠、raw stream/headerと認証middleware、focused fixture定義を照合した。無関係な過去のWiki全体を再検証したとは扱わない。

詳細契約は上の調査導線の正本に置き、横断説明は同じ契約へリンクする。新たなschedulerやproviderを導入せず、Starting回復、DevServer script所有、MCP一回承認、quota snapshotと実行token累積を区別した。変更時の検証入口と残る範囲は [開発・検証](operations/development.md#起動回復受動参照mcp-操作を変更するとき)を参照する。[共有起動](../crates/local-deployment/src/lib.rs#L145-L163)・[MCP登録](../crates/mcp/src/task_server/tools/mod.rs#L56-L91)・[外部dispatch](../crates/server/src/remote_mcp/mod.rs#L187-L241)

供給された開発/候補合成記録には局所検証の成功と修復履歴があるが、最終自動マージは明示的に検証省略だった。実agentの承認待ち到達と取消は人の承認後の実tool成功ではなく、offline UI fixtureも実API/認証ではない。この保守はソース・試験定義・差分・リンクとOpenWiki lifecycleを検証し、製品suite、実Auth0/ChatGPT、実モデル、稼働DB移行、配布・restartを再実行していない。[候補の検証限界](operations/development.md#起動回復受動参照mcp-操作を変更するとき)

### 自動マージ名称 Sync の照合範囲

凍結入力の manifest と全4チャンクの SHA-256、統合 source `e4a2dc3ca101233becb63b355f7020f5396da415` を照合した。UI/i18n、MCP の catalog・受付・読取・判断、host の merge/validation/publication、DB の予約・Done receipt と回帰試験、Wiki 開始 guard を独立した入口から確認した。名称と互換性の詳細は [自動マージの正本](concepts/formal-integration.md#名称と互換境界)、実際の順序は [並列成果を公開する手順](workflows/task-to-integration.md#並列変更を試してから正式統合する)に置く。

変更は利用者向け名称と DB 試験期待値への追随で、既存の機械識別子・承認・保存形式を維持する。最終候補の検証成功は供給された Manifest の記録であり、この Wiki 保守はソース・試験定義・差分・リンクの静的照合を行った。製品 suite、実モデル・Auth0 / ChatGPT、稼働 DB・通知配送・配布を再実行しておらず、無関係な歴史的内容全体を再検証したとは扱わない。

### REVIEW・MCP素材・Wiki継続の統合

今回の候補は固定baseから採用3sourceを統合し、コード検証後の同じbranchでWikiを更新した。調査先は次の正本に分ける。

- レビューmodeの保存・通常会話・権限と自動source採用抑制は [REVIEW](integrations/agent-providers.md#review-の実行境界)。
- 素材の実bytes入力、Project保存、Workspace共有の検証済み配置、receipt回収は [MCP素材搬送](integrations/remote-access.md#mcpで素材ファイルを渡す)。Library IDやローカルパスを搬送成功と扱わない。
- RunをまたぐOpenWikiの正規再開は [同じ会話の監査証明](concepts/formal-integration.md#同じ会話でwiki更新を再開する)。standaloneのphase閉鎖は維持する。

コード `7a740a94` でfrontend build/check/lint/backend:test/types/SQLxの同一batchが成功し、その後のtest-only差分は `b803a1ed` の267/272隔離E2E・Chromium・Vitest15件・fmt・server all-targets Clippyで検証した。旧failedを成功へ書換えず、最終SHAで変更のないsuiteの再実行省略はagent判断として記録した。このWikiの後続commitにコード試験を付け替えない。

実ChatGPT/Library fileParams・署名URL期限・host/proxy制限、実Codexと実外部OpenWikiのRun跨ぎ再開は公開後の適合ゲートであり、fixture成功で完成とは扱わない。保存quota/TTLは追加していない。検証の入口と限界は [開発・検証](operations/development.md#review添付搬送wiki再開を変更するとき)を参照する。

### TODO番号とOpenWiki 0.7.2の統合

LOCAL-281/282の固定sourceを同じ候補へ競合なく統合した。TODOの不変番号・非再利用counter・両MCPのProject内参照は [Project Todo](concepts/project-todos.md#project-ごとの不変番号)、maintenance writerの読取4tool・linked参照・operation監査分離は [OpenWiki保守](operations/openwiki-maintenance.md#072-の読取横断参照と検証)を読む。通常agentへの新たな検索公開や並列writerは導入していない。

コード候補 `75e7cb71b5e82df998bfb440180fe35d022913c1` でlocked install、frontend build、check、lint、backend:test、生成型/SQLx整合が管理commandとして成功した。Wiki更新はその後の別commitであり、後続Wiki SHAへコード試験を付け替えない。281のChange Manifest欠落と取得できなかった完了run本文は成功証拠に数えず、282の最終deltaは試験/検証記録だけだったことを照合した。fixtureと実Auth0/ChatGPT、稼働DB移行、配布image、共有runtime切替は区別する。今回のWiki lifecycle自体はこの環境の登録済みMCPを利用し、共有OpenWikiインストールの更新は行っていない。

### 284/285/286/288統合の調査導線

- TODO入力の候補は [未完了項目からのカテゴリー候補](concepts/project-todos.md#未完了項目からのカテゴリー候補)を読む。完了表示・検索と候補導出を混ぜず、保存値と自由入力を維持する。
- 通知設定は [イベント選択と二つの保存範囲](integrations/web-push.md#イベント選択と二つの保存範囲)を読む。Web Pushの登録別即時保存とOS通知のインスタンス共通config保存を区別する。
- 設定のアカウント画面は [本人情報の取得境界](concepts/server-access.md#設定内のアカウントと本人情報)を確認する。新たな役割や本人情報取得scopeを追加した変更ではない。
- MCP失敗時は [共通診断と安全な復旧](integrations/remote-access.md#全mcp共通の失敗結果不明復旧契約)を読む。分類だけから副作用を否定せず、元request/receiptと対象状態を確認する。

コード候補は `e1a08b8e167bc7f4180662c072228ba71af3070b`。固定baseから検証済み候補をそのまま採用し、追加競合・コード修正はない。既存の型/lint・生成型・browser・MCP・認証・通知・DB回帰の成功記録を参照し、ユーザー指定で重複検証を省略した。全backend suite・実Auth0/Push・live環境は未検証であり、このWiki保守での製品試験成功とは呼ばない。Wikiのみの後続commitとコードSHAを区別する。LOCAL-213の未commit文言を入力にせず、後のi18n統合時には追加Wiki commitとの差分を再確認する。

### LOCAL-213のi18n統合

英日UIと他5言語の既存辞書・英語fallback、ブラウザー言語と保存configの関係は [言語解決と診断表示](architecture/web-and-sync.md#言語解決と診断表示)を読む。通知は [登録別Pushと共有OS通知の言語保存](integrations/web-push.md#通知言語の二つの保存範囲)を区別する。診断詳細は翻訳説明と原文の併記であり、結果不明を失敗に変えたり再送する仕様ではない。

常設checkerと少数locale固有browser、および終了した移行QAの保証範囲は [i18nの常設検査と移行検証](operations/development.md#i18nの常設検査と移行検証)に置く。コード候補 `1957d8edfa98ebe512ea4a960a38a59e936507fc` は固定main base `5686d655` へsource `73c4504c` を競合なく統合したもの。管理commandでinstall/build・frontend型/lint・i18n検査・残存browser44件の一括成功、通知/Push対象Rust・生成型同期・unit94件を確認した。全Rust workspace/qa-mode Clippy再実行はagent判断で省略し、実Auth0/ChatGPT/Push/OS配送は未確認である。Wikiはコード検証後の別commitで、mainへの実Git適用は公開要求後のhost receiptで確認する。

## 共通ユーティリティと実行承認を調べる

Kanban / Workspace の右サイドバーは共通環境 panel とファイル・単一端末・条件付き Wiki を持つ。Kanban は登録 main repo、Workspace は作業 worktree を対象にし、本文を既存分割ペインで読む。端末項目の明示折畳みは所有 process を終了するが、sidebar 非表示は停止意思ではない。exit/検出した通信断では無言で折畳み、自動再生成しない。[対象・寿命・検出限界](operations/workspace-inspection.md#kanban-の-main-と-workspace-の対象を区別する)

実行承認の対象・引数・一回限り scope は承認 UI 内で確認する。外部 MCP の読取は `get_question(kind=approval)`、明示判断は read/write の外部認証済み member に限定した `decide_execution_approval`、読戻しは `get_execution_approval` を使う。内部 Agent の自己承認、仕様回答、恒久許可、自動マージ公開へ転用しない。[認可・receipt・判断確定の境界](integrations/remote-access.md#実行承認を読み単発の判断を送る)

### LOCAL-300 の製品UI・Pet統合

今回の保守はコード検証SHA `7bcd5db39419d50ad265592e288264c63906e1da` の製品候補を対象にする。[Pet仕様と安全境界](concepts/pets.md)、[ホスト表示名と設定導線](architecture/web-and-sync.md#ホスト表示名と設定導線)、[利用枠の共通表示](architecture/web-and-sync.md#codex--claude-のアカウント利用枠)、[ファイルとterminalの対象](operations/workspace-inspection.md#terminal-と-editor)を入口とする。docs/は復活させず、Rabbitの製品LICENSE/ATTRIBUTIONを保持する。Agentation試験はpreview2側の開発用途に残し、製品候補の導入・依存・lockfile・専用テストを除外した。元Workspace、DevServer、DB、認証と注釈データは変更していない。

候補で重点unit/browser、型・i18n・対象lint、production build、Rustの表示名/旧scratch/Pet添付保持とClippyを確認した。全workspace反復は担当判断で省略し、公開認証後画面・実Auth0/provider・ユーザー実物v2画像は未検証である。source開発時検証、今回のコード検証、Wiki完了とhostによるmain反映をそれぞれ区別する。今回のClaim照合は変更された参照を持つ関連ページに限定し、無関係なWiki全体の再検証ではない。

### LOCAL-316/319後のPet動作・取込更新

固定base `e9712fe96fbdc65dfad7c6b9f327bcfc76d95ba0` へLOCAL-300 source `ebc230edc6ec7d13c1704874237f3d7d8cc5a6da` を競合なく統合した。コード検証SHAは `4e84a6fe1770ccc5aa4fe1086f828fafb2849c68`。既存デザイン・ホスト表示名・利用枠・Pet v1/v2を保ち、[Petの活動状態と操作](concepts/pets.md#sidebarの活動状態と操作)、fflate ZIP、任意neutralセルと取込feedbackを更新した。LOCAL-316/319の既存HTTP transportと空表示を維持し、新たな通信監視は追加しない。

隔離候補だけでAgentationのBootstrap呼出し・wrapper・package/lock依存・専用testsを削除し、preview起動補助を固定baseへ戻した。元LOCAL-300 Workspace/branch、preview2・Agentation注釈・サーバー・実データ・共有設定は変更していない。コード候補ではlocked offline install、Pet単体/生成browser/sidebar統合、frontend三package型、consumer/変更production source lint、i18n、production build、成果物とsource/lockのAgentation不在が成功した。無関係の全backend検査はagent判断で省略し、実認証後UI・実OS window移動・ユーザー原物ZIPのnative decode/保存は未検証である。Wikiはコード検証後の別commit、main公開成立はRun終了後のhost receiptで確認する。


## LOCAL-332: Run成果とWiki運用状態の整理

開発中の文脈はproviderのSessionが扱い、Workspace Memory・no-change専用検証記録・commit reminderは撤去された。通常成功coding Runのsource自動commitとファイルManifestはWiki無効でも有効。`manifest_in_commit_body` は既定OFFの選択肢で、ONは通常LVK source commitへ意図と判断理由を人間可読に併記する。Gitで共有される本文からManifestを復元する二重管理は行わない。[責務と設定の詳細](concepts/repository-memory.md)・[通常Runの最終報告](integrations/remote-access.md#専用検証mcpの廃止と通常run報告)

可変なWiki state・receipt・publication・source integration対応はinstance DBへ集約し、Manifest・固定sync-inputs・source-publication・setup原本journalはファイルとして保持する。一度限りの移行CLI・importer・旧JSON decoder・互換guardとrunbookはLOCAL-336で撤去済みであり、現行版で再importしない。[現行のDB正本と撤去・保持境界](concepts/repository-memory.md#運用状態のdb集約と一度限りの開発移行)

今回のコード候補 `41c1bae500bcec05bffe4002cf61b5413b51a524` は最終source `289373ec23493c0a21247a87da63c04bd4dbea3d` と同一treeで、競合・追加修復なし。tree一致・差分空白・clean確認を管理実行し、source各段階の既存SHA対応検証を再利用して包括suiteの再実行を省略した。省略を今回の試験合格と呼ばない。Wiki保守はsource・試験定義・Manifestの静的照合とlifecycle処理であり、liveDB移行・旧共有ファイル削除・停止/再起動・push・main build/bundle・実provider/外部認証を検証していない。過去のWorkspace Memory記述は過去の供給履歴であり、現行の生成/参照経路ではない。


### LOCAL-341: Wikiツリーの可変高

右サイドバーのWikiが利用可能な残り高さを使う仕組みは [高さ配分](architecture/web-and-sync.md#wikiツリーの高さ配分)、操作は [Wiki高さ](operations/workspace-inspection.md#右サイドバーのwiki高さ)を参照する。本文幅や検索・選択の状態管理は今回の変更範囲外である。

コード候補 `a3fab8acc31037cef9d3739efcb6c254217c2cd7` は固定base `c04a57308a8a97ee357359c35201fd7b7145854c` からsourceを競合なく採用した。source Run終了のChange Manifest（event `3961223f-d7dd-4b68-bb8a-85d6a310b6d0`）から目的・設計判断・検証要約を読み、ユーザー指定で型・対象lint・整形・Chromium簡易レイアウト24条件の既存証拠を再利用した。管理commandで候補とbaseの同じWikiテストを比較し、双方18成功・同じQueryClientProvider不足2失敗を確認した。比較判定は成功だが全テスト成功ではない。実アプリの認証後UIは未確認である。Wikiはコード検証後の別commitであり、main公開はRun終了後のhost適用receiptで別に確認する。無関係な過去のWiki全体を再検証したとは扱わず、既存の要確認Claimだけを照合した。


### LOCAL-336: 移行完了後の一時コード撤去

固定base `68160407e0de38d9363796ee33b4c9ea7e189caf` からLOCAL-336 source `ed6ac2deec100263a49aec3883e6dff85dadc1bb` を競合なく採用し、コード検証SHAも同じである。338/339の未マージ差分は含めない。一度限りのCLI・importer・decoder・未移行guardと手順書を撤去し、通常DB model・forward migration・Manifest・固定sync-inputs・Git復旧情報を保持する。[保存先と境界](concepts/repository-memory.md#運用状態のdb集約と一度限りの開発移行)・[保守運用](operations/openwiki-maintenance.md#local-332以降の保存先と移行境界)

候補SHAで差分空白・固定baseの祖先関係・撤去参照の不在を管理commandで確認した。実装ManifestのDB 2件・utils 14件・server全targetコンパイル・fmt成功を参照し、同一sourceで追加修復がないため同じsuiteと包括検証の再実行をagent判断で省略した。これは今回の試験実行passではない。Wikiのみを後続commitにし、target publicationはRun終了後のhost receiptで別に確認する。devのビルド・再起動・切替、実DB・backup・release・旧共有データの変更や削除は実施していない。336/332の旧データ残件の完了をコード公開から推定しない。


### RepositoryのWiki設定・削除・全体再構築（LOCAL-344）

執筆指示と探索除外は[設定ファイルの保存](operations/openwiki-maintenance.md#執筆指示と探索除外を保存する)で扱う。保存は以後の生成入力を変えるだけで既存ページを更新しない。消す場合は[完全入力付き削除](operations/openwiki-maintenance.md#danger-zoneで生成wikiを削除する)、新指示で構成から見直す場合は[固定旧Wikiを参考にする再構築](operations/openwiki-maintenance.md#danger-zoneで既存wikiを参考に作り直す)を選ぶ。削除後は明示Initializeまで通常Sync/自動マージから再生成しない。再構築は正規公開成功まで現行Wikiを残す。[表示元と未生成表示](operations/workspace-inspection.md#削除後のcanonical-wiki表示)

コード候補は `cb0c335a9d5c8c78fb4cc813c9b6f9ba9543506b`。344のみを固定baseへ競合なく統合し、346/348の修復を保持した。管理検証は既存結合試験の製品tree再現・非Wiki差分ゼロ・祖先・空白・cleanを確認した。重複suiteはユーザー指定で省略し、既存試験を今回SHAのpassへ付け替えない。[検証範囲と未確認事項](operations/openwiki-maintenance.md#local-344の検証範囲)を参照する。Wikiは別commitであり、mainへの反映成立はRun終了後のhost結果と分ける。

## Projectプリセット・作業場所を変更するとき

LOCAL-349でProjectは新規WorkspaceのRepoプリセット、Workspaceは実Repo/基準branchの所有者になった。新規作成はGit/直接folderを分け、Gitは単一/複数Repoとも共通root配下のworktreeを使う。既存選択は孤立Workspaceだけで、移す際は明示解除後に再リンクする。プリセット変更/リンクは既存構成・成果物・Session履歴を再作成しない。

- 作成・保存配置・未知pathの保全・DirectFolderのGit支援除外は[Workspace](concepts/workspace.md)。
- プリセットと既存関連の区別・リンク競合は[ProjectとIssue](concepts/project-and-issue.md#projectのrepositoryプリセット)。
- 親子AGENTSの適用範囲、Codex extraRoots、Claude add-dirと同名拒否は[provider別の経路](integrations/agent-providers.md#共通rootのagentsとrepo-skills)。
- 旧保存cwdを保った継続と検証限界は[Session](concepts/session-and-agent-run.md#共通rootと旧sessionの継続)、新添付・chat file link・editorは[Workspace inspection](operations/workspace-inspection.md#local-349の保存配置と直接フォルダ)。

DirectFolderを選んでも実行権限/MCP許可rootを拡張しない。metadata発見・本文到達・自動選択・手順実行・指示遵守は別々に確認する。今回の統合では最終sourceとコード候補treeの一致を管理commandで検証し、アプリ試験/buildの反復はSHA付き既存証拠に基づき省略した。Wiki保守で実provider行列や旧binary移行を再検証したとは扱わない。


## LOCAL-361の指示配信と本文整理

Issue固有の要求は通常本文で閲覧・編集し、旧Card Contextの専用操作やhidden分離は行わない。保存Issue由来の完全一致入力だけに既知lvk共有文5版のhash除去を適用し、編集・未知・不完全・引用は保持する。[保存本文](concepts/card-context-and-llm-wiki.md)

通常起動は現在identity/draft_pathと短い契約を届け、Manifest・parallel・資源/Runnerの詳細はMCPなしでも読める実ガイドファイルへ置く。任意get_context guideも同じ定義を返す。Codex継続は履歴保持の推測をせず現在developer指示を明示injectし、Claudeは既存user suffixを使う。[配信境界](integrations/agent-providers.md#codex-resume-の固定指示と現在-identity)・[Manifest](concepts/repository-memory.md#現在identityとオンデマンドのmanifest形式)

コード候補は `da8f775878924b833971888e7c8ee28586cdf84b`。Gとtree一致の無競合merge、実frontend/最適化3binary build、sourceアクセス拒否下の隔離bundle試験を管理記録で確認した。重複検証は既存SHAの証拠と差分riskから理由付き省略であり、新しいpassではない。実Auth0/browser/modelとdev切替・再起動は今回未実施。Wikiは別commitとし、mainへの正式反映はRun終了後のhost結果を確認する。[検証範囲](operations/development.md#local-361の統合検証とbundle)


## LOCAL-365のMCP契約移行

stdio/remoteのIssue mutation・開始継続・receipt保証、priority:null保持、旧Workspaceファイル4toolからquery_repositoryへの移行は[接続契約](integrations/remote-access.md#stdioとremoteのmutation保証local-365)と[範囲read](integrations/remote-access.md#同じ本文を確認して周辺行を読む)を参照する。orchestrator modeは案内付き明示エラーで、globalへの設定変更では旧Workspace制限がなくなる。[stdio起動](integrations/agent-providers.md#外部-mcp-client--lvk)

コード候補は `bacddc9c01995713ef063df85de18835815c05d8`。固定baseの直接子を競合なく採用し、同一SHAの公開Memoryにある代表試験とClippy証拠を再利用した。ユーザー指定で重複suiteを省略した記録は新たな試験passではない。実Auth0/provider/native・全組合せE2Eと稼働版反映は未確認で、ignored添付fixtureの追従課題は[検証入口](operations/development.md)に残す。Wikiは別commit、正式公開はRun終了後のhost結果で確認する。サーバーの停止・切替・再起動はこの担当では行わない。


## LOCAL-373の独立したWiki領域管理

マーカーのない子indexを含むopenwiki/の生成保守契約と、INSTRUCTIONS/.openwikiignoreの明示保護は[削除・再構築の運用](operations/openwiki-maintenance.md#danger-zoneで生成wikiを削除する)を読む。[独立成果の検証と残課題](operations/openwiki-maintenance.md#local-373の独立領域管理と残課題)ではコードSHA 45f0e390574614129023baba11e3b01c396b205aの107件既存成功・重複省略と、実OpenWiki依存4件および標準i18n/Prettier未成立を区別する。372接続・専用進行管理撤去・独立再Review・品質警告伝播は未実装であり、373全体の完了を意味しない。Wikiはコード検証後の別commit、main正式公開はRun終了後のhost結果で確認する。


## LOCAL-364/368：共通自動マージと会話・Markdown表示

WorkspaceのGit操作、Kanban、MCPは[同じ自動マージ](concepts/formal-integration.md#共通入口採用資料異常時の診断local-364)へ進む。旧直接マージ専用経路を撤去し、PR/rebase/共用競合解消と歴史的読取を維持する。複数sourceは一候補で一回公開し、sourceのHEAD差・実行中Run・HEAD観測不能による自動Done保留を公開成功から分ける。[開始と停止後診断](operations/workspace-inspection.md#workspaceから自動マージを依頼診断するlocal-364)で期待branch/pathと実Git状態を確認する。

[初回依頼・思考見出し・引用](architecture/web-and-sync.md#local-364の初回依頼表示とlocal-368の追加調整)は、保存済み初回原文のidentity、非操作のインラインMarkdown、正規DECISION引用と太字の通常意図ラベル、Previewのソフト改行とLexicalの保持を説明する。[検証fixtureと限界](../tests/markdown/README.md)を使い、認証済み実アプリの確認とは区別する。

固定base `f042245ca8c8855a9b98ba16d378a0302bc6f219` にsource `90f4fcbc5432b834da76e1466d2e4aa54159d6c8` と `cd3edef7ba99de7cc9bf3f6f3262353315a81c0c` を採用し、コード検証SHAは `f9bd24b17bb2876a44f3ae6e56e457b058273514`。DisplayConversationEntryのimport競合は初回依頼概要と思考Markdownの両描画を保持して解消した。373の製品差分と既存Wikiを保持し、他Workspace/共有dev/preview2や外部pluginを操作していない。

候補上のfrontend型・対象lint・会話/Markdown回帰・明暗狭幅fixture・build、DB/server/executor対象試験と影響crate Clippy、隔離3シナリオ（複数source、HEAD観測不能、Wiki receipt回復）は管理commandで成功した。全suite・型/SQLx再生成は既存SHAのManifest証拠と差分に基づくagent判断の省略であり、新たなpassではない。実provider/認証済UIは未確認で、旧devの今回worker実行を364新入力のprovider確認に数えない。Wikiはコード検証後の別commitであり、正式公開の成立はRun終了後のhost結果で確認する。


## LOCAL-376：native質問の配送とdelivery保持

質問UI/MCPからhost waiterを通って元native requestへ返す経路は[Sessionの質問契約](concepts/session-and-agent-run.md#plan一般質問との境界)、元deliveryと旧mapper互換は[runtime投影](architecture/agent-runtime.md#local-376のnative質問配送とdelivery保持)、複数質問のIDと回答形式は[MCP](integrations/remote-access.md#local-376のnative質問回答)、表示は[質問UI](architecture/web-and-sync.md#local-376のnative質問ui)を読む。372のasync発行・継続・元turn終了・失効を維持し、本文/DECISIONから状態を推論しない。

固定base `43098c170a900fb7d8a5786ba3fc1a5a10d10a31`へ376 source `e7023f69df107099636fc94f37ee4b9a482dc475`を競合なく採用した。コード検証SHAは `2260c073ce179dbdb2bac0d2ebf75ef33f819d19`。364/368/373を保持し、372のworktreeと稼働devを変更していない。候補でfrontend型・会話回帰・質問Chromium・production build、Rustのmapper/配送/取消/終了/stale回帰・生成型を管理実行で成功確認した。[検証範囲](operations/development.md#local-376のnative質問回帰と統合検証)に、sourceの既存成功証拠を再利用したagent判断の包括suite省略と、未実施の実モデル・実認証一体E2Eを分けて記す。

Wikiはコード検証後の別commitで、正式main反映は公開要求後にhostが確認する。稼働dev再起動・container再作成・security変更・適用は今夜の対象外であり、公開後の追加buildは後続指示で扱う。


## LOCAL-372：固定presetと共通実行

新規開始はIssueの共通Workspace dialogから単一Agentまたは「実装→独立レビュー」「多角調査→統合」「棚卸し→削減案」を選ぶ。各presetは報告を返して停止し、推奨後の採用・修正・削除・Git merge/公開は別指示で判断する。[使い分けと固定parameter](concepts/workflow-attempt.md#現行の3-presetと共通開始)・[MCPの同じサービス](integrations/remote-access.md#固定workflow-presetの共通toollocal-372)

[実装commit後の引渡し・全成功合流・watcher](architecture/workflow-runtime.md#固定presetの成果成立と一度だけの合流)と、[通常REVIEWと異なる事前ソース保護](integrations/agent-providers.md#固定presetの事前ソース保護local-372)を確認する。旧任意graph/editorとArenaの新規入口は終了し、既存定義・Run・retryと[Arena互換履歴](concepts/arena.md)は保持する。373固有品質や専用処理撤去を完成扱いにはしない。

固定base74317e44へsource6a8c43a6を無競合採用したコード検証SHAは `23ac54d4c270a71274e8c6c631180877fd6d2eb7`。[候補上の管理検証・省略理由・未実証事項](operations/development.md#local-372の統合検証と未実証の保護成功経路)に、build/check、Workflow/provider制限/runtime回帰、Clippy、生成型/SQLx、配布検査とmock UI成功を記録する。実provider完走、source書込拒否＋tmp/cache成功の同時成立、認証付き実UI/MCP、image/runtimeは未検証であり、実装完成・実環境検証完了は認定しない。

Wikiはコード検証後の正式prepareによる原本journal保存後に同じ候補branchで更新する。Wiki完了とmain公開は別で、正式公開要求の保存後に担当Runを終了し、hostがwriter収束・既知setup差分の限定復元・clean検査とGit反映を確認する。dev/server停止・再起動・切替、デプロイ、image構築、container再作成、権限/共有データ変更は今回行っていない。


## LOCAL-377：Issue本文の手動版と現在仕様

Issueの本文は自由な名前の編集可能な版として保持し、閲覧タブと現在仕様を分ける。通常保存で版は増えず、コピー/空作成と現在指定は明示操作で行う。[正本・移行・競合](concepts/project-and-issue.md#手動の本文版と現在仕様local-377)・[版別draftと手動保存](architecture/web-and-sync.md#手動版タブと現在仕様local-377)・[stdio/remoteの5toolと旧clientのrevision要件](integrations/remote-access.md#手動本文版のmcp操作local-377)

現在仕様のdescription互換コピーは既存読取・将来の起動・自動マージsnapshotへ流れる。既送信promptと旧レビュー参照を後日の版へ読み替えない。[コメント・レビューの境界](concepts/issue-collaboration.md#手動版本文とレビュー対象の境界local-377)

372を含む固定base `5d3e625d9731923e948f8054c1ea21db4c13003d` へ377だけを無競合統合し、コード検証SHAは `00817c5e2c8edb1d5a26f9adcd86fdfb2ebf3fd6`。候補上でbuild・check・lint・i18n・生成型/SQLx・版browser/Vitest・全backend再実行が成功した。初回backendの既存終了状態テスト失敗、単独/全suite再実行成功と残る再発可能性、実認証/provider・稼働DB・別環境未実施は[検証記録](operations/development.md#local-377の手動版回帰と統合検証)へ分けて記す。375は次回対象である。Wikiは別commit、正式main反映はRun終了後にhostが確認する。push・tag・認証変更・デプロイ・稼働環境操作は行わない。


## LOCAL-375：稼働版を根拠にする一般ヘルプ

UI/MCPの共通会話、質問ごとの参照版、失敗・取消・削除、REVIEWの目的と権限境界は[対話ヘルプ](concepts/interactive-help.md)を参照する。Project不要の公開source読取と既存Project認可、非冪等askの再送境界は[MCP契約](integrations/remote-access.md#稼働版sourceと対話ヘルプlocal-375)、公開SHAの全祖先とUID10001・volume外配置は[同梱配布](operations/server-container.md#稼働版ヘルプ用の公開source同梱local-375)に記す。

固定base `1b511168` に375 source `78fb649b` を採用し、en/ja末尾の競合で377本文版と375ヘルプを両方保持した。広域検証を実行したSHAは `69f9f382c74316d2c7f872826d00ab11f348450d`。画像fixtureの撮影だけを補正した最終コード検証SHAは `e5cddb5128e878ce0f0d9bd37bb0c150ea88268a` で、そのSHAでは差分限定とmock Chromiumを再実行した。重複広域suiteのagent省略と別環境試験を準備に留めるユーザー境界を区別して記録した。[検証結果と未実施](operations/development.md#local-375の対話ヘルプ統合検証)

完成image・異なるimageへの保持volume更新、実OAuth/MCP、実browser/provider結合、回答生成中取消等は未検証である。[別環境手順](concepts/interactive-help.md#別環境で残る受入検証)を準備し、今回は実行していない。コード検証後のprepareで原本journalを保存してから同じ候補branchでWikiを別commitにする。Wiki完了・公開要求・hostのローカルmain実Git反映を区別し、GitHub push・tag・認証変更・デプロイ・稼働環境操作は行わない。
