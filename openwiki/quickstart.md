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
  - id: openwiki-source-0ba09128382dd913fe9a4c4a
    resource: repo://crates/db/src/models/tag.rs
  - id: openwiki-source-c30cc36c4bbe315c59b8e38f
    resource: repo://crates/db/src/models/workspace_tokens.rs
  - id: openwiki-source-38bb7eaa90f0c92a0514844d
    resource: repo://crates/executors/src/legacy_wiki.rs
  - id: openwiki-source-076036ed7bdac82d628d39d1
    resource: repo://crates/server/src/remote_mcp/environment_process.rs
  - id: openwiki-source-0801a7e5bbcc064c12effc80
    resource: repo://crates/server/src/remote_mcp/environment.rs
  - id: openwiki-source-a43253b5be9686da077b8067
    resource: repo://crates/server/src/remote_mcp/issue_comments.rs
  - id: openwiki-source-71f837a8aa36443ad9688945
    resource: repo://crates/server/src/remote_mcp/mod.rs
  - id: openwiki-source-0545bff1ae2507a92137efaf
    resource: repo://crates/server/src/remote_mcp/repository_wiki.rs
  - id: openwiki-source-0987a696c656ec0d4f318f44
    resource: repo://crates/server/src/remote_mcp/run_events.rs
  - id: openwiki-source-5720d865d1c837f0e6b7dab7
    resource: repo://crates/server/src/routes/integrations/conversation.rs
  - id: openwiki-source-b393e75029374eaf9cba5f7d
    resource: repo://crates/server/src/routes/integrations/runtime.rs
  - id: openwiki-source-231241ac3c842dcd67525890
    resource: repo://crates/server/src/routes/openwiki.rs
  - id: openwiki-source-4d617b0e2fa188c6ad63ae56
    resource: repo://crates/server/src/routes/sessions/questions.rs
  - id: openwiki-source-6c4d6d22a9d50e0def61c50d
    resource: repo://crates/server/src/routes/workspaces/integration.rs
  - id: openwiki-source-5590f47e4cee001a35a42117
    resource: repo://crates/server/src/routes/workspaces/wiki.rs
  - id: openwiki-source-df6b7819295f53fc2381f23d
    resource: repo://deploy/server/compose.yaml
  - id: openwiki-source-89d0fb22262f0a0ac1df7d1e
    resource: repo://deploy/server/server.mjs
  - id: openwiki-source-5d1b4189d87b8ac4c58a8f4c
    resource: repo://docs/development/design-notes-validation.md
  - id: openwiki-source-98acd1706f9355ed92cb62a8
    resource: repo://docs/docs.json
  - id: openwiki-source-2e1d91f21691cd271afffb6f
    resource: repo://docs/self-hosting/server-container.mdx
  - id: openwiki-source-ffe48d9fc170154e85861afd
    resource: repo://docs/settings/creating-task-tags.mdx
  - id: openwiki-source-a4deecf361e70bd3b2703d13
    resource: repo://packages/local-web/src/app/auth/serverAccess.ts
  - id: openwiki-source-f5c8f8cc45fdf36d98b214ce
    resource: repo://packages/local-web/src/app/entry/Bootstrap.tsx
  - id: openwiki-source-ddf0f7c06298de76526a4f7c
    resource: repo://packages/web-core/src/features/pipeline/model/cardContext.ts
  - id: openwiki-source-7e0da77c31130e66a4b635a5
    resource: repo://packages/web-core/src/shared/dialogs/command-bar/EditorSelectionDialog.tsx
  - id: openwiki-source-ab4a5800fcfc16d8494e494a
    resource: repo://packages/web-core/src/shared/lib/localIdentity.ts
  - id: openwiki-source-d4726fdd33da0f7c2f1ae1ee
    resource: repo://packages/web-core/src/shared/lib/remoteApi.ts
  - id: openwiki-source-23775c3de52f3ab95a13cb8b
    resource: repo://README.md
generated: { by: "codex", at: "2026-10-29T09:38:09.075Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-29T09:38:09.075Z
---

# クイックスタートと調査案内

lucky-vibe-kanban（LVK）は、Issue で仕事を整理し、Workspace 内で coding agent を動かし、成果物をレビューして Git に統合する製品である。README は看板での計画、agent 実行と変更のレビューを目的として掲げ、複数の Session を graph で協働させる Workflow を主要機能に位置づける。[製品の目的](../README.md) 実装の境界は [システム構成](architecture/system.md)、一連の操作は [Issue から統合まで](workflows/task-to-integration.md) から読む。

## 最初の作業

1. [サーバーコンテナー](operations/server-container.md)の手順で LVK の Docker image を起動し、公開配置では [LVKの初回認証設定](concepts/server-access.md#初回設定と方式変更)、続いて実行環境内で coding agent の認証を済ませる。LVK は npm パッケージを公開しない。ソースから開発する場合は [開発環境と検証範囲](operations/development.md) を参照する。[配布版の案内](../README.md)
2. [Project / Issue](concepts/project-and-issue.md) と、変更先の [Repo / Workspace](concepts/workspace.md) を確認する。仕事の分類と、実ファイルを置く場所は別の設定である。
3. 通常 Session、[WorkflowAttempt](concepts/workflow-attempt.md)、または [Arena](concepts/arena.md) を選び、[Setup → 実行 → レビュー → 統合](workflows/task-to-integration.md) を進める。local Board で複数 Card の採用成果を統合する場合は [自動マージ](concepts/formal-integration.md)を使う。同じ割当Issue / Workspace / Sessionの担当が候補・必要な検証/根拠付き省略・同候補Wiki・公開要求まで進める。担当Run終了後にhostが安全条件を検査してlocal targetへ公開する。検証省略は成功したテストを意味しない。
4. canonical repository memory は [OpenWiki 初期生成・Sync](operations/openwiki-maintenance.md)を Repo ごとに設定する。新規 Card の既定 [Shared directories context](concepts/card-context-and-llm-wiki.md)とは独立しており、旧 LLM Wiki の自動生成は終了している。成果物は [OpenWiki Viewer の表示元](operations/workspace-inspection.md#wiki-の本文ツリーと表示元を確認する)を確認し、閲覧可能と publication 成功を区別する。

サーバーに常駐させる場合は [単一サーバーコンテナー](operations/server-container.md)を参照する。信頼された一チーム向けHTTPS配布であり、データ永続化と実行プロセスの継続を区別する。旧 Cloud / relay は退役している。

旧 LVK npx ランチャーと Tauri desktop shell は撤去済みである。現在はブラウザーを使う。以前の server・MCP・desktop の起動設定を使っている場合は、[移行先と既存データの保持範囲](operations/server-container.md#旧-npx-ランチャーの撤去と移行)を確認する。[配布の移行案内](../README.md#distribution)

画面の基本操作は [Get Started](../docs/getting-started.mdx) が入口になる。現行Boardはlocal APIを使い、公開配置ではBasic/Auth0の認証actorを使う。旧Cloud sign-inやremote API baseは要求しない。入口認証と外部 MCP の認可は [接続境界](integrations/remote-access.md)を参照する。[local API](../packages/web-core/src/shared/lib/remoteApi.ts)

## 主要概念の説明先

| 知りたいこと | 説明の本拠地 |
| --- | --- |
| local Board の identity と Organization は何か | [Organization・Member・Invitation](concepts/organization-and-membership.md)。固定Organization、承認制メンバー、現在actorと過去ownerの区別 |
| LVKへ入る認証と設定の寿命は何か | [サーバー認証・設定・セッション](concepts/server-access.md)。Local/Public、初回setup、Basic/Auth0切替、失効、内部agentと回復 |
| 何を作るか、誰の仕事か、status や親子関係は何か | [Project・Issue](concepts/project-and-issue.md)。ボード表示と共有 status の差、Workspace との link と旧 Task の違いもここ |
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
| prompt の再利用文脈と並列 source をどう扱うか | [Card context・Shared dirs・旧 LLM Wiki](concepts/card-context-and-llm-wiki.md)。fresh 会話への保存文脈、固定 snapshot と移行境界 |
| 採用した並列成果を安全に local target へ反映するには | [自動マージ](concepts/formal-integration.md)。固定source/base、同じ会話の候補・検証/省略・Wiki、hostの公開と取消 |
| 並列作業の知識を統合済み repository にどう反映するか | [Repository memory](concepts/repository-memory.md)。canonical Wiki、Change Manifest、Workspace Memory、source checkpoint と receipt |

名称だけを確認する場合は [自動マージの名称と互換境界](concepts/formal-integration.md#名称と互換境界)を読む。API・tool の識別子と画面名を区別し、マージ済みコードと候補内Wikiの完了と公開の結果を別々に確認する。

## 変更内容から調査する

| 変更・障害 | 調査経路 |
| --- | --- |
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
| サーバーの CPU・RAM・DISK と制限を読む | [画面の共通観測](architecture/web-and-sync.md#cpuramdisk-の共通観測を表示する)と [外部 MCP](integrations/remote-access.md#オンデマンド資源観測)は同じ collector を使う。system-visible と cgroup、保存先の filesystem、unknown・部分失敗を区別する。資源予約の代わりにはならない |
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

Settings の Tag は tag_name/content を持つ、再利用する prompt text の保存モデルである。[Creating Tags](../docs/settings/creating-task-tags.mdx) は @mention から snippet を挿入する操作を説明する。Issue の分類 Tag は project_id/name/color を持ち、IssueTag が issue_id と tag_id を関連づける。名前が同じでも保存対象と利用目的が異なる。[prompt Tag](../crates/db/src/models/tag.rs)・[分類 Tag](../crates/api-types/src/tag.rs)・[IssueTag](../crates/api-types/src/issue_tag.rs)

### Review の違い

Issue の [共有レビュー](concepts/issue-collaboration.md#共有レビューは通常コメントとして残す)は、外部 AI が通常コメントへ意見を残す仕組みであり、要求変更・実装開始・承認を意味しない。[投稿境界](../crates/server/src/remote_mcp/issue_comments.rs#L135-L153)

通常 Workspace の [Agent review](workflows/task-to-integration.md#3-会話を続け成果物を確認する) と [OpenWiki の独立 Review](operations/openwiki-maintenance.md) は別 lifecycle である。旧 Cloud Review の upload / GitHub App worker は現行 local 配布にない。

## 既存文書を使うとき

操作手順は Wiki へ全文コピーせず、[文書 navigation](../docs/docs.json) から原典を使う。Workspace 操作、Boards and issues、Settings、agent 別設定、IDE / Git host integration が主な文書群である。[navigation](../docs/docs.json)

実装の確認には source・tests・実際の設定を使い、文書は目的・契約・過去の判断の証拠として状態を区別する。特に次の適用差は各説明先で扱う。

- Workflow の graph と prompt 展開は [WorkflowAttempt](concepts/workflow-attempt.md) の現行 validator を確認する。
- Bootstrap v2 の一部は後続 implementation record が更新している：[OpenWiki 設計文書](operations/openwiki-maintenance.md#設計文書の読み方)。
- [内蔵 Astra は廃止済み](concepts/project-chat.md)。旧モデル設定や専用会話履歴の説明を現行機能と扱わず、残る質問回答と外部 MCP の契約を確認する。
- 自己開発の旧 development container と固定 staging 候補は区別する：[開発・検証](operations/development.md)・[サーバー運用](operations/server-container.md)。

## カバー範囲と限界

この構成は、README と文書 index に加え、local server の router/startup、local Board と外部 MCP の router、Workflow/Agent runtime、Git と memory の境界から照合している。文書の章構成や直近差分だけを説明範囲にはしていない。[local API の入口](../crates/server/src/routes/mod.rs)・[外部 MCP](../crates/server/src/remote_mcp/mod.rs)・[MCP の入口](../crates/mcp/src/task_server/tools/mod.rs)

この Wiki は実装理解のための要約であり、すべての UI 項目や依存 package の一覧ではない。外部サービス、配布済み binary、IDE extension、model の意味的な出力品質は、この source 調査だけでは確定しない。製品 test の実行結果については [検証記録の限界](operations/development.md) を参照する。

過去の Sync は凍結された pending changes と統合済み source `066b2b4bd93d06d046865d66ae16bf1537e8b184` に沿い、Workspace 累積トークンを照合した。native adapter と親 thread scope、migration と snapshot/host cursor の保存、UI の query と HTTP router、stdio/外部 MCP、独立した Rust fixture と Chromium spec を入口として確認した。集計は取得済み Codex 親 thread の報告値であり、未取得は unknown、報告ありも partial で全量を保証しない。[集計契約](concepts/workspace.md#workspace-累積トークン)・[根拠となる応答](../crates/db/src/models/workspace_tokens.rs#L42-L70)・[検証の分担](operations/development.md#workspace-トークン集計の検証)。無関係な過去の説明全体を再検証したという意味ではない。

前回の通常 Sync は凍結された pending changes と統合済み source `e378eb281dde23cd9283c96e6db3cfae3379518b` を対象とし、[Runtime Access](concepts/runtime-access.md) の grant 簡素化を照合した。外部HTTPのscope入口、dispatchとProject照合、設定のDB/legacy読取とrevision付き保存、Linux process registryと再起動receipt、設定UI・生成型・日本語ヘルプ、HTTP/設定/exec/restartの各fixtureを独立した入口として確認した。subject／実行ファイル許可リストの撤去と、残る所有者・cwd・Workspace境界を正本ページへ反映した。関連する外部MCP・システム構成の説明も照合し、無関係な過去の内容全体を再検証したとは扱わない。[現行grantとdispatch](../crates/server/src/remote_mcp/environment.rs#L309-L403)・[exec遷移の回帰](../crates/server/src/remote_mcp/environment_process.rs#L640-L700)・[設定互換性の回帰](../crates/server/src/remote_mcp/environment_settings_tests.rs#L496-L507)

前回の Runtime Access の凍結入力は、開発時のテスト成功報告と自動マージ の検証省略を区別していた。これはその対象範囲の履歴であり、後続 自動マージ の検証結果を示すものではない。


### 共有レビュー Sync の照合範囲

統合済み source `be97e258698174883272eadcd325ae743d30b7d8` と凍結された pending changes を対象に、[Issue の共有レビュー](concepts/issue-collaboration.md#共有レビューは通常コメントとして残す)を照合した。独立した入口として外部 HTTP の認証・catalog・dispatch、投稿 receipt と actor 継承、migration と通常 Web 編集、Rust API 型・生成型・Web 表示、Unicode decoder・SQL・handler 回帰、隔離 JWT bridge と Chromium spec を確認した。対象 Issue revision と本文 revision、追記と要求変更、来歴と監査証明を分けて正本へ反映した。[投稿・読取の根拠](../crates/server/src/remote_mcp/issue_comments.rs#L85-L279)・[UI の根拠](../packages/ui/src/components/IssueCommentsSection.tsx#L327-L373)・[検証入口](concepts/issue-collaboration.md#変更時の検証入口)

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

凍結 Change Manifest は開発時の局所・native 試験成功と自動マージ の検証省略を区別している。[開発記録](../docs/development/preview-routes-validation.md)も実 ACME・DNS・VPS・アプリ・配布 image を未確認としている。今回の検証は統合 source・試験定義・文書差分・リンクと OpenWiki lifecycle の静的照合であり、製品試験や証明書発行の再実行ではない。

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

供給された開発/候補合成記録には局所検証の成功と修復履歴があるが、最終自動マージは明示的に検証省略だった。実agentの承認待ち到達と取消は人の承認後の実tool成功ではなく、offline UI fixtureも実API/認証ではない。この保守はソース・試験定義・差分・リンクとOpenWiki lifecycleを検証し、製品suite、実Auth0/ChatGPT、実モデル、稼働DB移行、配布・restartを再実行していない。[候補の検証限界](../docs/development/seven-source-candidate.md#root-cloud-qaと残る確認)

### 自動マージ名称 Sync の照合範囲

凍結入力の manifest と全4チャンクの SHA-256、統合 source `e4a2dc3ca101233becb63b355f7020f5396da415` を照合した。UI/i18n、MCP の catalog・受付・読取・判断、host の merge/validation/publication、DB の予約・Done receipt と回帰試験、Wiki 開始 guard を独立した入口から確認した。名称と互換性の詳細は [自動マージの正本](concepts/formal-integration.md#名称と互換境界)、実際の順序は [並列成果を公開する手順](workflows/task-to-integration.md#並列変更を試してから正式統合する)に置く。

変更は利用者向け名称と DB 試験期待値への追随で、既存の機械識別子・承認・保存形式を維持する。最終候補の検証成功は供給された Manifest の記録であり、この Wiki 保守はソース・試験定義・差分・リンクの静的照合を行った。製品 suite、実モデル・Auth0 / ChatGPT、稼働 DB・通知配送・配布を再実行しておらず、無関係な歴史的内容全体を再検証したとは扱わない。
