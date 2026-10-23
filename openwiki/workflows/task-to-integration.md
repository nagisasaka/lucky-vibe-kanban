---
type: guide
title: Issue から実行・レビュー・統合まで
description: 通常作業をまたぐ作成、Setup gate、会話、レビュー、Git/PR、Issue 状態と repository memory の更新順序を説明する。
tags: [workflow, setup, review, git, pull-requests, scheduling]
sources:
  - id: openwiki-source-c01fa644be961cb7ee12d2b5
    resource: repo://crates/db/src/models/integration.rs
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
  - id: openwiki-source-1083b3cf6ec502f1c6eb3811
    resource: repo://crates/server/src/remote_mcp/integration_write.rs
  - id: openwiki-source-b5eef61c63c11d3fe10b3ccf
    resource: repo://crates/server/src/remote_mcp/launch.rs
  - id: openwiki-source-54244fc34a5fffa34c37581d
    resource: repo://crates/server/src/routes/integration_validation.rs
  - id: openwiki-source-595d5d0308d670e38ef27fbf
    resource: repo://crates/server/src/routes/integrations/admission.rs
  - id: openwiki-source-a1c709eaad59af16183269df
    resource: repo://crates/server/src/routes/integrations/legacy_checkpoint.rs
  - id: openwiki-source-864e2ae97802aa9a00527c40
    resource: repo://crates/server/src/routes/integrations/mechanical.rs
  - id: openwiki-source-b393e75029374eaf9cba5f7d
    resource: repo://crates/server/src/routes/integrations/runtime.rs
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
  - id: openwiki-source-08b3f4dcb1a91c97b6d127f8
    resource: repo://crates/worktree-manager/src/worktree_manager.rs
  - id: openwiki-source-a3944588bb3b08c863d66298
    resource: repo://docs/workspaces/openwiki.mdx
  - id: openwiki-source-db035046d01e3f24722812c8
    resource: repo://packages/web-core/src/features/kanban/ui/IntegrationPanel.tsx
  - id: openwiki-source-a836651555f6dfa0a9ab8ce3
    resource: repo://packages/web-core/src/features/workspace-chat/ui/IntegrationInterventionPanel.tsx
  - id: openwiki-source-06868af84d952b8087cf3ebb
    resource: repo://packages/web-core/src/i18n/locales/ja/common.json
  - id: openwiki-source-bfe4f914c0604a66edc02213
    resource: repo://packages/web-core/src/i18n/locales/ja/settings.json
  - id: openwiki-source-d6bd59c0727afcf22386a8f5
    resource: repo://packages/web-core/src/shared/dialogs/settings/settings/ReposSettingsSection.tsx
  - id: openwiki-source-ffc55e6e31e6ce477ca57329
    resource: repo://packages/web-core/src/shared/hooks/useWorkflowRun.ts
generated: { by: "codex", at: "2026-10-23T23:50:43.997Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-23T23:50:43.997Z
---

# Issue から実行・レビュー・統合まで

[Issue](../concepts/project-and-issue.md) は作業の要求、[Workspace](../concepts/workspace.md) は変更するファイルの場所、[Session / AgentRun](../concepts/session-and-agent-run.md) は会話と実行を表す。一つの Issue に複数 Workspace や WorkflowAttempt が関わり得るため、「作業が成功した」をどの対象の状態として扱うかを区別する。

## 1. 作業先を作り、起動を予約する

通常の create-and-start 経路は Workspace の作成、Issue link、作業ディレクトリ準備、Session 作成の順に進む。明示された attachment_ids と linked_issue は local の処理で関連づけ、失敗はエラーになる。後段の起動が失敗しても全体が一つの DB/Git transaction で巻き戻るわけではない。再試行時は既存の Workspace/link/Session を先に確認する。[作成と添付](../../crates/server/src/routes/workspaces/create.rs)

Session に渡す local File と作業先のコピーは [Workspace の添付](../operations/workspace-inspection.md#session-の添付)、Board の file 参照は [Issue の共同作業](../concepts/issue-collaboration.md) を参照する。

入力の user description と再利用する context は [Card context と LLM Wiki](../concepts/card-context-and-llm-wiki.md) で区別する。Workflow の graph を実行する場合は、通常 Session の起動と別に [WorkflowAttempt の準備・Run snapshot](../concepts/workflow-attempt.md) を作る。

## 2. Setup の依存を満たす

通常 Workflow では Attempt の row や graph が作られた時点で、実 worktree まで準備済みとは限らない。Agent dispatch で既存 container 準備を行い、Session の所属と Integration 予約を検査する。実行専用の内部環境は owner が準備した root を使い、閲覧や dispatch で勝手に再作成しない。[Workflow の準備境界](../architecture/workflow-runtime.md#作業場所の準備は実-dispatch-の責任)

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

差分・実ファイル・動作確認には [Workspace inspection](../operations/workspace-inspection.md) を使う。人の inline comment から Agent に修正を依頼する操作は [Reviewing Code](../../docs/reviewing-code.mdx) にまとまっている。

Agent の review API は Workspace 内に active AgentRun があれば拒否する。use_all_workspace_commits は Repo ごとの fork point を review context に使い、intent=Review、selected_skills なしで起動する。OpenWiki の independent Review と同じ隔離プロトコルではない。[review の起動](../../crates/server/src/routes/sessions/review.rs)

関連 Card は Workspace 作成・予約では未着手のままで、確認された実開始で In Progress へ進む。成功後も関連 run が活動中なら待ち、条件が揃えば In Review になる。追加実行では再び In Progress へ戻る。列が進まない場合は [開始根拠・手動優先・複数 Workspace の同期条件](../concepts/project-and-issue.md#kanban-と実行の同期契約)と AgentRun の失敗表示を確認する。In Review は成果物の検証や Git 公開の証拠ではなく、正式 Integration の公開後 Done は別段階である。[列同期](../../crates/db/src/models/agent_runtime_kanban.rs#L13-L109)・[公開後の Done](../../crates/db/src/models/integration.rs#L541-L610)

## 並列変更を試してから正式統合する

他 Card の文脈や branch を調べるときは、[Shared dirs と並列 context](../concepts/card-context-and-llm-wiki.md)で関係を発見し、採用する commit を固定する。発見された最新 HEAD や他 Workspace の未統合 Memory を、既に統合済みの仕様として扱わない。

組合せ試験には `POST /api/workspaces/<id>/integration/preview` がある。自分の base commit と peer の commit を明示し、Repo membership、各 commit がその Workspace branch に帰属する祖先であること、自分の checkout が指定 base に一致して clean なことを確認する。古い固定 commit は許すが、自動的に新 HEAD へ追従しない。自分を含む source は最大100件である。[preview の受付](../../crates/server/src/routes/parallel_context.rs)

API が作るのは base からの detached worktree と固定 peer 一覧であり、peer の merge や tests を自動実行する API ではない。返す指示に従ってその試行先で組み合わせを検証し、元 Workspace の reset/stash、peer の修正、target/Card 更新、Wiki 執筆を持ち込まない。試行は通常 Workspace cleanup の外側の sibling directory に残し、調査後に明示的な worktree cleanup を行う。正式統合成功の証拠にはならない。[試行の用途](../../crates/server/src/routes/parallel_context.rs)、[保持と source ref 不変の test](../../crates/worktree-manager/src/worktree_manager.rs)

検証コマンドを変更する場合は、先に Settings → Repositories の「正式Integrationの検証」を開き、表示された登録 checkout の path・branch・HEAD を確認する。順序付きの複数行コマンドを編集して保存し、読戻し成功を確認する。不正な既存 JSON は修復欄から直せる。保存は未コミットのファイル更新なので、差分を確認して `.lvk/integration-validation.json` を通常の Git 手順で目的の target branch に反映してから、新規 Integration を開始する。保存だけではコマンドを実行せず、既存 run にも反映しない。[UI の反映案内](../../packages/web-core/src/i18n/locales/ja/settings.json#L3-L19)・[受付の固定](../../crates/server/src/routes/integrations/admission.rs#L242-L262)。競合時の下書き保持、再読込、外部編集との排他限界は [検証コマンドの編集と反映](../concepts/formal-integration.md#検証コマンドの編集と反映)を確認する。

local Board の **Integrate / Auto Merge** では、Repo・local target と各 Card の採用 Workspace を選び、Code 実行として送信する。通信再試行は同じ request ID と入力を保持する。source と観測 target は受付時に固定する。queue 先頭で preparation を始めるときも同じ target であることを要求し、前進していれば停止する。実行用 Workspace の履歴で進行・検証・失敗を確認する。[Board の選択と送信](../../packages/web-core/src/features/kanban/ui/IntegrationPanel.tsx)

この [正式 Integration](../concepts/formal-integration.md)は、選択要件と固定 source を保存し、専用環境で受付時の検証方針に従って処理した結果 commit を local target へ公開する。Board の Skip validation は保存される選好であり、run の方針は受付時に固定する。省略時の NotRun はテスト成功ではない。[検証方針の契約](../concepts/formal-integration.md#検証方針は受付時に固定する)を確認する。Git 公開、条件付き Card Done、後続 Wiki receipt は別の結果なので個別に確認する。取消できる phase と recovery_required の扱いも所有フローに従う。詳細な受付・検証・公開契約は同ページに集約し、以下では既存の手動 Git/PR 経路を説明する。

外部会話からは `start_integration` の `skip_validation` で同じ検証方針を選び、受付 receipt と `get_integration` で保存済みの選択を確認できる。再接続時の入力一致と結果の読み分けは [MCP の検証方針・再送契約](../integrations/remote-access.md#mcp-から-integration-の検証方針を選ぶ)を参照する。

外部会話で判断を続ける場合は、`get_integration(section="pending_gates")` の保存説明・失敗証拠と対象候補を読み、明示承認時にだけ `approve_integration` を送る。中止は `cancel_integration` で要求し、受付 receipt と `summary.lifecycle.cancellation_complete` を分けて確認する。固定証拠の渡し方と古い候補の拒否、取消後の予約解放は [MCP の承認・取消契約](../integrations/remote-access.md#mcp-から-integration-を承認取消する)を参照する。[公開操作](../../crates/server/src/remote_mcp/integration_write.rs#L17-L79)

新規 run の通常 merge は host が進める。競合したときだけ有限の Agent job が index を解消し、利用者は保存された候補の解決差分を承認する。その後に次の source へ進み、全 source 完了後の最終候補で凍結 validation plan を実行する。失敗した required check は retry / 候補限定例外 / cancel の判断を待つ。[競合と承認](../concepts/formal-integration.md#競合だけを有限の-agent-job-にする)・[最終検証](../concepts/formal-integration.md#開発検証と-human-gate)

Board の案内も現在は「順に取り込み、最後の候補を検証する」という流れを説明する。競合で止まったら実行 Workspace を開き、保存済みの解消説明と未検証・古い候補の表示を読み、必要に応じて詳細の差分を開く。説明が不十分なら承認せず確認する。固定候補のレビュー中に追加指示で修正する経路はなく、承認して続行するか Integration を中止する。[説明・操作条件の正本](../concepts/formal-integration.md#保存済み説明から候補をレビューする)・[Board の案内](../../packages/web-core/src/i18n/locales/ja/common.json#L1302-L1305)・[再開 guard](../../crates/server/src/routes/integrations/runtime.rs#L125-L143)

承認後も Git の反映、Card の Done、Wiki の結果を別々に確認する。「反映結果の照合中」は未反映の確定ではないので、古い画面や status だけから再統合しない。[公開状態と回復](../concepts/formal-integration.md#公開donewiki-は別の結果)を確認する。実行の識別子や保存済み Session は [inspection の詳細](../operations/workspace-inspection.md#実行専用-workspace-の確認)から調べられる。

旧 run の通常 Resume 拒否は新規 engine の安全境界であり、任意の payload 書換えで回避しない。特定履歴を対象とする [checkpoint adoption](../concepts/formal-integration.md#旧-run-の-checkpoint-adoption)も、検査・adoption・正式 Resume が別の操作である。

## 4. Rebase と direct merge

Git 操作は Repo 単位で target_branch を持つ。Rebase API は新 branch の存在確認後に WorkspaceRepo の target を更新し、それから Git rebase を行う。conflict は file と operation を持つ構造化エラーになり、continue / abort の経路がある。エラー時には、Git 状態だけでなく更新済み target 設定も確認する。[Rebase の順序](../../crates/server/src/routes/workspaces/git.rs)・[操作 router](../../crates/server/src/routes/workspaces/git.rs)

Git safety tests は通常の untracked file の保存、tracked dirty change の拒否、base に上書きされる untracked file の保存を検証する。conflict を「とりあえず reset」で消すことを前提にしない。[Rebase tests](../../crates/git/tests/git_ops_safety.rs)

手動 Direct merge は正式 Integration とは別の入口で、その Repo に open PR がある場合と、target が remote branch の場合に拒否する。実際の統合は squash であり、base が先行していれば拒否する。base が別 checkout にあればその場所で CLI merge、checkout がなければ libgit2 の ref 操作を使う。統合後は task branch ref も新 squash commit へ更新し、続きの作業をそこから行えるようにする。[API の制約](../../crates/server/src/routes/workspaces/git.rs)・[Git の統合](../../crates/git/src/lib.rs)

checkout 済み base の staged changes は拒否する。専用 test は staged file を残したまま merge が失敗することを検証する。[test](../../crates/git/tests/git_ops_safety.rs)

memory 有効時は source checkpoint と明示 event の integration 記録を準備してから Git を実行し、成功した commit を保存する。その後で local Merge record を保存する。Workspace は自動 archive しない。Git 成功と後段の記録保存は一つの transaction ではない。[統合順序](../../crates/server/src/routes/workspaces/git.rs)

## 5. Push と Pull Request

Push と PR 作成は、外部公開より前に未完了の source checkpoint を処理する。PR 作成は push remote と base remote を解決し、base branch の存在を検証、source を準備、branch を push、その後 GitHost API で PR を作る。push 後の PR 作成が失敗した場合も、branch の push は既に済み得る。[Push](../../crates/server/src/routes/workspaces/git.rs)・[PR 作成](../../crates/server/src/routes/workspaces/pr.rs)

push は既定 remote の push URL を尊重し、PR base は target branch の remote、解決できなければ既定 fetch remote を使う。fork へ push する設定だけで PR base も fork に変わるわけではない。GitHub の登録 PAT と system 認証の選択、接続確認の限界は [GitHub 認証と push/base の契約](../integrations/github-review.md)を確認する。[remote 解決](../../crates/server/src/routes/workspaces/pr.rs#L236-L254)

GitHostService の現行 provider は GitHub と Azure DevOps。未知 URL は UnsupportedProvider である。[provider 境界](../../crates/git-host/src/lib.rs) provider 固有の CLI 認証や使い方は [GitHub integration](../../docs/integrations/github-integration.mdx) と [Azure Repos integration](../../docs/integrations/azure-repos-integration.mdx) を参照する。

PR の外部作成成功後、local PR record の保存エラーはログに残し、ブラウザー起動と任意の description follow-up の失敗は警告して成功応答を返す。「PR が作成された」と「全ての付随処理が成功した」を区別して診断する。[作成後の部分失敗](../../crates/server/src/routes/workspaces/pr.rs)

PR の状態観測と archive の契約は [GitHub と PR](../integrations/github-review.md) に集約する。旧 Cloud Review の webhook / worker は現行配布にない。

## 6. Issue の完了と Workspace archive

正式 Integration は Git 公開を確認してから、選択 Card と Integration Card の完了結果を保存する。現在は受付時の title/revision 一致を Done の条件にせず、利用可能な可視列（Done 優先、なければ末尾）へ進める。他 run の予約等で更新できなかった結果も記録する。詳細は [Git・Done・Wiki の独立した結果](../concepts/formal-integration.md)を参照する。

Direct merge の成功、PR の merged 観測、正式 Integration の Card 完了は、Workspace を自動 archive しない。PR monitor は状態と merge commit を記録し、open PR が0になった場合の analytics を更新する。[Direct merge 後](../../crates/server/src/routes/workspaces/git.rs)・[PR 観測](../../crates/services/src/services/pr_monitor.rs)

作業領域を解放する場合は [手動 archive の確認と失敗条件](../concepts/workspace.md#archive削除期限-cleanup)へ進む。未 commit の作業も削除対象になるため、統合成功だけで削除可否を推定しない。Card の Done、履歴の保持、物理 worktree の削除は別の結果である。

## 7. 統合後に canonical memory を更新する

[Repository memory](../concepts/repository-memory.md) が有効な場合、成功した coding run の semantic draft と Git 証拠から Change Manifest を作り、統合された event を maintenance の入力にする。source merge の成功と Wiki 更新の成功は別であり、Wiki 失敗は source を巻き戻さない。

LVK 自身の手動 direct squash と正式 Integration はそれぞれ明示的な event ID を記録できる。外部 PR の squash/rebase は元 commit の ancestry を証明できない場合があり、Workspace ID や時刻だけで event を消費しない。統合先 local branch を準備し、必要なら [Sync Wiki](../operations/openwiki-maintenance.md) で実 source を再確認する。[記録された運用契約](../../docs/workspaces/openwiki.mdx)・[外部 PR の制約](../../docs/workspaces/openwiki.mdx)

生成物は [Wiki Viewer](../operations/workspace-inspection.md#wiki-の本文ツリーと表示元を確認する)で確認できる。ただし Viewer は選択 Workspace の現在のファイルを読むため、maintenance の未公開成果物も見える。Wiki 本文を開けたこと、Agent が完了を宣言したこと、Workflow と target branch への publication が成功したことを分けて確認する。[表示元の実装](../../crates/server/src/routes/workspaces/wiki.rs)、[publication の検証](../../crates/server/src/workflow_runtime/bootstrap.rs)

## 定時に Workflow を起動する

ScheduledTask は既存 Workflow template を日次・週次で起動する設定である。現行 target は Workflow のみ、concurrency policy は SkipIfRunning のみ。時刻になった task を期限付き DB claim で取得する。[型](../../crates/server/src/routes/scheduled_tasks.rs)・[claim](../../crates/server/src/routes/scheduled_tasks.rs)

毎回 template の Agent node の session_id を消した graph から新しい WorkflowAttempt を作る。実行は [通常の Workflow lifecycle](../concepts/workflow-attempt.md) に入り、trigger_source は schedule / schedule_manual を使う。[起動](../../crates/server/src/routes/scheduled_tasks.rs)・[Session binding の初期化](../../crates/server/src/routes/scheduled_tasks.rs)

前回の last_run_id が pending / running / awaiting_human / awaiting_arena なら今回は skipped とし次回時刻へ進める。人や Arena の待ち時間も「前回が active」に含まれ、同じ slot を無期限 queue に積む契約ではない。[skip](../../crates/server/src/routes/scheduled_tasks.rs)・[active の範囲](../../crates/server/src/routes/scheduled_tasks.rs)

時刻計算は IANA timezone を用い、daily/weekly と DST の focused tests がある。server のローカル timezone や固定 UTC offset だけで schedule を再実装しない。[tests](../../crates/server/src/routes/scheduled_tasks.rs)

定時 task の起動と、その後に Agent 完了を検知して graph を進めることも別である。この checkout では completion watcher は embedded backend の startup で登録され、standalone main には登録がない。GET/SSE は reconciliation を行い、UI は active run を既定 4 秒で poll する。無人の standalone 運用で予定通り起動したことだけを根拠に、後続段階まで自動進行すると判断しない。[起動形態と前進の契機](../architecture/workflow-runtime.md#起動形態と前進の契機) に登録箇所・read 側の処理・文書の自動連鎖という意図との差をまとめる。[embedded 登録](../../crates/server/src/startup.rs)、[standalone 入口](../../crates/server/src/main.rs)、[poll](../../packages/web-core/src/shared/hooks/useWorkflowRun.ts)
