---
type: concept
title: 正式 Integration の選択・検証・公開
description: local Board で採用した Card と Workspace の変更を固定し、host 検証済み commit だけを公開する契約。予約、取消、条件付き Done、Wiki と回復の境界。
tags: [integration, git, validation, lifecycle, recovery]
sources:
  - id: openwiki-source-c01fa644be961cb7ee12d2b5
    resource: repo://crates/db/src/models/integration.rs
  - id: openwiki-source-d40a52c39b8604505b97c3e9
    resource: repo://crates/git/src/publication.rs
  - id: openwiki-source-595d5d0308d670e38ef27fbf
    resource: repo://crates/server/src/routes/integrations/admission.rs
  - id: openwiki-source-b393e75029374eaf9cba5f7d
    resource: repo://crates/server/src/routes/integrations/runtime.rs
  - id: openwiki-source-001b80ecf41631f84729dd2e
    resource: repo://crates/server/src/routes/integrations/steps.rs
generated: { by: "codex", at: "2026-10-16T12:21:28.631Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-16T12:21:28.631Z
---

# 正式 Integration の選択・検証・公開

正式 Integration は local Board で選択した Card の固定 commit を、一つの Repo の local target branch へ順番に統合する製品実行である。受付時に Integration 用 Card・[Workspace](workspace.md#操作用途と実行所有者) とリンクを作り、専用 Session で作業する。通常 Workflow、手動 squash merge、[detached preview](../workflows/task-to-integration.md#並列変更を試してから正式統合する)とは別の lifecycle を持つ。[受付](../../crates/server/src/routes/integrations/admission.rs)、[原子的な作成](../../crates/db/src/models/integration.rs#L251-L317)。

## 選択するのは commit

各 Card に一つの採用 Workspace と `expected_commit` を指定する。選択元が現在その Card / Project に関連し、対象 Repo の branch HEAD が expected commit と一致することを受付時に確認する。保存後の対象はその immutable commit であり、source branch が進んでも採用内容は変わらない。source の作業中差分、active Goal、他 Workspace・他 Repo の未統合変更を全体停止の条件にしない。後続検査は固定 Git object の利用可能性を確認する。[source 検査と再検査](../../crates/server/src/routes/integrations/admission.rs#L59-L160)。

Card の情報、branch、OID と、帰属・祖先関係を確認できた Memory event IDs を記録する。記録が欠けていること自体は選択 commit を失格にしない。同じ request ID は同じ選択・設定にのみ再利用できる。採用 scope は commit の履歴であり、Card description は変更ファイルの完全な許可リストではない。[受付と冪等性](../../crates/server/src/routes/integrations/admission.rs)、[実行契約](../../crates/server/src/routes/integrations/runtime.rs#L659-L739)。

source Card / Workspace 全体を予約する旧方式ではない。canonical Git common directory を基準に target 実行を直列化し、実行用 Workspace と、公開時の target writer を予約する。別 Repo ID や別 branch という別名でも同じ Git storage の順番を回避できない。予約と実際の writer 収束は dispatcher lease と別である。[target 順番](../../crates/server/src/routes/integrations/admission.rs)、[実行環境の予約](../../crates/server/src/routes/integrations/runtime.rs#L990-L1080)、[公開時の予約](../../crates/server/src/routes/integrations/runtime.rs#L1643-L1696)。共有デバイス等の操作は別の [共有リソース調停](shared-resource-coordination.md)契約に従う。

## B から一件ずつ R を検証する

B は queue の順番が来て preparation を始めた時点の target commit である。各 step は前 step の検証済み結果に次の source を history-preserving merge する。host は候補 R が B・前 step・ここまでの source の子孫であることと clean 状態を確認する。後の source を先取りして一括検証することは許されない。ただし既に履歴に含まれる場合を除く。`R=B` も許され、空 commit は不要である。[準備](../../crates/server/src/routes/integrations/runtime.rs#L990-L1048)、[祖先検査](../../crates/server/src/routes/integrations/runtime.rs#L860-L919)。

各 step には自動 merge tree と候補との差分を保存する。conflict がある、または自動 merge 結果以外の変更がある場合、検証後に解決差分と説明の human review を要求する。承認は候補と scope に結び付けられ、候補が変われば流用できない。承認・検証を終えてから次の source へ進み、全 step 完了後にだけ target 公開へ進む。[解決差分](../../crates/server/src/routes/integrations/steps.rs)、[step の遷移](../../crates/server/src/routes/integrations/runtime.rs#L1287-L1330)。

## 開発検証と human gate

agent は Code mode で、選択集合と一致する JSON proposal、command・相対 cwd・開発上の根拠・環境要件・除外理由を提出する。ready には required command が必要で、空 command や `true` / `exit 0` / `:` は不可。対象は main とここまで採用した変更の開発受入検証を累積したもの。CI / release に存在するという理由だけで必須条件を追加せず、廃止・置換・分類変更をした検証は理由を残す。軽量な前提調査で既知の環境不足をまとめ、実行不能な plan は host テスト開始前に clarification とする。[proposal 検査](../../crates/server/src/routes/integrations/runtime.rs#L452-L626)、[方針](../../crates/server/src/routes/integrations/runtime.rs#L704-L728)。

host は固定 R 上で command を Bash の `IntegrationValidation` ExecutionProcess として逐次実行する。cwd は symlink 解決後も Repo 内、前後の commit は同じ clean R、上限は一 command 一時間で、終了後の after-HEAD と process cleanup も待つ。ログの成功宣言ではなく、Session・reason・command / cwd・非 dropped・終了結果・before/after commit を再照合する。[実行](../../crates/server/src/routes/integrations/runtime.rs#L1171-L1286)、[証拠照合](../../crates/server/src/routes/integrations/runtime.rs#L1587-L1641)。これは plan の実行証明であり、意味的な完全網羅の機械的証明ではない。

required check が通らない場合は blocked とし、失敗証拠とリスクを保存する。利用者は retry・その check と候補に限定した例外 continue・cancel を選べる。例外は合格への書換えではなく、対象候補の保証欠落を明示した判断である。actor、request ID、時刻と scope を保存し、古い判断や別候補への再利用を拒否する。[gate](../../crates/server/src/routes/integrations/runtime.rs#L1332-L1359)、[原子的判断](../../crates/db/src/models/integration.rs#L319-L460)。

## 公開・Done・Wiki は別の結果

公開直前に target が B のままであること、固定 source の存在、検証・review の完了、実行環境と target writer の idle を確認する。取消と `publication_intent` は DB writer transaction で一方だけが勝つ。この境界を越えた Cancel は Git undo にならない。Git は ref lock 下で B を比較し、追加 merge commit を作らず **検証済み R そのもの**を公開する。target checkout は許可された managed path・clean 状態・一つの checkout を要求し、ignored file の衝突も保護する。Git の ref / index / files と DB は一 transaction ではない。[公開](../../crates/server/src/routes/integrations/runtime.rs#L1643-L1721)、[Git publication](../../crates/git/src/publication.rs)。

Git publication 確認後、選択 Card と Integration Card を Done に移す。可視列では名称 Done を優先し、なければ最後の列を選ぶ。Card の title・description・revision やリンクが途中で変わったことは現在の Done 不適格条件ではない。別 Integration の予約、Card / 移動先不在なら理由を記録して変更しない。Card 更新と receipt は同一 transaction で、一度記録した結果は再適用しない。[Done の実装](../../crates/db/src/models/integration.rs#L519-L592)、[変更後の Card を扱うテスト](../../crates/db/src/models/integration.rs#L1251-L1412)。

Memory 有効時は凍結した event IDs と統合固有 Manifest を outbox へ結ぶ。Memory 無効なら記録を新設しない。semantic outbox や後続 Wiki の失敗は成功した Git / Done を取り消さず、Integration succeeded は Wiki 完了の証明ではない。[後処理](../../crates/server/src/routes/integrations/runtime.rs#L1779-L1892)。記録の寿命と acknowledgement は [Repository Memory](repository-memory.md)に説明する。

## 中断・介入・回復

公開前に blocked となった run は、保持した Workspace を確認して追加指示・明示 resume を行える。選択 commit の存在、writer の停止、target B を再確認し、古い候補・検証結果・未判断 gate を引き継いだ成功扱いにはしない。server restart 後の検証 command は自動 replay せず、保持ログを確認して明示 resume から再検証する。[介入](../../crates/server/src/routes/integrations/runtime.rs#L111-L246)、[中断検出](../../crates/server/src/routes/integrations/runtime.rs#L964-L978)。取消は child agent と script の実停止を確認してから予約を解放する。

公開 intent 後は Recover が Git 証拠を照合する。R が適用済みなら後処理へ、ref / index / files が B のままなら未適用として止まり、新しい選択を求める。途中状態や未知の ref を推測で reset しない。確定 receipt がある場合のみ、その後の正当な target 前進を許容できる。[回復](../../crates/server/src/routes/integrations/runtime.rs#L1723-L1777)、[Git 証拠](../../crates/git/src/publication.rs#L82-L132)。

standalone と embedded の monitor 配線差は [起動順序](../architecture/system.md#起動時は内側の所有者から回復する)を参照。実行結果・ログ・owner 情報は保持されるが、終了と cleanup の条件を満たす Workspace は一時的な execution-only 予約を解除し通常操作へ戻せる。[用途解除](workspace.md#操作用途と実行所有者)。
