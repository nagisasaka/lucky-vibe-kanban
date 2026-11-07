---
type: guide
title: Issue から実行・レビュー・統合まで
description: 通常作業をまたぐ作成、Setup gate、会話、レビュー、Git/PR、Issue 状態と repository memory の更新順序を説明する。
tags: [workflow, setup, review, git, pull-requests, scheduling]
sources:
  - id: openwiki-source-124913f65e422092e3ffbd4a
    resource: repo://assets/skills/auto-merge/SKILL.md
  - id: openwiki-source-3c57aab40b425fc8b5d0d5c5
    resource: repo://crates/db/migrations/20261027090000_generated_issue_status.sql
  - id: openwiki-source-9bc71fe4f6cbbaa6fa9168e1
    resource: repo://crates/db/src/models/agent_runtime_kanban_tests.rs
  - id: openwiki-source-5c693a62e6ccf09ccb60592f
    resource: repo://crates/db/src/models/agent_runtime_kanban.rs
  - id: openwiki-source-c01fa644be961cb7ee12d2b5
    resource: repo://crates/db/src/models/integration.rs
  - id: openwiki-source-2e4f4c7654ac4a8e4d26283a
    resource: repo://crates/db/src/models/issue_number_tests.rs
  - id: openwiki-source-6fe5fbd6df406f973641b771
    resource: repo://crates/db/src/models/issue_number.rs
  - id: openwiki-source-29ff4fcd8f66b24767de429c
    resource: repo://crates/git-host/src/lib.rs
  - id: openwiki-source-bd15422ef1716eca23408f1f
    resource: repo://crates/git/src/lib.rs
  - id: openwiki-source-209cf31759691c1167d30e1b
    resource: repo://crates/git/tests/git_ops_safety.rs
  - id: openwiki-source-a13fe4db1eee073d0a7e2c4d
    resource: repo://crates/server/src/main.rs
  - id: openwiki-source-3ec71fa8cd8936e31e523736
    resource: repo://crates/server/src/remote_mcp/integration_read.rs
  - id: openwiki-source-b5eef61c63c11d3fe10b3ccf
    resource: repo://crates/server/src/remote_mcp/launch.rs
  - id: openwiki-source-54244fc34a5fffa34c37581d
    resource: repo://crates/server/src/routes/integration_validation.rs
  - id: openwiki-source-595d5d0308d670e38ef27fbf
    resource: repo://crates/server/src/routes/integrations/admission.rs
  - id: openwiki-source-5720d865d1c837f0e6b7dab7
    resource: repo://crates/server/src/routes/integrations/conversation.rs
  - id: openwiki-source-b393e75029374eaf9cba5f7d
    resource: repo://crates/server/src/routes/integrations/runtime.rs
  - id: openwiki-source-24838d7108e2888a7fdc2714
    resource: repo://crates/server/src/routes/local_remote/issue_archive.rs
  - id: openwiki-source-231241ac3c842dcd67525890
    resource: repo://crates/server/src/routes/openwiki.rs
  - id: openwiki-source-391f34050223d13f679b3793
    resource: repo://crates/server/src/routes/parallel_context.rs
  - id: openwiki-source-e2a1cc763db4a100d48eac0a
    resource: repo://crates/server/src/routes/scheduled_tasks.rs
  - id: openwiki-source-2b4c7ad36a9b595e1991b189
    resource: repo://crates/server/src/routes/sessions/review.rs
  - id: openwiki-source-1043187ef1285c84335110a0
    resource: repo://crates/server/src/routes/sessions/setup_gate.rs
  - id: openwiki-source-a517c0f8e44d2cf0c440fbfc
    resource: repo://crates/server/src/routes/workflows.rs
  - id: openwiki-source-bdb4dae5722fe76ec5e44001
    resource: repo://crates/server/src/routes/workspaces/create.rs
  - id: openwiki-source-303d74f74def684d4690dea9
    resource: repo://crates/server/src/routes/workspaces/git.rs
  - id: openwiki-source-27c2433bab843f01a51418b1
    resource: repo://crates/server/src/routes/workspaces/pr.rs
  - id: openwiki-source-5590f47e4cee001a35a42117
    resource: repo://crates/server/src/routes/workspaces/wiki.rs
  - id: openwiki-source-ed8c84278dba8a1f45af40e9
    resource: repo://crates/server/src/startup.rs
  - id: openwiki-source-04d5adb51b5929dacce999bc
    resource: repo://crates/server/src/workflow_runtime/bootstrap.rs
  - id: openwiki-source-72948db7f01925b4b2ff17af
    resource: repo://crates/services/src/services/pr_monitor.rs
  - id: openwiki-source-9c5d335ecd1178e88c47c0a8
    resource: repo://crates/services/src/services/queued_message.rs
  - id: openwiki-source-6067be8d6f85caa8ac03638f
    resource: repo://crates/services/src/services/repository_memory.rs
  - id: openwiki-source-6043692809cbf72f253c5631
    resource: repo://crates/services/src/services/workspace_usage/lifecycle.rs
  - id: openwiki-source-08b3f4dcb1a91c97b6d127f8
    resource: repo://crates/worktree-manager/src/worktree_manager.rs
  - id: openwiki-source-db035046d01e3f24722812c8
    resource: repo://packages/web-core/src/features/kanban/ui/IntegrationPanel.tsx
  - id: openwiki-source-bfe4f914c0604a66edc02213
    resource: repo://packages/web-core/src/i18n/locales/ja/settings.json
  - id: openwiki-source-2fab4d75b959f83ee7aaf538
    resource: repo://packages/web-core/src/shared/dialogs/settings/settings/RepositoryMemorySettings.tsx
  - id: openwiki-source-d6bd59c0727afcf22386a8f5
    resource: repo://packages/web-core/src/shared/dialogs/settings/settings/ReposSettingsSection.tsx
  - id: openwiki-source-ffc55e6e31e6ce477ca57329
    resource: repo://packages/web-core/src/shared/hooks/useWorkflowRun.ts
generated: { by: "codex", at: "2026-11-06T15:24:11.469Z" }
verified:
  - by: openwiki/0.7.2
    at: 2026-11-07T09:46:42.639Z
---

# Issue から実行・レビュー・統合まで

[Issue](../concepts/project-and-issue.md) は作業の要求、[Workspace](../concepts/workspace.md) は変更するファイルの場所、[Session / AgentRun](../concepts/session-and-agent-run.md) は会話と実行を表す。一つの Issue に複数 Workspace や WorkflowAttempt が関わり得るため、「作業が成功した」をどの対象の状態として扱うかを区別する。

## 1. 作業先を作り、起動を予約する

通常の create-and-start 経路は Workspace の作成、Issue link、作業ディレクトリ準備、Session 作成の順に進む。明示された attachment_ids と linked_issue は local の処理で関連づけ、失敗はエラーになる。後段の起動が失敗しても全体が一つの DB/Git transaction で巻き戻るわけではない。再試行時は既存の Workspace/link/Session を先に確認する。[作成と添付](../../crates/server/src/routes/workspaces/create.rs)

Session に渡す local File と作業先のコピーは [Workspace の添付](../operations/workspace-inspection.md#session-の添付)、Board の file 参照は [Issue の共同作業](../concepts/issue-collaboration.md) を参照する。

入力の user description と再利用する context は [Card context と LLM Wiki](../concepts/card-context-and-llm-wiki.md) で区別する。Workflow の graph を実行する場合は、通常 Session の起動と別に [WorkflowAttempt の準備・Run snapshot](../concepts/workflow-attempt.md) を作る。

## 2. Setup の依存を満たす

通常 Workflow では Attempt の row や graph が作られた時点で、実 worktree まで準備済みとは限らない。Agent dispatch で既存 container 準備を行い、Session の所属と 自動マージ 予約を検査する。実行専用の内部環境は owner が準備した root を使い、閲覧や dispatch で勝手に再作成しない。[Workflow の準備境界](../architecture/workflow-runtime.md#作業場所の準備は実-dispatch-の責任)

Repo setup script の扱いは三つに分かれる。

| 条件 | Agent 起動との順序 |
| --- | --- |
| DirectFolder、または setup なし | setup を待たず起動 |
| setup のある Repo がすべて parallel_setup_script | script を開始し、完了を待たず Agent も開始 |
| 一つでも非 parallel | AgentRun を先に予約し、永続 setup gate の成功後に起動 |

全 parallel 経路では script の開始失敗を警告し、Agent の開始へ進む。依存物が必須の setup を「parallel」にすると待機保証がなくなる。[分岐](../../crates/server/src/routes/workspaces/create.rs)

gate は WaitingSetupStart / WaitingSetup / Satisfied / Failed を保存する。setup が Completed かつ exit 0 で、next_action がない最後の action まで終わったときだけ成功とする。Failed / Killed / 非0終了は予約した AgentRun を失敗にする。[成功判定](../../crates/server/src/routes/sessions/setup_gate.rs)・[gate の反映](../../crates/server/src/routes/sessions/setup_gate.rs)

completion event を取りこぼした watcher は DB の gate から追いつく。逐次 chain の途中成功では起動しないことと、失敗時に進めないことには focused test がある。[回復](../../crates/server/src/routes/sessions/setup_gate.rs)・[tests](../../crates/server/src/routes/sessions/setup_gate.rs)

## 3. 会話を続け、成果物を確認する

実行中の approval/input、steer、cancel、terminal 後の retry は [Session と AgentRun](../concepts/session-and-agent-run.md) の契約に従う。同じ Workspace に別 Session を作ってもファイルは共有される。follow-up は Session ごとのメモリー内 FIFO であり、失敗後は一時停止して明示再開を待つ。再起動で残る永続 task queue ではない。詳細と成功後の source checkpoint は同概念ページを参照する。

差分・実ファイル・動作確認には [Workspace inspection](../operations/workspace-inspection.md) を使う。人の inline comment から Agent に修正を依頼する操作は [差分レビューの操作](../operations/workspace-inspection.md#差分レビューと外部アプリの確認) にまとまっている。

Agent の review API は Workspace 内に active AgentRun があれば拒否する。use_all_workspace_commits は Repo ごとの fork point を review context に使い、intent=Review、selected_skills なしで起動する。OpenWiki の independent Review と同じ隔離プロトコルではない。[review の起動](../../crates/server/src/routes/sessions/review.rs)

通常の作業 Card は Workspace 作成・予約だけでは現在列を変えず、確認された新しい実開始で対象の標準列から In Progress へ進む。成功後も関連 run が活動中なら待ち、条件が揃えば In Review になる。In Review や Done の Card も追加実行の実開始で再び In Progress へ戻る。Starting や開始前の失敗だけでは Done を戻さない。列が進まない場合は [開始根拠・手動優先・複数 Workspace の同期条件](../concepts/project-and-issue.md#kanban-と実行の同期契約)と AgentRun の失敗表示を確認する。In Review は成果物の検証や Git 公開の証拠ではなく、自動マージ の公開後 Done は別段階である。[列同期](../../crates/db/src/models/agent_runtime_kanban.rs#L13-L109)・[公開後の Done](../../crates/db/src/models/integration.rs#L541-L610)

生成された自動マージ・Wiki 保守 Card は製品実行の状態を表すため、通常 Card の開始順同期から除外する。実行・再開は In Progress、停止・取消・判断待ちは In Review、公開確認後は Done となる。Agent の正常終了だけで公開完了と判断せず、[生成 Issue の状態契約](../concepts/project-and-issue.md#生成-issue-の状態契約)で各製品の根拠と手動介入の保護を確認する。[状態投影](../../crates/db/migrations/20261027090000_generated_issue_status.sql#L8-L55)

## 並列変更を試してから正式統合する

他 Card の文脈や branch を調べるときは、[Shared dirs と並列 context](../concepts/card-context-and-llm-wiki.md)で関係を発見し、採用する commit を固定する。発見された最新 HEAD や他 Workspace の未統合の私的文脈 を、既に統合済みの仕様として扱わない。

組合せ試験には `POST /api/workspaces/<id>/integration/preview` がある。自分の base commit と peer の commit を明示し、Repo membership、各 commit がその Workspace branch に帰属する祖先であること、自分の checkout が指定 base に一致して clean なことを確認する。古い固定 commit は許すが、自動的に新 HEAD へ追従しない。自分を含む source は最大100件である。[preview の受付](../../crates/server/src/routes/parallel_context.rs)

API が作るのは base からの detached worktree と固定 peer 一覧であり、peer の merge や tests を自動実行する API ではない。返す指示に従ってその試行先で組み合わせを検証し、元 Workspace の reset/stash、peer の修正、target/Card 更新、Wiki 執筆を持ち込まない。試行は通常 Workspace cleanup の外側の sibling directory に残し、調査後に明示的な worktree cleanup を行う。自動マージ成功の証拠にはならない。[試行の用途](../../crates/server/src/routes/parallel_context.rs)、[保持と source ref 不変の test](../../crates/worktree-manager/src/worktree_manager.rs)

検証コマンドを変更する場合は、先に Settings → Repositories の「自動マージの検証」を開き、表示された登録 checkout の path・branch・HEAD を確認する。順序付きの複数行コマンドを編集して保存し、読戻し成功を確認する。不正な既存 JSON は修復欄から直せる。保存は未コミットのファイル更新なので、差分を確認して `.lvk/integration-validation.json` を通常の Git 手順で目的の target branch に反映してから、新規 自動マージ を開始する。保存だけではコマンドを実行せず、既存 run にも反映しない。[UI の反映案内](../../packages/web-core/src/i18n/locales/ja/settings.json#L3-L19)・[受付の固定](../../crates/server/src/routes/integrations/admission.rs#L242-L262)。競合時の下書き保持、再読込、外部編集との排他限界は [検証コマンドの編集と反映](../concepts/formal-integration.md#検証コマンドの編集と反映)を確認する。

local Boardの **自動マージ** では、Repo・local target・各Cardの採用WorkspaceとCode設定を選び、同じrequest ID/入力で再送する。受付はsource SHA、base SHA、要件、検証方針を固定する。開始時のtarget変更は停止条件であり、後から受付baseを直接書き換えない。[受付](../../crates/server/src/routes/integrations/admission.rs)・[選択UI](../../packages/web-core/src/features/kanban/ui/IntegrationPanel.tsx)。

Wiki有効時はコード検証後、最初のbegin前に `prepare_wiki(expected_commit=code SHA)` の成功を確認する。同じ候補でWikiのみ別commitにし、生成AGENTS/CLAUDE差分は未stageで残す。code/Wiki/final SHAを固定してpublish要求後にRunを終了し、hostがwriter収束・完了proof・原本からの限定復元・clean・公開guardを確認する。復元途中のprepare拒否をbegin許可と解釈しない。[準備と回復の手順](../operations/openwiki-maintenance.md#自動マージ担当が更新する場合)・[実行契約](../../assets/skills/auto-merge/SKILL.md#L10-L11)

担当は専用Issue / Workspace / Sessionの一つの会話で、候補作成→必要な検証または根拠付き省略→有効時のWiki更新→公開要求まで進める。質問、追加指示、失敗後の修復も同じ会話で続け、旧host merge / 競合だけの有限job / gate承認方式を使わない。既存コード検証を再利用する場合はtested SHAと新候補との差分・理由・残リスクを記録し、失敗や未実行をpassへ変更しない。[担当契約](../../crates/server/src/routes/integrations/runtime.rs)・[同じ会話の進行](../../crates/server/src/routes/integrations/conversation.rs)。

Wiki有効時はコード検証後の同じ候補branchで正規OpenWiki lifecycleを完了し、Wikiだけを別commitにする。コード検証SHAとWiki/公開SHAを区別し、Wiki失敗なら公開しない。専用Wiki Issue/Workspaceへ分岐しない。[Wiki保守](../operations/openwiki-maintenance.md#自動マージ担当が更新する場合)。

外部MCPは `start_integration` の `skip_validation` で追加検証方針を渡し、receiptと `get_integration` の保存値を確認する。summaryのconversationとvalidation sectionから割当先・候補・コードSHAごとの証拠を読み、質問や指示はそのSessionへ送る。旧 `pending_gates` / `approve_integration` はない。中止は `cancel_integration` で要求し、受付と `summary.lifecycle.cancellation_complete` による予約解放を区別する。[外部MCP](../integrations/remote-access.md#mcp-から-integration-の会話と取消を扱う)。

公開要求後は担当Runを終了し、hostがwriter収束、clean候補、固定base/sourceと現在mainの祖先性、Wiki証拠を検査してCAS公開する。mainが進んだ場合も、その変更を含む安全な候補が必要で、祖先性や競合を無視して公開しない。公開要求の受理はmain反映ではなく、Agent正常終了だけでもDoneではない。停止中の候補と証拠は保持し、baseや状態のDB直書きでguardを回避しない。[公開条件・Done・archive](../concepts/formal-integration.md)。

旧方式runは現行会話へ暗黙変換せず停止理由を確認する。通常のGit/PR操作は以下の別経路であり、自動マージの公開要求に代用しない。

## 4. Rebase と direct merge

Git 操作は Repo 単位で target_branch を持つ。Rebase API は新 branch の存在確認後に WorkspaceRepo の target を更新し、それから Git rebase を行う。conflict は file と operation を持つ構造化エラーになり、continue / abort の経路がある。エラー時には、Git 状態だけでなく更新済み target 設定も確認する。[Rebase の順序](../../crates/server/src/routes/workspaces/git.rs)・[操作 router](../../crates/server/src/routes/workspaces/git.rs)

Git safety tests は通常の untracked file の保存、tracked dirty change の拒否、base に上書きされる untracked file の保存を検証する。conflict を「とりあえず reset」で消すことを前提にしない。[Rebase tests](../../crates/git/tests/git_ops_safety.rs)

手動 Direct merge は自動マージ とは別の入口で、その Repo に open PR がある場合と、target が remote branch の場合に拒否する。実際の統合は squash であり、base が先行していれば拒否する。base が別 checkout にあればその場所で CLI merge、checkout がなければ libgit2 の ref 操作を使う。統合後は task branch ref も新 squash commit へ更新し、続きの作業をそこから行えるようにする。[API の制約](../../crates/server/src/routes/workspaces/git.rs)・[Git の統合](../../crates/git/src/lib.rs)

新しい squash commit の identity は、CLI 経路では base checkout、libgit2 経路では task worktree から解決する。rebase は元 author を保持して committer を解決する。identity エラー時は利用者が設定してから再試行し、agent が名前やメールを推測しない。[Repo の設定・legacy 承認・再試行](../concepts/workspace.md#git-commit-identity)・[両 squash 経路](../../crates/git/src/lib.rs#L780-L830)・[rebase guard](../../crates/git/src/lib.rs#L1338-L1343)

checkout 済み base の staged changes は拒否する。専用 test は staged file を残したまま merge が失敗することを検証する。[test](../../crates/git/tests/git_ops_safety.rs)

Wiki有効化と独立しては source checkpoint と明示 event の integration 記録を準備してから Git を実行し、成功した commit を保存する。その後で local Merge record を保存する。Workspace は自動 archive しない。Git 成功と後段の記録保存は一つの transaction ではない。[統合順序](../../crates/server/src/routes/workspaces/git.rs)

## 5. Push と Pull Request

Push と PR 作成は、外部公開より前に未完了の source checkpoint を処理する。PR 作成は push remote と base remote を解決し、base branch の存在を検証、source を準備、branch を push、その後 GitHost API で PR を作る。push 後の PR 作成が失敗した場合も、branch の push は既に済み得る。[Push](../../crates/server/src/routes/workspaces/git.rs)・[PR 作成](../../crates/server/src/routes/workspaces/pr.rs)

push は既定 remote の push URL を尊重し、PR base は target branch の remote、解決できなければ既定 fetch remote を使う。fork へ push する設定だけで PR base も fork に変わるわけではない。GitHub の登録 PAT と system 認証の選択、接続確認の限界は [GitHub 認証と push/base の契約](../integrations/github-review.md)を確認する。[remote 解決](../../crates/server/src/routes/workspaces/pr.rs#L236-L254)

GitHostService の現行 provider は GitHub と Azure DevOps。未知 URL は UnsupportedProvider である。[provider 境界](../../crates/git-host/src/lib.rs) provider 固有の CLI 認証や使い方は [GitHub認証とPR](../integrations/github-review.md) と [Azure CLIの準備](#azure-repos-で-pr-を作る前に) を参照する。

PR の外部作成成功後、local PR record の保存エラーはログに残し、ブラウザー起動と任意の description follow-up の失敗は警告して成功応答を返す。「PR が作成された」と「全ての付随処理が成功した」を区別して診断する。[作成後の部分失敗](../../crates/server/src/routes/workspaces/pr.rs)

PR の状態観測と archive の契約は [GitHub と PR](../integrations/github-review.md) に集約する。旧 Cloud Review の webhook / worker は現行配布にない。

## 6. Issue の完了と Workspace archive

自動マージ は Git 公開を確認してから完了結果を保存する。採用元の選択 Card は受付時の title/revision 一致を条件にせず、利用可能な可視列（Done 優先、なければ末尾）へ進め、他 run の予約等の未更新理由も記録する。新規専用 自動マージ Card は公開結果の投影で可視 Done を要求し、列欠落時は代替しない。後処理は専用 Card の Cancelled や公開後の手動再オープンを上書きしない。詳細は [Git・Done・Wiki の独立した結果](../concepts/formal-integration.md)を参照する。

Direct merge の成功と PR の merged 観測では、通常の作業 Workspace を自動 archive しない。PR monitor は状態と merge commit を記録し、open PR が0になった場合の analytics を更新する。[Direct merge 後](../../crates/server/src/routes/workspaces/git.rs)・[PR 観測](../../crates/services/src/services/pr_monitor.rs)

作業領域を解放する場合は [手動 archive の確認と失敗条件](../concepts/workspace.md#archive削除期限-cleanup)へ進む。未 commit の作業も削除対象になるため、統合成功だけで削除可否を推定しない。Card の Done、履歴の保持、物理 worktree の削除は別の結果である。

新規の自動マージ は、公開成功と予約解放、process 収束後の owner 解除時に、内部生成された専用 Workspace だけを一度 archive 試行する。採用元 Workspace は保持し、専用 Issue も他の未 archive Workspace がある間は残す。Cancelled や公開後に手動再オープンした生成 Issue は自動 archive を拒否する。成功・Done・物理削除は別の結果なので、archive 拒否や削除失敗を再統合の理由にしない。[一度限りの資格と観測](../concepts/workspace.md#生成された専用-workspace-の自動-archive)・[自動マージ の成功と後処理](../concepts/formal-integration.md#成功後の専用作業領域)

## 7. 統合後に canonical memory を更新する

自動マージでは同じ担当がコード検証後の候補でWikiを更新し、Wikiだけを別commitにして公開要求する。公開後イベントやbacklogから別Wiki Workspace/Agentは起動しない。mainを直接変更した場合も自動同期せず、必要に応じて手動更新・再同期を明示的に開始する。再起動と定期回復は保存済みownerを回収する。[受付と回復の境界](../operations/openwiki-maintenance.md#開始条件と通常-sync)・[現行の回復monitor](../../crates/server/src/routes/openwiki.rs#L522-L550)

通常成功coding RunはWiki有効化と独立してsemantic draftとGit証拠からファイルChange Manifestを作る。Wiki有効時は統合されたeventをmaintenanceの入力にする。source merge の成功と Wiki 更新の成功は別であり、Wiki 失敗は source を巻き戻さない。

LVK 自身の手動 direct squash と自動マージ はそれぞれ明示的な event ID を記録できる。外部 PR の squash/rebase は元 commit の ancestry を証明できない場合があり、Workspace ID や時刻だけで event を消費しない。統合先 local branch を準備し、必要なら [Sync Wiki](../operations/openwiki-maintenance.md) で実 source を再確認する。[外部mergeの観測](../../crates/server/src/routes/openwiki.rs#L674-L700)・[sourceとWikiの統合契約](../concepts/repository-memory.md#source-統合と-wiki-反映を分ける)

Project 設定から手動同期すると、開始元 Project が保守 Issue の紐付け先として渡される。Repo 単独設定からの明示開始では Issue なしの互換経路もあるため、Card の有無だけで起動失敗と判断せず maintenance の状態を確認する。選択先の検証と status 不在時の対処は [開始元 Project と保守 Issue](../operations/openwiki-maintenance.md#開始元-project-と保守-issue)を参照する。[共通受付](../../crates/server/src/routes/openwiki.rs#L170-L202)・[保守Issueの所属](../operations/openwiki-maintenance.md#開始元-project-と保守-issue)

Wiki 保守 Card も Agent 終了と host 公開を分けて読み、検証済み no-op を含む公開成功で Done になる。成功した新規内部生成 Wiki 保守環境も、公開・owner 解放後に専用 Workspace と条件を満たす専用 Issue が archive され得る。未公開・失敗・取消の残存や手動で復元した環境は自動再試行で消さない。[保守環境の解放](../operations/openwiki-maintenance.md#成功後の保守環境の解放)を参照し、canonical branch と実行履歴で成果を確認する。

生成物は [Wiki Viewer](../operations/workspace-inspection.md#wiki-の本文ツリーと表示元を確認する)で確認できる。ただし Viewer は選択 Workspace の現在のファイルを読むため、maintenance の未公開成果物も見える。Wiki 本文を開けたこと、Agent が完了を宣言したこと、Workflow と target branch への publication が成功したことを分けて確認する。[表示元の実装](../../crates/server/src/routes/workspaces/wiki.rs)、[publication の検証](../../crates/server/src/workflow_runtime/bootstrap.rs)

## 定時に Workflow を起動する

ScheduledTask は既存 Workflow template を日次・週次で起動する設定である。現行 target は Workflow のみ、concurrency policy は SkipIfRunning のみ。時刻になった task を期限付き DB claim で取得する。[型](../../crates/server/src/routes/scheduled_tasks.rs)・[claim](../../crates/server/src/routes/scheduled_tasks.rs)

毎回 template の Agent node の session_id を消した graph から新しい WorkflowAttempt を作る。実行は [通常の Workflow lifecycle](../concepts/workflow-attempt.md) に入り、trigger_source は schedule / schedule_manual を使う。[起動](../../crates/server/src/routes/scheduled_tasks.rs)・[Session binding の初期化](../../crates/server/src/routes/scheduled_tasks.rs)

前回の last_run_id が pending / running / awaiting_human / awaiting_arena なら今回は skipped とし次回時刻へ進める。人や Arena の待ち時間も「前回が active」に含まれ、同じ slot を無期限 queue に積む契約ではない。[skip](../../crates/server/src/routes/scheduled_tasks.rs)・[active の範囲](../../crates/server/src/routes/scheduled_tasks.rs)

時刻計算は IANA timezone を用い、daily/weekly と DST の focused tests がある。server のローカル timezone や固定 UTC offset だけで schedule を再実装しない。[tests](../../crates/server/src/routes/scheduled_tasks.rs)

定時 task の起動と、その後に Agent 完了を検知して graph を進めることも別である。この checkout では completion watcher は embedded backend の startup で登録され、standalone main には登録がない。GET/SSE は reconciliation を行い、UI は active run を既定 4 秒で poll する。無人の standalone 運用で予定通り起動したことだけを根拠に、後続段階まで自動進行すると判断しない。[起動形態と前進の契機](../architecture/workflow-runtime.md#起動形態と前進の契機) に登録箇所・read 側の処理・文書の自動連鎖という意図との差をまとめる。[embedded 登録](../../crates/server/src/startup.rs)、[standalone 入口](../../crates/server/src/main.rs)、[poll](../../packages/web-core/src/shared/hooks/useWorkflowRun.ts)

## 仕事をarchiveし、必要なら再開する

IssueのActive/Archived/Allで仕事を探し、Kanban/Listは独立して選ぶ。Issueをarchiveする際、配下Workspaceも解放するなら既定ONの確認で全対象を確認する。使用中の対象があれば全体を拒否するので、停止・cleanupが収束してから準備し直すか、checkboxをOFFにしてIssueだけをarchiveする。Issue解除はWorkspaceを復元せず、Workspaceの明示再開とは別操作である。archive中に既存実施は継続できるが、新しい実施を作るには先にIssueを解除する。[属性と失敗時の正本](../concepts/project-and-issue.md#issue-のアーカイブ)・[Workspaceの解放](../concepts/workspace.md#archive削除期限-cleanup)。

Issueの表示番号は作成時に固定したProject keyを使い、priority/tagは仕事の分類として設定する。systemはLVK生成Cardの分類であり、採用・実行・公開を承認するものではない。[識別と分類](../concepts/project-and-issue.md#仕事の識別と分類)。

通常作成と生成 Card は Project UUID ごとの永続高水位を共有し、commit 済み番号を削除後に再利用しない。移行前に消えた番号の履歴は自動復元せず、初期 floor の確認と rollback 可能な未公開予約の区別は [採番の寿命と移行の限界](../concepts/project-and-issue.md#採番の寿命と移行の限界)を参照する。[allocator](../../crates/db/src/models/issue_number.rs#L23-L46)・[履歴不足のmigration回帰](../../crates/db/src/models/issue_number_tests.rs#L176-L222)

## 調査modeと素材、Wiki再開を引き継ぐ

実装前の調査・レビューは、対応するCodexのREVIEWを選び、対象・範囲・期待する報告を指定して会話への報告後に停止させる。Workspace内の必要な再現・テストは許すが、所見から製品実装や採用へ自動移行しない。実装には別の明示指示を必要とする。[REVIEWの実行境界](../integrations/agent-providers.md#review-の実行境界)

素材はMCPの保存と配置を分け、ID・サイズ・SHA-256を確認してから初回/続行にattachment_idsを渡す。保存受付は配置や実行受付の成功ではない。再送で取得元が失効した場合は元request_idのreceiptを読む。実ChatGPT/Libraryや実Codexの適合確認は隔離fixture成功と分ける。[MCP搬送](../integrations/remote-access.md#mcpで素材ファイルを渡す)

自動マージのWikiがRun途中で止まった場合は、同じ会話を明示再開してOpenWikiの正規beginを使う。所有済みactive ID/modeの連続監査がある場合だけ後続writerのresumeを認め、別依頼や欠損証拠を引き継がない。standalone保守の閉鎖条件とは別であり、仕組みの詳細は [同じ会話でWiki更新を再開する](../concepts/formal-integration.md#同じ会話でwiki更新を再開する)を参照する。

## Azure Repos で PR を作る前に

LVKを実行するaccountの環境へAzure CLIを用意し、`az extension add --name azure-devops` と `az login` を行う。必要に応じて組織・Projectの既定値を設定し、対象remoteと同じ権限でアクセスできることを確認する。LVKは `az repos` を実行し、CLI欠落・認証失敗・対象不存在を区別する。[Azure wrapper](../../crates/git-host/src/azure/cli.rs)

WorkspaceのPR作成画面で対象Repo、head/base、title・descriptionを確認して作成する。URLが返ってもlocal記録まで成功したかを確認し、応答不明時は同branchの既存PRを調べてから再試行する。GitHub用PATの保存をAzure認証に流用しない。[共通PR経路](../../crates/server/src/routes/workspaces/pr.rs)


自動マージpanelを開いている間、統合一覧は5秒、repository activitiesは10秒周期で取得する。初回取得・操作後の無効化と固定requestによる再送は維持する。[panel](../../packages/web-core/src/features/kanban/ui/IntegrationPanel.tsx#L100-L180)


### LOCAL-332のRun成果と運用状態

通常成功coding Runのsource commitとファイルManifestはWiki無効でも確定する。既存HEADを再利用し、変更なしRunには空Manifest・余分なcommit・専用検証記録を作らない。検証報告は通常Runの最終報告で参照する。上位AIはIssue化や既存Sessionへの追加指示に確認済みの動機・制約・判断理由を含め、coding agentは人間の意図と実装判断を区別してsemantic draftへ残す。承認は技術的理由の代用にしない。[Manifestの契約](../concepts/repository-memory.md)

source checkpoint / integration準備、Git、integration結果、local Mergeは一transactionではない。Wiki receipt・state・publicationとsource integration対応はinstance DBへ集約し、Gitの実SHA/treeを照合する回復を維持する。live移行と旧共有ファイル削除はコード統合と別の実行許可が必要である。[保存先と一度限り移行](../concepts/repository-memory.md#運用状態のdb集約と一度限りの開発移行)・[直接merge](../../crates/server/src/routes/workspaces/git.rs#L223-L310)
