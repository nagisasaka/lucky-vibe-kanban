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
  - id: openwiki-source-dfa8135bd6d4dbe37911c061
    resource: repo://crates/db/src/models/workspace_usage.rs
  - id: openwiki-source-38bb7eaa90f0c92a0514844d
    resource: repo://crates/executors/src/legacy_wiki.rs
  - id: openwiki-source-076036ed7bdac82d628d39d1
    resource: repo://crates/server/src/remote_mcp/environment_process.rs
  - id: openwiki-source-6cb471b32e19ce921b78d86e
    resource: repo://crates/server/src/remote_mcp/environment_settings.rs
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
  - id: openwiki-source-b393e75029374eaf9cba5f7d
    resource: repo://crates/server/src/routes/integrations/runtime.rs
  - id: openwiki-source-231241ac3c842dcd67525890
    resource: repo://crates/server/src/routes/openwiki.rs
  - id: openwiki-source-4d617b0e2fa188c6ad63ae56
    resource: repo://crates/server/src/routes/sessions/questions.rs
  - id: openwiki-source-5590f47e4cee001a35a42117
    resource: repo://crates/server/src/routes/workspaces/wiki.rs
  - id: openwiki-source-df6b7819295f53fc2381f23d
    resource: repo://deploy/server/compose.yaml
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
  - id: openwiki-source-db035046d01e3f24722812c8
    resource: repo://packages/web-core/src/features/kanban/ui/IntegrationPanel.tsx
  - id: openwiki-source-ddf0f7c06298de76526a4f7c
    resource: repo://packages/web-core/src/features/pipeline/model/cardContext.ts
  - id: openwiki-source-ab4a5800fcfc16d8494e494a
    resource: repo://packages/web-core/src/shared/lib/localIdentity.ts
  - id: openwiki-source-d4726fdd33da0f7c2f1ae1ee
    resource: repo://packages/web-core/src/shared/lib/remoteApi.ts
  - id: openwiki-source-23775c3de52f3ab95a13cb8b
    resource: repo://README.md
generated: { by: "codex", at: "2026-10-24T11:23:36.861Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-24T11:23:36.861Z
---

# クイックスタートと調査案内

lucky-vibe-kanban（LVK）は、Issue で仕事を整理し、Workspace 内で coding agent を動かし、成果物をレビューして Git に統合する製品である。README は看板での計画、agent 実行と変更のレビューを目的として掲げ、複数の Session を graph で協働させる Workflow を主要機能に位置づける。[製品の目的](../README.md) 実装の境界は [システム構成](architecture/system.md)、一連の操作は [Issue から統合まで](workflows/task-to-integration.md) から読む。

## 最初の作業

1. [サーバーコンテナー](operations/server-container.md)の手順で LVK の Docker image を起動し、公開配置では [LVKの初回認証設定](concepts/server-access.md#初回設定と方式変更)、続いて実行環境内で coding agent の認証を済ませる。LVK は npm パッケージを公開しない。ソースから開発する場合は [開発環境と検証範囲](operations/development.md) を参照する。[配布版の案内](../README.md)
2. [Project / Issue](concepts/project-and-issue.md) と、変更先の [Repo / Workspace](concepts/workspace.md) を確認する。仕事の分類と、実ファイルを置く場所は別の設定である。
3. 通常 Session、[WorkflowAttempt](concepts/workflow-attempt.md)、または [Arena](concepts/arena.md) を選び、[Setup → 実行 → レビュー → 統合](workflows/task-to-integration.md) を進める。local Board で複数 Card の採用成果を統合する場合は [正式 Integration](concepts/formal-integration.md)を使う。専用環境で通常 merge は host、競合解消は有限の Agent job が担当し、review 後の最終候補を受付時の検証方針に従って処理する。local target の公開と条件付き Done を扱う。検証省略の NotRun は成功したテストを意味しない。
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
| 外部会話からログを読み既存アプリを動かすには | [実行環境へのアクセス](concepts/runtime-access.md)。Project・ディレクトリ単位の管理者grant、runtime scope、直接processの寿命とAgentRun・共有資源との違い |
| タブを閉じた後の Codex 待機・終了を知りたい | [Web Push・通知同意・会話への復帰](integrations/web-push.md)。Project/browser 登録、logout と別の同意寿命、配送失敗、参照先への復帰 |
| 複数段階の仕事をどう定義して動かすか | [WorkflowAttempt](concepts/workflow-attempt.md)。template と Run、分岐・合流、人の判断待ち |
| 候補をどう比較し、採用後の実装へ移るか | [Arena](concepts/arena.md)。Design / Implementation、候補、promotion と Workflow 内の選択 |
| prompt の再利用文脈と並列 source をどう扱うか | [Card context・Shared dirs・旧 LLM Wiki](concepts/card-context-and-llm-wiki.md)。fresh 会話への保存文脈、固定 snapshot と移行境界 |
| 採用した並列成果を安全に local target へ反映するには | [正式 Integration](concepts/formal-integration.md)。受付予約、host merge・競合 review・最終検証、公開と取消、限定 checkpoint adoption |
| 並列作業の知識を統合済み repository にどう反映するか | [Repository memory](concepts/repository-memory.md)。canonical Wiki、Change Manifest、Workspace Memory、source checkpoint と receipt |

## 変更内容から調査する

| 変更・障害 | 調査経路 |
| --- | --- |
| 起動、DB、standalone と残存 startup API の差 | [システム境界と起動順](architecture/system.md) → [開発・検証](operations/development.md) |
| Agent が止まった、表示と監査が合わない、再接続したい | [Session の状態](concepts/session-and-agent-run.md) → [Agent Runtime の監査・投影・回復](architecture/agent-runtime.md) |
| Workspace の累積トークン、内訳、不明・一部のみの意味 | [Workspace 累積トークン](concepts/workspace.md#workspace-累積トークン) → [画面で確認](operations/workspace-inspection.md#累積トークンを確認する)。context gauge・各 run 時間との違い、thread 単位の保持、MCP の同じ集計を確認する |
| provider や model 設定、MCP、Skill の追加 | [Provider integration](integrations/agent-providers.md)。effort override と解決済み値、外部 MCP を使う方向と LVK MCP を公開する方向を分ける |
| Workflow の分岐、取消、outbox、再起動回復 | [Graph の契約](concepts/workflow-attempt.md) → [Workflow Runtime](architecture/workflow-runtime.md) |
| 定期的に作業を実行したい | [ScheduledTask の起動・skip 条件](workflows/task-to-integration.md#定時に-workflow-を起動する) |
| Card の列が動かない、失敗後も In Progress、追加実行で戻る | [Kanban と実行の同期契約](concepts/project-and-issue.md#kanban-と実行の同期契約)。実開始時刻、手動変更優先、他 run の待機と正式 Integration の Done の違い |
| frontend、local Board の取得と更新 | [Web と同期](architecture/web-and-sync.md) → [外部 MCP と認証](integrations/remote-access.md) |
| ファイル、Session 添付、diff、preview、terminal、editor | [Workspace inspection](operations/workspace-inspection.md)。Board の参照は [Issue の共同作業](concepts/issue-collaboration.md) |
| 外部 MCP が認証・認可で失敗する | [Auth0 JWTとscope](integrations/remote-access.md)。active membership、read/write/merge と runtime専用scope、stateless transportの境界 |
| 実行環境の許可を保存・取り消す、旧形式・legacy設定を復旧する | [Runtime Access の管理設定](concepts/runtime-access.md#管理設定の保存適用取消)。DB優先・revision競合・明示復旧と、取消が停止を意味しない契約。削除済みfieldを含む設定は[旧grant形式の扱い](concepts/runtime-access.md#旧-grant-形式と画面の境界)へ |
| 外部会話から保存済み Codex 進捗を読みたい | [list_run_events](integrations/remote-access.md#保存済み-codex-進捗の読取)。明示 Run 参照、除外を含む cursor、commentary と最終回答の分離 |
| 既存 Goal を外部会話から一時停止・再開・取消・解除する | [Goal の寿命と保存再開](concepts/session-and-agent-run.md#goal-の寿命) → [MCP の対象固定と結果確認](integrations/remote-access.md#既存-goal-を-mcp-から操作する)。達成・解除・Run 終了の違いと、revision・receipt・不確実な配送を確認する |
| Plan が戻る、旧 Plan with Goal、context や子の表示が違う | [Session の Plan 遷移・context・子 lifecycle](concepts/session-and-agent-run.md#plan-の遷移) → [CLI / wire 型と mapper 互換](integrations/agent-providers.md#cli-検出と-provider-互換性) |
| GitHub の認証、push 先、PR 結果や merge 観測が合わない | [GitHub 認証と PR](integrations/github-review.md)。リポジトリ別 PAT、agent shell との適用差、push/base と部分成功 |
| サーバーの CPU・RAM・DISK と制限を読む | [画面の共通観測](architecture/web-and-sync.md#cpuramdisk-の共通観測を表示する)と [外部 MCP](integrations/remote-access.md#オンデマンド資源観測)は同じ collector を使う。system-visible と cgroup、保存先の filesystem、unknown・部分失敗を区別する。資源予約の代わりにはならない |
| 開発 instance の成果物を切り替える、失敗後に復旧する | [切替 engine の運用](operations/development.md#開発用成果物切替と復旧)。専用 MCP の [自己再起動](concepts/runtime-access.md#開発-instance-の専用自己再起動)からの受付、排他、backup、停止証拠、durable 結果を区別する |
| 終了した Workspace のディスクを解放する | [手動 archive](concepts/workspace.md#archive削除期限-cleanup)。busy 検査、一度の確認、削除失敗と履歴保持。外部会話からは [MCP の準備・確認と結果読戻し](integrations/remote-access.md#workspace-を外部-mcp-から-archive-する)へ進む。merge・PR・Arena の終了だけでは自動 archive しない |
| 外部マシンの実行を予約・回復する | [共有資源と Execution Bridge](concepts/shared-resource-coordination.md)。snapshot、Runner、settlement と operator 回復 |
| 実行の元データを保全したい | [Native Audit の保存・エクスポート・削除](architecture/agent-runtime.md#native-audit-の保全エクスポート寿命) |
| Merge / Rebase / PR 後の状態が合わない | [統合までの順序と部分失敗](workflows/task-to-integration.md) → [memory の統合契約](concepts/repository-memory.md) |
| Wiki を読む、表示対象が違う | [Wiki の本文・ツリーと表示元](operations/workspace-inspection.md#wiki-の本文ツリーと表示元を確認する) → [共有 reader とチャット保持](architecture/web-and-sync.md#wiki-のナビゲーションと本文を一つの状態で動かす) |
| 外部会話から既存 Wiki を同期・再同期する | [MCP Sync の固定対象・再送・結果読戻し](operations/openwiki-maintenance.md#mcp-から既存-wiki-を同期する)。受付成功、公開・照合完了、現在の鮮度を分け、初期生成は Repository settings で行う |
| Wiki 保守の Card がない、紐付け先 Project が違う | [開始元 Project と保守 Issue](operations/openwiki-maintenance.md#開始元-project-と保守-issue)。Project 設定/MCP の引継ぎ、関連検証と未指定時の互換動作を確認する |
| Wiki が stale/error、Bootstrap が途中で止まる | [OpenWiki の phase・共通索引・レポート・完了証拠・回復](operations/openwiki-maintenance.md)。MCP 起動前確認と公開条件を分けて診断する |
| Integration の説明不足、古い候補、反映結果の照合中 | [保存済み解消説明と承認条件](concepts/formal-integration.md#保存済み説明から候補をレビューする) → [公開・Done・Wiki の区別](concepts/formal-integration.md#公開donewiki-は別の結果)。操作は [統合までの手順](workflows/task-to-integration.md#並列変更を試してから正式統合する) |
| 外部会話から Integration の承認・取消を進める | [MCP の固定 gate・期待値・操作 receipt](integrations/remote-access.md#mcp-から-integration-を承認取消する)。候補の失効、検証例外、取消受付と予約解放の違いを確認する |
| 統合・Wiki 実行のログを見る、所有者を止める | [Execution の閲覧と停止](operations/workspace-inspection.md#実行専用-workspace-の確認)。通常開発用 Workspace と操作入口を分ける |
| local LVK を常駐サーバーへ置く | [単一サーバーコンテナー](operations/server-container.md)。HTTPSとbackend認証、preview handoff、volume、停止と rollback |
| モバイル表示の検証 | [built preview と検証](operations/development.md)。稼働 API と分離した frontend bundle |

## 編集・接続の不調から調べる

ブラウザーの Markdown / HTML / raw paste、音の試聴、server 側 editor 起動は [Web の操作境界](architecture/web-and-sync.md#ブラウザー操作と廃止された-native-bridge)を確認する。native shell の廃止と共有機能の存続を分け、貼り付け fixture と実 OS clipboard・live 通知の検証範囲は [開発・検証](operations/development.md#ブラウザー貼り付けの回帰)で確認する。

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

前回の Runtime Access の凍結入力は、開発時のテスト成功報告と正式 Integration の検証省略を区別していた。これはその対象範囲の履歴であり、後続 Integration の検証結果を示すものではない。


### 共有レビュー Sync の照合範囲

統合済み source `be97e258698174883272eadcd325ae743d30b7d8` と凍結された pending changes を対象に、[Issue の共有レビュー](concepts/issue-collaboration.md#共有レビューは通常コメントとして残す)を照合した。独立した入口として外部 HTTP の認証・catalog・dispatch、投稿 receipt と actor 継承、migration と通常 Web 編集、Rust API 型・生成型・Web 表示、Unicode decoder・SQL・handler 回帰、隔離 JWT bridge と Chromium spec を確認した。対象 Issue revision と本文 revision、追記と要求変更、来歴と監査証明を分けて正本へ反映した。[投稿・読取の根拠](../crates/server/src/remote_mcp/issue_comments.rs#L85-L279)・[UI の根拠](../packages/ui/src/components/IssueCommentsSection.tsx#L327-L373)・[検証入口](concepts/issue-collaboration.md#変更時の検証入口)

同じ範囲で、[正式 Integration の固定検証 plan](concepts/formal-integration.md#検証方針は受付時に固定する)に依存準備 `pnpm install --frozen-lockfile` が先頭追加されたことを確認した。[設定](../.lvk/integration-validation.json)

凍結入力には、初回の型生成・Rust/browser 検証の保留、その後の開発検証成功、最後の正式 Integration の install / frontend build / check / lint / backend:test 成功が順に記録されている。これらは供給された過去の実行記録であり、今回の文書保守による再実行結果ではない。今回行ったのは source・テスト内容・文書差分の静的照合で、実 Auth0/ChatGPT、配布サーバー、実DB移行は確認していない。無関係な過去の Wiki 内容全体を再検証したとは扱わない。


### 今回の設計メモ Sync の照合範囲

凍結入力全9チャンクと統合済み source `b7371bd196c9113a16e1efc8116bcf480f588d09` を対象に、[設計メモ](concepts/design-notes.md)を照合した。独立した入口として Project 表示と編集state、RESTとactor、stdio/HTTP MCPのcatalog・scope・receipt、DBの版管理・有界読取、保持migrationとProject削除、DB/server/隔離ブラウザの回帰定義を確認した。共有routerの変更で影響した既存Claimsも再照合した。無関係な過去のWiki内容全体を再検証したとは扱わない。

途中のProject削除409方針は撤回済みで、最終実装は履歴を保持し既存アクセスを閉じる。保持後の閲覧・持出し・再関連付け・期限は未決である。[現行保持契約](concepts/design-notes.md#project-を削除した後の保持)。メモの記録を実行承認とせず、独立追記と本文への明示統合を区別する。

凍結入力は開発検証の保留・後続成功と、最終Integrationの install / frontend build / check / lint / backend:test 成功を記録している。欠落したsemantic draftのcheckpointから動機は推定していない。これらは供給された過去の実行記録であり、今回の文書保守によるテスト再実行ではない。今回の検証はソース・テスト内容・リンク・文書差分とOpenWiki lifecycleの検証で、実Auth0/ChatGPTや稼働DBへのmigration適用は含まない。[変更時の検証入口](concepts/design-notes.md#変更時の検証入口)

### Repository Wiki MCP 同期の照合範囲

今回の通常 Sync は凍結 manifest と全4チャンク（SHA-256照合済み）、統合済み source `de20c156dc28ead686f5f1322ae787b1abd6ff42` の pending changes を対象とした。UI設定からRESTへの入口、外部MCPのcatalog・scope・adapter、共通maintenance受付とscope解決、operation receipt、Workspace owner結果保存、Git公開と回復、独立した対象拒否・再送・公開判定・HTTP scopeのfixtureを照合した。詳細契約は [OpenWiki保守](operations/openwiki-maintenance.md#mcp-から既存-wiki-を同期する)、権限境界は [外部MCP](integrations/remote-access.md#repository-wiki-の同期を依頼する)、鮮度の意味は [Repository Memory](concepts/repository-memory.md#並行性receipt失敗)に置く。無関係な過去の説明全体を再検証したとは扱わない。

凍結Change Manifestでは開発時の型検査成功と実行試験の保留、正式Integrationの明示的な検証省略が記録されている。付随するWorkspace Memoryは後続の限定試験成功も報告するが、これは供給された履歴であり、今回の実行結果ではない。今回の検証は統合source・試験定義・リンク・文書差分とOpenWiki lifecycleの照合で、製品試験、実Auth0 / ChatGPT接続、実モデルによる同期は再実行していない。[試験と限界の入口](operations/openwiki-maintenance.md#不確実な結果と検証の境界)

### 保守開始元 Project の照合範囲

今回の Sync は、凍結 manifest・全4チャンクの SHA-256 と source `7297e1148324cb8a2b0c03c29b23d4a74dd4092d` を照合した。Project 設定 UI、MCP の認可・adapter、共通受付、ProjectRepoDefaults を含む関連集合、Issue 作成 transaction、過去補完、repository Workflow の所有形式と各 fixture から、[開始元 Project と保守 Issue](operations/openwiki-maintenance.md#開始元-project-と保守-issue)の契約を更新した。共有ファイルの変更で stale となった既存 Claims も該当部分を確認したが、無関係な過去の Wiki 全体を再検証したものではない。

凍結入力の最初の checkpoint には意味 draft がなく、意図・理由・試験結果は不明である。後続 Manifest の開発試験成功は履歴として扱い、正式 Integration の検証省略を合格へ読み替えない。この保守で行ったのは source・試験定義・文書差分・リンク・OpenWiki lifecycle の検証であり、製品試験、実モデルによる製品の同期処理、実DB・外部認証は再実行していない。

### 外部 MCP archive の照合範囲

今回の Sync は凍結 manifest と全1チャンクの SHA-256、統合済み source `505b8f9cac53b26c9609068672fa18a87b7b58a8` を照合した。HTTP 認証・catalog・scope、外部 adapter、UI/stdio の共通 archive、DB 所属・busy 検査、strict cleanup、operation receipt と現在値の読取、署名 JWT を使う HTTP fixture を独立した入口として確認した。詳細は [Workspace の確認と回復](concepts/workspace.md#外部-mcp-の確認と結果の回復)、利用導線は [外部 MCP](integrations/remote-access.md#workspace-を外部-mcp-から-archive-する)に集約した。共有入口に関係する既存 Claims は再確認したが、無関係な過去の説明全体を再検証したものではない。

凍結 Change Manifest は正式 Integration の検証を明示的に省略したと記録し、意味的な変更理由や開発テスト結果は供給していない。この保守は実装・試験定義・文書差分の静的照合であり、製品テストや実 Auth0/ChatGPT 接続の再実行ではない。fixture の script liveness と実環境の検証範囲も区別する。
