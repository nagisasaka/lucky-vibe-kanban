---
type: concept
title: Workspace と Repository
description: 作業環境の所有権と用途、実行所有者、複数 Repo、共有資源、削除と回復の境界。
tags: [workspace, repository, worktree, ownership, storage]
sources:
  - id: openwiki-source-94559828ba40b7dfdd09bad0
    resource: repo://crates/db/migrations/20260921000000_workspace_usage.sql
  - id: openwiki-source-f9386b5537a5146f7ac841ae
    resource: repo://crates/db/migrations/20261021173000_workspace_tokens.sql
  - id: openwiki-source-d20a82e2192a07c687b838cb
    resource: repo://crates/db/src/models/agent_runtime.rs
  - id: openwiki-source-ccd357b1781e6e90e56fd858
    resource: repo://crates/db/src/models/repo.rs
  - id: openwiki-source-d76d1a64a0c116ad812aa678
    resource: repo://crates/db/src/models/workspace_repo.rs
  - id: openwiki-source-c30cc36c4bbe315c59b8e38f
    resource: repo://crates/db/src/models/workspace_tokens.rs
  - id: openwiki-source-dfa8135bd6d4dbe37911c061
    resource: repo://crates/db/src/models/workspace_usage.rs
  - id: openwiki-source-5f083da2f629bcd57e386ad4
    resource: repo://crates/db/src/models/workspace.rs
  - id: openwiki-source-d0244e39b6cb88790907b9b2
    resource: repo://crates/executors/src/executors/codex/agent_scope.rs
  - id: openwiki-source-f4e0f9b2ce513d3f8b8b070e
    resource: repo://crates/executors/src/executors/provider_adapter.rs
  - id: openwiki-source-788fa2f6f1a3d449a1a9efe4
    resource: repo://crates/local-deployment/src/agent_run_port.rs
  - id: openwiki-source-343f68138e6e2d3312d2774c
    resource: repo://crates/local-deployment/src/container.rs
  - id: openwiki-source-7464a2d1db8e85c09420cf28
    resource: repo://crates/server/src/middleware/model_loaders.rs
  - id: openwiki-source-9e15762bf7a432b7605f8466
    resource: repo://crates/server/src/remote_mcp/issue_write.rs
  - id: openwiki-source-6adda10f98b079272f82a4cc
    resource: repo://crates/server/src/remote_mcp/operations.rs
  - id: openwiki-source-3651b7ce4535b584634e8bc8
    resource: repo://crates/server/src/remote_mcp/runtime_read.rs
  - id: openwiki-source-0ce0e179a46d5c36c5f05ecf
    resource: repo://crates/server/src/remote_mcp/workspace_archive.rs
  - id: openwiki-source-044c1a309edb203d2c4c162b
    resource: repo://crates/server/src/remote_mcp/workspace_archive/tests.rs
  - id: openwiki-source-5da526ef53eeceecc5bb4b70
    resource: repo://crates/server/src/routes/filesystem.rs
  - id: openwiki-source-16bb0bc0baa397cce8e201b1
    resource: repo://crates/server/src/routes/github_credentials.rs
  - id: openwiki-source-0f6f4d2990a4bfa7aca81dbe
    resource: repo://crates/server/src/routes/openwiki/preparation.rs
  - id: openwiki-source-a34361c91f1fbaa3ea42afc5
    resource: repo://crates/server/src/routes/openwiki/project_context.rs
  - id: openwiki-source-9d283785aa68b40f3cd1ceb2
    resource: repo://crates/server/src/routes/workspaces/archive_processes.rs
  - id: openwiki-source-15fe6957ffd50a6292794c19
    resource: repo://crates/server/src/routes/workspaces/archive.rs
  - id: openwiki-source-6a0749527a9648c33ba957da
    resource: repo://crates/server/src/routes/workspaces/core.rs
  - id: openwiki-source-bdb4dae5722fe76ec5e44001
    resource: repo://crates/server/src/routes/workspaces/create.rs
  - id: openwiki-source-303d74f74def684d4690dea9
    resource: repo://crates/server/src/routes/workspaces/git.rs
  - id: openwiki-source-379c8cd3e1e9932f9adda9ee
    resource: repo://crates/services/src/services/container.rs
  - id: openwiki-source-371e2191a5fb043bb2506b5b
    resource: repo://crates/services/src/services/filesystem.rs
  - id: openwiki-source-72948db7f01925b4b2ff17af
    resource: repo://crates/services/src/services/pr_monitor.rs
  - id: openwiki-source-c128bdfa7a156621b004df06
    resource: repo://crates/services/src/services/workspace_usage.rs
  - id: openwiki-source-6043692809cbf72f253c5631
    resource: repo://crates/services/src/services/workspace_usage/lifecycle.rs
  - id: openwiki-source-5837e5316279163dd8bfbfd3
    resource: repo://crates/utils/src/github_credentials.rs
  - id: openwiki-source-19419edea1d312cc9f07ef2d
    resource: repo://crates/utils/src/path.rs
  - id: openwiki-source-7d5590729f3e0f42d994b0e8
    resource: repo://crates/workspace-manager/src/shared_resources.rs
  - id: openwiki-source-965b490de6f8364cb7695944
    resource: repo://crates/workspace-manager/src/workspace_manager.rs
  - id: openwiki-source-08b3f4dcb1a91c97b6d127f8
    resource: repo://crates/worktree-manager/src/worktree_manager.rs
  - id: openwiki-source-1dc77f9d01b63e6f9dfd9b6b
    resource: repo://docs/development/workspace-token-usage.md
  - id: openwiki-source-5f6ef843ad438c4b8a15f879
    resource: repo://docs/workspaces/managing-workspaces.mdx
  - id: openwiki-source-00110cdcfec890895b99690a
    resource: repo://packages/web-core/src/shared/dialogs/settings/settings/RemoteProjectsSettingsSection.tsx
  - id: openwiki-source-dbe2d46e94202832ad4e5e7b
    resource: repo://packages/web-core/src/shared/dialogs/shared/ServerFolderPickerDialog.tsx
  - id: openwiki-source-5d2382a0f231c830f95f103c
    resource: repo://packages/web-core/src/shared/dialogs/shared/WorkspaceTargetDialog.tsx
  - id: openwiki-source-5ea1902fdf1213351e6386d9
    resource: repo://packages/web-core/src/shared/lib/api.ts
  - id: openwiki-source-9ca1dca04d93c4fed8664d25
    resource: repo://packages/web-core/src/shared/lib/localApiTransport.ts
generated: { by: "codex", at: "2026-10-24T14:40:59.450Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-24T14:40:59.450Z
---

# Workspace と Repository

Workspace は、エージェントの [Session](session-and-agent-run.md)、作業ディレクトリ、ブランチ、接続 Repo を束ねる実施環境である。[Issue](project-and-issue.md) の仕事とは関連で結び、一つの Workspace に複数 Repo・Session を持てる。`container_ref` はローカル実装ではファイルシステム上のパスを表す。名称だけから Docker や OS の隔離境界を想定しない。[Workspace モデル](../../crates/db/src/models/workspace.rs#L62-L82)、[ローカルの作成](../../crates/local-deployment/src/container.rs)

## Repo と WorkspaceRepo

**Repo** は登録された source パスと、setup/cleanup/archive/dev script、copy files、target branch・working directory の既定値を持つ。**WorkspaceRepo** は登録 Repo を特定 Workspace に接続し、その実施で使う target branch を保存する。Repo の既定値と、既存 Workspace の選択を区別する。[Repo](../../crates/db/src/models/repo.rs#L36-L54)、[membership](../../crates/db/src/models/workspace_repo.rs#L11-L34)

通常の追加は Repo 存在、target branch 存在、重複 attachment を検証する。managed Workspace のルート直下に Repo 名ごとの Git worktree を作り、共通の作業 branch 名を各 Repo で使用する。各 Repo の Git 履歴・target branch は独立しているため、一つの Workspace が複数 Repo をまとめていても Git の一括 transaction にはならない。[追加検証](../../crates/workspace-manager/src/workspace_manager.rs#L133-L162)、[worktree 作成](../../crates/workspace-manager/src/workspace_manager.rs#L528-L571)、[複数 Repo の利用契約](../../docs/workspaces/multi-repo-sessions.mdx)

GitHub 認証の設定入口は Repo にあるが、token の保存 identity は Repo UUID や Workspace ではなく、instance 内の正規化した GitHub `owner/repository` である。同じ remote を使う Workspace は資格情報を共有する。対象は登録 Repo の remotes と既定 push URL から選ぶ。[PAT の管理・適用範囲](../integrations/github-review.md#リポジトリ別-pat-の所有と管理)に保存・削除・失敗時の契約を集約する。[対象の解決](../../crates/server/src/routes/github_credentials.rs#L32-L63)・[保存キー](../../crates/utils/src/github_credentials.rs#L147-L177)

## 二つの環境と所有権

| 種別 | 作業場所 | ファイル所有 |
| --- | --- | --- |
| Worktree | LVK 管理のルート内に作る Repo ごとの worktree | Managed |
| DirectFolder | 利用者が選んだ既存ディレクトリ | External |

削除可能性は kind の名称からではなく `container_ownership` から判定する。External の container path を LVK が削除することは許可しない。[所有権](../../crates/db/src/models/workspace.rs#L35-L53)、[判定とテスト](../../crates/db/src/models/workspace.rs#L137-L148)、[削除テスト](../../crates/db/src/models/workspace.rs#L876-L886)

DirectFolder で選択先が開ける Git Repo なら Repo を登録し、現在の branch を使い、選択ディレクトリの親を container_ref にする。通常の Repo 相対解決で選択先に到達するためである。Git Repo でなければ選択先自体をルートとし、Repo membership を持たない。DirectFolder は新しい Git worktree を作る操作ではない。[DirectFolder 作成](../../crates/server/src/routes/workspaces/create.rs)、[既存パス検証](../../crates/local-deployment/src/container.rs)

Worktree の保管先を変更すると、指定先の `.vibe-kanban-workspaces` 子ディレクトリを使用する。アプリ所有領域を区別するための配置であり、現在は起動時 orphan 回収を行わない。[保管先](../../crates/worktree-manager/src/worktree_manager.rs#L609-L625)

## 操作用途と実行所有者

`usage` は Workspace で人が行える操作の契約で、物理的な kind/ownership、archive、実行中の一時予約とは独立する。既定の `interactive` は通常の開発用、`execution_only` は [正式統合](formal-integration.md)や [OpenWiki 保守](../operations/openwiki-maintenance.md)のために所有者が作った実行環境である。execution-only は実行中の一時予約であり、所有者の終了と process / cleanup の収束後は interactive に戻せる。履歴・execution_owner・結果は保持する。[用途モデル](../../crates/db/src/models/workspace_usage.rs#L1-L53)、[永続列](../../crates/db/migrations/20260921000000_workspace_usage.sql)

`execution_owner` は kind・run ID・Repo ID により製品の実行記録を指す。準備中は run ID が未確定でもよいが、dispatch 前に所有者へ結び付け、別 run に付け替えて過去の環境を再利用しない。Sync など製品側に残らない結果は、同じ所有者へ succeeded/failed/cancelled、Wiki commit、no-op、診断を保存する。AgentRun の成功だけでは公開結果を推定しない。[一度だけの結付けと結果保存](../../crates/db/src/models/workspace_usage.rs#L81-L131)

実行専用環境でも Session と監査ログは残り、ファイルが現存すれば閲覧できる。予約中の汎用 chat・Session 作成・retry・Git・terminal・script・ファイル変更等は通常の開発入口では許可しない。現在委任中の Wiki AgentRun への追加指示だけは owner identity と非終端・非取消を確認する専用例外がある。Workspace middleware は read と seen を残し、停止は所有者専用入口へ渡す。手動 archive の prepare/confirm だけは usage による拒否の例外で、実際の使用中判定を必須にする。unarchive・再利用・他の mutation の権限は緩めない。旧 editor tunnel は現行構成から削除されている。[共通境界](../../crates/server/src/middleware/model_loaders.rs)。表示と停止の導線は [実行履歴の確認](../operations/workspace-inspection.md#実行専用-workspace-の確認)を参照する。

所有者ラベル自体は起動権限ではない。各 dispatch で Repo membership と未完了の所有を確認し、Integration は保存済み Workspace/Session/correlation と予約を、Bootstrap は Workflow と現在の delegated child を、Sync は maintenance state と起動前に固定した AgentRun ID を照合する。不明 kind や欠けた owner は閲覧対象のままにし、任意の実行権限を与えない。[agent の検証](../../crates/services/src/services/workspace_usage.rs)

script の例外は IntegrationValidation だけで、validating 状態・未取消・同じ予約・Session/Repo・保存済み未完了検証項目の command/cwd を要求する。`next_action` による別の起動も許可しない。検証計画の意味と結果 commit の固定は [正式統合の host 検証](formal-integration.md)で説明する。[script の検証](../../crates/services/src/services/workspace_usage.rs)

既存データの移行は名称から推測しない。Integration/Repository bootstrap の実行記録、または一致する maintenance の保存記録を根拠に分類し、証拠なしは interactive のままにする。過去 Sync の AgentRun が一つに定まらなければ run ID は不明のまま、複数の所有証拠が衝突すれば `conflicting_evidence` として閲覧専用にする。archive・Session・監査・Git 履歴を書き換える移行ではない。[証拠に基づく backfill](../../crates/services/src/services/workspace_usage.rs)

終了後の解除は `cleanup_allowed` に加え、Integration が succeeded / failed / cancelled で Workspace 予約も消えていることを要求する。blocked や recovery_required を通常編集へ解放しない。保存済み owner JSON を compare-and-swap して用途だけを変更する。Wiki の Card link は検証済みの開始元 Project を優先し、未指定なら Repo 所属から Board が一意に決まる時だけ作る。既存 link を上書きせず、定期 monitor で利用者の解除を復元しない。Issue なしの保守を全面禁止する契約ではない。関連検証・表示 status・過去補完の詳しい条件は [開始元 Project と保守 Issue](../operations/openwiki-maintenance.md#開始元-project-と保守-issue)に集約する。[解除と link](../../crates/services/src/services/workspace_usage/lifecycle.rs)。

## Workspace 累積トークン

Workspace 累積トークンは、人や上位 AI が消費の偏りを確認するための、provider が報告した取得済み使用量である。料金・週次枠・[現在の context 占有率](session-and-agent-run.md#context-使用量の意味)や各 run の経過時間を表すものではない。操作権限を表す上記の `usage` とも別概念である。画面の確認方法は [累積値の閲覧](../operations/workspace-inspection.md#累積トークンを確認する)へ進む。[目的と範囲](../../docs/development/workspace-token-usage.md#取得できる範囲と制約)

### 何を取得し、何を未知とするか

対象は既存投影経路で親に属すると判定された Codex native `thread/tokenUsage/updated` の `tokenUsage.total`。thread ID と総量があり、`total = input + output`、cached input ≤ input、reasoning ≤ output が成立するときだけ採用する。cached input は入力、reasoning は出力の内数なので総量へ再加算しない。`last.totalTokens` は context 用であり、総量と内訳が矛盾する context-full 合成通知、汎用 usage、識別情報のない旧 snapshot を集計の証拠にしない。[adapter の検証](../../crates/executors/src/executors/provider_adapter.rs#L1969-L2025)・[親 scope](../../crates/executors/src/executors/codex/agent_scope.rs#L187-L200)

新規テーブルへの過去履歴の backfill はなく、既存 snapshot や監査全体を走査して欠損を埋めない。ただし最初に届いた報告が、その thread の以前の消費を含むことはある。子 thread を独立に計測・加算せず、親報告に含まれない子消費、未対応 provider、未報告の最終分は不明のままにする。[移行](../../crates/db/migrations/20261021173000_workspace_tokens.sql)・[応答の scope](../../crates/db/src/models/workspace_tokens.rs#L60-L70)

### 集計の identity と寿命

保存キーは Workspace・provider・provider session（Codex thread）である。同じ thread の resume が別 Session / RunAttempt にまたがっても、累積 snapshot を足さず、既報 total 以上の報告で置き換える。別 thread は合計する。低い total は無視するため、失敗・取消・再送で既報消費を減らさない。provider が同じ thread の counter を reset しても差分を推定せず、既報最大値を保持する。これは reset 後の真の消費量を保証する方式ではない。[更新条件](../../crates/db/src/models/agent_runtime.rs#L1830-L1860)・[resume と終端状態の試験](../../crates/db/src/models/agent_runtime.rs#L2941-L3058)

採用する後続報告で cached/reasoning が欠損した場合は既報内訳を残す。累積情報のない新しい snapshot に置き換わっても thread 集計は残る。テーブルは Workspace に外部キーで従属し、Workspace の DB 削除で cascade される。保存の原子性・host journal と snapshot の関係は [Runtime の保存境界](../architecture/agent-runtime.md#workspace-累積値の保存境界)にまとめる。[欠損の保持](../../crates/db/src/models/agent_runtime.rs#L1845-L1859)・[valid → None の試験](../../crates/db/src/models/agent_runtime.rs#L3061-L3170)・[寿命](../../crates/db/migrations/20261021173000_workspace_tokens.sql#L1-L14)

### API 値の読み方と採用理由

`WorkspaceTokenUsage::get` が全入口の共通集計である。有効行なしなら総量・内訳は `null`、`completeness=unknown`。有効な 0 の報告は数値 0 で、1 thread 以上あれば常に `partial` とし `complete` を返さない。内訳は取得値だけの和で、cached/reasoning はそれぞれ取得 thread 数も返す。その数は全消費の取得率でも attempt 数でもない。最初の観測時刻は行の最小、最終更新は最大であり、provider の最終消費まで捕捉できた証明ではない。[共通集計](../../crates/db/src/models/workspace_tokens.rs#L42-L70)

`providers` は保存済み報告の provider 一覧、`requested_models` は同 Workspace の attempt request にある要求モデル一覧である。実使用モデルの確定や thread 内モデル別消費の配分には使えない。[一覧の取得](../../crates/db/src/models/workspace_tokens.rs#L55-L70)

[実装記録](../../docs/development/workspace-token-usage.md)は、同一 thread の resume 二重加算を避けること、意味を確認できない通知を推計しないことを設計理由としている。レビュー後には最新の置換可能 snapshot に基づく報告 attempt 数が後退し得るため、その分子・表示専用分母を削除し、everflag や履歴走査を加えなかった。現行 API にもそれらの attempt 数はない。正常 resume の外部実装根拠は記録中の特定 Codex 版についての説明であり、実アカウント精度や異常 reset の保証とは区別する。[レビュー判断](../../docs/development/workspace-token-usage.md#2026-10-21-独立レビュー後の最小修正)・[現行応答型](../../crates/db/src/models/workspace_tokens.rs#L7-L25)。fixture の分担と残る未検証範囲は [トークン集計の検証](../operations/development.md#workspace-トークン集計の検証)を参照する。

## 一覧から作成先を選ぶ

通常の Workspace 入口は既存一覧を表示する。新しい作業を始めるときは「＋」から作成先 chooser を開く。create 画面で再び「＋」を押しても chooser を開き直せ、取消で本文 draft や確定済みの作成先を捨てない。route と再選択要求の伝達は [Web の一覧遷移と作成要求](../architecture/web-and-sync.md#workspace-の一覧遷移と作成要求)にまとめる。[一覧入口](../../packages/web-core/src/pages/workspaces/WorkspacesLanding.tsx#L1-L5)・[選択の適用](../../packages/web-core/src/shared/components/CreateChatBoxContainer.tsx#L210-L250)

chooser では directory を読み込み、作業方式を選び、Worktree なら Repo と target branch を確認してから確定する。パスを編集すると inspection・方式・branch を破棄し、読み込み失敗や処理中には確定できない。branch 一覧にない以前の選択値を復元しない。確認済みかつ有効な target が揃って初めて composer が表示される。これは実際の [作成・Setup・起動](../workflows/task-to-integration.md#1-作業先を作り起動を予約する)に先立つ UI の契約であり、サーバー側の作成検証を代替しない。[derived state の無効化と branch](../../packages/web-core/src/shared/dialogs/shared/WorkspaceTargetDialog.tsx#L107-L151)・[inspection と失敗](../../packages/web-core/src/shared/dialogs/shared/WorkspaceTargetDialog.tsx#L156-L238)・[確定条件](../../packages/web-core/src/shared/dialogs/shared/WorkspaceTargetDialog.tsx#L340-L374)・[composer の表示条件](../../packages/web-core/src/shared/components/CreateChatBoxContainer.tsx#L104-L117)

### サーバーのパスと端末のファイルを区別する

「サーバーを参照」は `ServerFolderPickerDialog` から既存の `/api/filesystem/directory` を読む。ブラウザー端末の file/directory input でも、サーバー OS の native dialog でもない。パス未指定なら server の home を一覧化し、API が返す entries のうち directory だけを表示する。API はドットで始まる子項目を除外するため、一覧に出ないことをパス不存在の証明にしない。パス直接入力も利用できる。[一覧 API の接続](../../packages/web-core/src/shared/lib/api.ts#L958-L969)・[picker](../../packages/web-core/src/shared/dialogs/shared/ServerFolderPickerDialog.tsx#L23-L133)・[一覧生成](../../crates/services/src/services/filesystem.rs#L316-L364)

入力したパスは「パスを読み込む」または Enter で明示的に読み込む。chooser の blur は inspection を起動しないので、参照ボタンへの focus 移動と読込処理を結び付けない。picker は新しい要求時に古い一覧を消し、失敗を表示する。読み込み中、一覧なし、入力値と取得済み `current_path` が不一致の場合は選択不可である。取消は `null` を返し、選択時だけ親 chooser がそのパスを inspect する。picker は effect cleanup の active flag、親は request ID により古い応答の反映を防ぐ。[明示読込](../../packages/web-core/src/shared/dialogs/shared/WorkspaceTargetDialog.tsx#L428-L472)・[一覧の応答と選択](../../packages/web-core/src/shared/dialogs/shared/ServerFolderPickerDialog.tsx#L31-L133)・[親への結果](../../packages/web-core/src/shared/dialogs/shared/WorkspaceTargetDialog.tsx#L274-L307)

directory 一覧と inspection は異なる処理である。inspection はパスを正規化し、存在・directory・可読性を検査する。選択先に `.git` があり Repo を開ける場合は **Repo を登録**して返すので、まだ Workspace を作成していない段階でも登録の副作用がある。chooser を後で取り消すことを、この Repo 登録の rollback と解釈しない。[検証](../../crates/server/src/routes/filesystem.rs#L42-L81)・[登録を伴う inspection](../../crates/server/src/routes/filesystem.rs#L117-L142)

同じ chooser は Project 設定の既定作業先にも使う。そこでは `purpose: project_default` で開き、確定結果を Git の Repo/branch または DirectFolder の path として保存用の値に変換する。参照・読込を変更すると新規 Workspace とこの設定の両方へ影響する。[Project の呼出側](../../packages/web-core/src/shared/dialogs/settings/settings/RemoteProjectsSettingsSection.tsx#L558-L605)。`fileSystemApi.list` は host scope を明示的に受け渡すが、remote host 接続を復活させたわけではなく、[local transport の拒否契約](../architecture/web-and-sync.md#同じ-api-呼び出しでも対象ホストが変わる)に従う。旧 native picker API も別用途の実装として残るため、全 folder picker が置き換わったとは扱わない。[scope の伝達](../../packages/web-core/src/shared/lib/api.ts#L149-L164)・[remote 拒否](../../packages/web-core/src/shared/lib/localApiTransport.ts#L35-L61)・[native picker](../../crates/server/src/routes/filesystem.rs#L83-L115)

変更時の検証入口と、Agent 起動・実 worktree 作成を含まない試験の限界は [Workspace 導線の検証範囲](../architecture/web-and-sync.md#workspace-導線の検証範囲)を参照する。

## 作成・準備の失敗

複数 Repo の worktree は順に作り、途中で失敗した場合はそれまで作った worktree の cleanup を試みて PartialCreation を返す。DB、ファイル作成、添付ファイル取り込み、AgentRun 起動を一つの完全な transaction とみなさない。[部分作成](../../crates/workspace-manager/src/workspace_manager.rs#L545-L606)

Agent 起動前の setup script は Repo 設定に従う。DirectFolder では自動 setup 対象を空にする。全ての setup が parallel 指定なら先行 script と agent を並行起動でき、それ以外は [起動ゲート](../workflows/task-to-integration.md) を経由する。この順序を変えると、依存物が未準備の agent 起動につながる。[setup 分岐](../../crates/server/src/routes/workspaces/create.rs)

## 共有ディレクトリの意味

managed Git Workspace の各 Repo にある `.lvk-shared/persistent/` と `.lvk-shared/cache/` は、**同じ登録 Repo を使う他 Workspace と内容を共有するリンク**である。保存先は既定 storage base の `shared/<sanitized-name>-<full-repo-UUID>/`。worktree の保管先 override とは独立している。[パス生成](../../crates/utils/src/path.rs#L140-L157)、[README の運用契約](../../README.md)

- persistent は利用者が維持するローカルファイル。[Repository Memory](repository-memory.md) の記録もその下に置く。
- cache は再生成できる成果物や cache。ツールごとの cache 設定を LVK が自動変更するものではない。
- Workspace 削除ではリンク先の共有内容を削除しない。自動清掃やバックアップが保証される領域でもない。
- DirectFolder ではこれらのリンクがない場合がある。存在を前提にする前に作業環境を確認する。

[共有の利用契約](../../README.md)、[削除後もデータを保持するテスト](../../crates/workspace-manager/src/shared_resources.rs)

provisioner は tracked な `.lvk-shared`、既存の通常ファイル、異なる宛先のリンクを拒否し、全リンクを検査してから作成する。Git のローカル `info/exclude` に追加し、利用者のパスを上書きしない。Windows のリンク作成には Developer Mode または symlink 権限が必要で、作成失敗は明示エラーになる。[検証・配置](../../crates/workspace-manager/src/shared_resources.rs)、[Windows](../../crates/workspace-manager/src/shared_resources.rs)

provision lock は配置処理を守る。利用者の共有ファイルへの同時書き込みまで直列化するものではない。共有内容を使うツール側が競合を扱う必要がある。[配置 lock](../../crates/workspace-manager/src/shared_resources.rs)、[共有利用時の指示](../../packages/web-core/src/features/pipeline/model/cardContext.ts#L15-L23)

## Archive、削除、期限 cleanup

Archive は、履歴・Git branch を残して managed worktree を解放する明示操作である。UI・stdio MCP・外部 HTTP MCP は共通の prepare → 一回確認「ワークツリーが削除されます」→ confirm を使う。prepare は状態を変更せず、Workspace ID・container path・branch に結びついた10分有効の token を返す。取消は token を送らないことで成立する。confirm は token を消費し、期限切れ・再利用・対象変化を拒否する。通常 update の `archived:true` はこの経路を要求して拒否する。[受付と確認](../../crates/server/src/routes/workspaces/archive.rs#L126-L178)・[通常更新](../../crates/server/src/routes/workspaces/core.rs#L40-L79)

確認前と削除直前に、非終端 AgentRun・process registry・script とその cleanup、Integration 予約と未完了 run、Workflow、資源 operation/mediation を検査する。Wiki は AgentRun が終端でも active maintenance/bootstrap 所有を検査し、host の installer 待ちなど実際の準備 future は RAII 予約で保護する。`execution_only` というラベルだけでは拒否しない。[共通 busy 判定](../../crates/server/src/routes/workspaces/archive.rs#L53-L124)・[準備予約](../../crates/server/src/routes/openwiki/preparation.rs#L1-L41)

実行予約・再生成・archive は Workspace lifecycle lock を共有し、順序は integration MUTATIONS → lifecycle → DB transaction。削除直前の再検査は `BEGIN IMMEDIATE` 内で行う。Linux では未登録 process の exe/cwd/fd/maps/パス引数と LVK 自身の通常 debug frontend 参照も観測する。`developer-runtime` featureで埋込み済みの場合はこの自己frontend参照の保護を省くが、exe/cwd/fd等の実参照検査は維持する。[bundleの独立性](../operations/development.md#開発用成果物切替と復旧)は既存worktreeの無条件削除許可ではない。ただし別 PID namespace、権限で不可視の参照、最終観測後の外部起動は防げない。観測不能な無関係 process だけでは拒否せず、OS 観測全体の失敗も警告して継続する。これは任意の外部 process に対する排他機構ではない。[lock](../../crates/services/src/services/workspace_lifecycle.rs#L1-L33)・[削除](../../crates/server/src/routes/workspaces/archive.rs#L215-L284)・[観測限界](../../crates/server/src/routes/workspaces/archive_processes.rs#L1-L54)

削除は Managed ownership だけを対象にし、External path は保持する。strict cleanup が失敗しても archive を確定し、`worktree_deleted=false` と `deletion_error` を返す。UI は実エラーを表示する。成功した archive は DB 履歴や branch の削除ではなく、既存 archive や orphan の一括移行削除もしない。接続が切れても受付済み confirm は継続する。[結果](../../crates/server/src/routes/workspaces/archive.rs#L265-L290)・[strict cleanup](../../crates/workspace-manager/src/workspace_manager.rs#L694-L713)

### 外部 MCP の確認と結果の回復

外部 MCP は明示した Project / Workspace に対して `prepare_workspace_archive` → 利用者の一回確認 → `confirm_workspace_archive` を行う。両段階で read/write scope と Project・Workspace 所属を検査し、prepare は target・branch・期限・確認メッセージを返す。stdio の入口は引き続き `update_workspace(archived=true)` であり、外部用の tool と同一ではない。[外部 adapter](../../crates/server/src/remote_mcp/workspace_archive.rs#L16-L117)・[stdio adapter](../../crates/mcp/src/task_server/tools/workspaces.rs#L224-L245)

外部 token は issuer/subject の principal と Project に追加で束縛され、UI/stdio の Local scope と相互流用できない。確認時に scope・Workspace・期限が一致した token を消費し、その後 target の path/branch、script、busy 状態を再検査する。異なる呼出元の試行は正当な token を消費しないが、正当な token の消費後に busy や対象変更で拒否された場合は同じ token で再実行できない。Project 所属は最終 DB transaction 内でも照合する。token はメモリー上のみで、server 再起動を越えない。[scope と token](../../crates/server/src/routes/workspaces/archive.rs#L22-L40)・[消費と再検査](../../crates/server/src/routes/workspaces/archive.rs#L215-L307)・[principal identity](../../crates/server/src/remote_mcp/issue_write.rs#L81-L85)

confirm は stable `request_id` と入力を永続 operation に結びつける。応答を失ったら `get_operation` または完全に同じ confirm 要求で結果を回復する。別 ID で削除を再試行しない。receipt の `succeeded` は共通 archive 処理が結果を返した意味で、物理削除の成功とは限らない。`result.archived`、`result.worktree_deleted`、`result.deletion_error` を別々に読み、現在値は `get_workspace` の二つの flag でも確認する。再起動前に未確定だった receipt は `uncertain` となり、自動再実行されない。[receipt への接続](../../crates/server/src/remote_mcp/workspace_archive.rs#L88-L117)・[再送と不確実性](../../crates/server/src/remote_mcp/operations.rs#L208-L245)・[現在値](../../crates/server/src/remote_mcp/runtime_read.rs#L80-L90)。接続時の権限と操作順序は [外部 MCP の archive 導線](../integrations/remote-access.md#workspace-を外部-mcp-から-archive-する)を参照する。

### 履歴と再開

閲覧は archive 済みや execution-only の worktree を再生成しない。削除済みなら diff は残った Git branch の commit を使い、消えた未commit内容は復元しない。unarchive または明示的な実行準備で環境を再生成する。直接の AgentRun 予約は archive/worktree削除済みを拒否する。[受動読取と明示再開](../../crates/services/src/services/container.rs#L705-L744)・[起動の検査](../../crates/local-deployment/src/agent_run_port.rs#L734-L753)・[差分閲覧](../operations/workspace-inspection.md#差分の未取得エラー空を区別する)

旧30分周期・1/72時間閾値と起動時 orphan 回収は撤去された。通常 merge・PR merge 観測・Arena 選択も自動 archive しない。経過時間、pinned、候補敗退を worktree 解放の証拠にしない。[手動操作の利用契約](../../docs/workspaces/managing-workspaces.mdx)・[merge](../../crates/server/src/routes/workspaces/git.rs#L275-L312)・[PR monitor](../../crates/services/src/services/pr_monitor.rs#L125-L159)・[Arena](arena.md)

明示削除の DB transaction は書き込み lock を先に取得し、非終端 AgentRun または spawned/running/unreachable の process registry が残る場合に拒否する。生きた orchestration の関連も保護する。terminal の正本表示だけで実プロセス消滅を証明したことにはならない。[削除 guard](../../crates/workspace-manager/src/workspace_manager.rs#L233-L349)

DB 削除後、ファイル・runtime ファイル・指定時の branch cleanup を background に渡し、API は 202 Accepted を返す。background cleanup の失敗はログに残り得るので、202 は全保存先の物理削除完了を意味しない。External Workspace は削除 context に container path を入れない。[削除 API](../../crates/server/src/routes/workspaces/core.rs)、[所有境界](../../crates/workspace-manager/src/workspace_manager.rs#L174-L205)、[background cleanup](../../crates/workspace-manager/src/workspace_manager.rs#L436-L483)

明示削除は Session の process logs と **Native Audit のディレクトリも削除対象**にする。この処理は container path の有無とは独立しているため、External の作業ファイル保護は監査履歴の保持を意味しない。調査・保全が必要なら削除前に [Native Audit の保存先とエクスポート契約](../architecture/agent-runtime.md#native-audit-の保全エクスポート寿命) を確認する。[削除対象と順序](../../crates/workspace-manager/src/workspace_manager.rs#L436-L515)

変更時は `manual_archive` Rust fixture と `workspace-archive:e2e` を併せて確認する。後者は UI/API/stdio MCP、取消・確認後競合、削除失敗、未登録 process と frontend 使用、履歴/branch 保持、diff と再開を扱う。fixture は実 installer・実 Auth0・非Linux の完全保護を証明しない。[単体入口](../../crates/server/src/routes/workspaces/archive.rs#L568-L933)・[受入入口](../../tests/server/specs/workspace-archive.spec.ts)

利用手順は [Multi-Repo & Sessions](../../docs/workspaces/multi-repo-sessions.mdx)、レビュー・ファイル操作の境界は [Workspace の確認と実行](../operations/workspace-inspection.md)、Git 統合までの順序は [依頼から統合まで](../workflows/task-to-integration.md) を参照。

外部 MCP の回帰 fixture は署名 JWT → Access → HTTP `/mcp` → 共通 archive を通し、scope・member 失効・Project allowlist、呼出元／対象／Local token の取り違え、期限、確認後 busy、branch 変化、同一要求の回復、物理削除失敗と履歴保持を検査する。script-store の liveness は fixture port なので実 script の収束や実 Auth0/ChatGPT 接続を証明しない。この Sync では定義を静的に照合し、製品テストを再実行していない。[HTTP 回帰](../../crates/server/src/remote_mcp/workspace_archive/tests.rs#L1-L611)
