---
type: concept
title: 正式 Integration の選択・検証・公開
description: local Board で採用した Card と Workspace の変更を固定し、固定候補を受付時の検証方針に従って公開する契約。予約、取消、条件付き Done、Wiki と回復の境界。
tags: [integration, git, validation, lifecycle, recovery]
sources:
  - id: openwiki-source-60b1754d1fc954c43279f888
    resource: repo://.lvk/integration-validation.json
  - id: openwiki-source-c01fa644be961cb7ee12d2b5
    resource: repo://crates/db/src/models/integration.rs
  - id: openwiki-source-d40a52c39b8604505b97c3e9
    resource: repo://crates/git/src/publication.rs
  - id: openwiki-source-33abd750e40bc0d544b210b5
    resource: repo://crates/server/src/remote_mcp/integration_write_tests.rs
  - id: openwiki-source-83d25ea893da220229c11f8b
    resource: repo://crates/server/src/remote_mcp/launch_tests.rs
  - id: openwiki-source-b5eef61c63c11d3fe10b3ccf
    resource: repo://crates/server/src/remote_mcp/launch.rs
  - id: openwiki-source-6adda10f98b079272f82a4cc
    resource: repo://crates/server/src/remote_mcp/operations.rs
  - id: openwiki-source-595d5d0308d670e38ef27fbf
    resource: repo://crates/server/src/routes/integrations/admission.rs
  - id: openwiki-source-a1c709eaad59af16183269df
    resource: repo://crates/server/src/routes/integrations/legacy_checkpoint.rs
  - id: openwiki-source-864e2ae97802aa9a00527c40
    resource: repo://crates/server/src/routes/integrations/mechanical.rs
  - id: openwiki-source-ecfa47de2e3842c785820c9b
    resource: repo://crates/server/src/routes/integrations/mod.rs
  - id: openwiki-source-b393e75029374eaf9cba5f7d
    resource: repo://crates/server/src/routes/integrations/runtime.rs
  - id: openwiki-source-693f5c01a606eaeac0692368
    resource: repo://crates/server/src/routes/integrations/scenario_tests.rs
  - id: openwiki-source-001b80ecf41631f84729dd2e
    resource: repo://crates/server/src/routes/integrations/steps.rs
  - id: openwiki-source-fc7b2dc1ea0b402e0e94b60b
    resource: repo://docs/development/integration-mcp-decisions.md
  - id: openwiki-source-db035046d01e3f24722812c8
    resource: repo://packages/web-core/src/features/kanban/ui/IntegrationPanel.tsx
  - id: openwiki-source-a836651555f6dfa0a9ab8ce3
    resource: repo://packages/web-core/src/features/workspace-chat/ui/IntegrationInterventionPanel.tsx
  - id: openwiki-source-2b041ccdd1b3cb2c0a7e82a6
    resource: repo://packages/web-core/src/features/workspace-chat/ui/IntegrationResolutionReview.tsx
  - id: openwiki-source-0b2c1cfa8089016da3ce0c95
    resource: repo://tests/integration-review/browser.mjs
  - id: openwiki-source-9871b94e679a5398da344b52
    resource: repo://tests/integration-review/README.md
generated: { by: "codex", at: "2026-10-23T23:24:04.082Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-23T23:24:04.082Z
---

# 正式 Integration の選択・検証・公開

正式 Integration は local Board で選択した Card の固定 commit を、一つの Repo の local target branch へ順番に統合する製品実行である。受付時に Integration 用 Card・[Workspace](workspace.md#操作用途と実行所有者) とリンクを作り、専用 Session で作業する。通常 Workflow、手動 squash merge、[detached preview](../workflows/task-to-integration.md#並列変更を試してから正式統合する)とは別の lifecycle を持つ。[受付](../../crates/server/src/routes/integrations/admission.rs)、[原子的な作成](../../crates/db/src/models/integration.rs#L251-L317)。

## 選択するのは commit

各 Card に一つの採用 Workspace と `expected_commit` を指定する。選択元が現在その Card / Project に関連し、対象 Repo の branch HEAD が expected commit と一致することを受付時に確認する。保存後の対象はその immutable commit であり、source branch が進んでも採用内容は変わらない。source の作業中差分、active Goal、他 Workspace・他 Repo の未統合変更を全体停止の条件にしない。後続検査は固定 Git object の利用可能性を確認する。[source 検査と再検査](../../crates/server/src/routes/integrations/admission.rs#L59-L160)。

Card の情報、branch、OID と、帰属・祖先関係を確認できた Memory event IDs を記録する。記録が欠けていること自体は選択 commit を失格にしない。同じ request ID は同じ選択・設定にのみ再利用できる。採用 scope は commit の履歴であり、Card description は変更ファイルの完全な許可リストではない。[受付と冪等性](../../crates/server/src/routes/integrations/admission.rs)、[実行契約](../../crates/server/src/routes/integrations/runtime.rs#L349-L392)。

source Card / Workspace 全体を予約する旧方式ではない。canonical Git common directory を基準に target 実行を直列化し、実行用 Workspace と、公開時の target writer を予約する。別 Repo ID や別 branch という別名でも同じ Git storage の順番を回避できない。予約と実際の writer 収束は dispatcher lease と別である。[target 順番](../../crates/server/src/routes/integrations/admission.rs)、[実行環境の予約](../../crates/server/src/routes/integrations/runtime.rs#L679-L778)、[公開時の予約](../../crates/server/src/routes/integrations/runtime.rs#L1329-L1381)。共有デバイス等の操作は別の [共有リソース調停](shared-resource-coordination.md)契約に従う。

## B から一件ずつ R を検証する

B は受付時に観測した target commit と一致する preparation の base である。queue 待ちの間に target が進めば、新しい HEAD を黙って採用せず `TARGET_CHANGED` で停止する。[固定 target](../../crates/server/src/routes/integrations/admission.rs#L141-L157)・[準備の照合](../../crates/server/src/routes/integrations/runtime.rs#L679-L703)

新規 run は `mechanical=true` の host-owned engine を使う。host が前 step の結果と現在の固定 source を `merge-tree` で評価し、競合しなければ AgentRun を作らず commit tree を生成する。commit の親は前結果と source で、source が既に祖先なら前結果をそのまま使う。R=B も許され、空 commit は不要である。host は clean R と前結果・source の祖先関係を再確認し、逐次履歴として B と採用済み source を保持する。[merge loop](../../crates/server/src/routes/integrations/runtime.rs#L467-L629)・[候補生成](../../crates/server/src/routes/integrations/mechanical.rs#L173-L208)

候補 OID を DB に保存してから private branch へ適用する。適用済みの同じ clean HEAD は再適用せず、既知の conflict merge metadata だけを片付ける。未知の dirty tree、移動した HEAD、別の MERGE_HEAD を推測で reset しない。これは branch 更新と DB 保存の間の中断を識別する仕組みである。[private branch と install](../../crates/server/src/routes/integrations/mechanical.rs#L29-L40)・[適用](../../crates/server/src/routes/integrations/mechanical.rs#L210-L269)

### 競合だけを有限の Agent job にする

競合時だけ、同じ Integration Workspace / Session に新しい AgentRun を作る。現在の step・base・source、受付時の公開 Memory、追加指示を含む完全な Create envelope を provider 起動前に保存し、再配送も同じ identity を使う。Integration 全体を一つの Orchestration node に重ねたり、古い AgentRun の Retry / Steer で次の source を渡したりしない。[保存から Create](../../crates/server/src/routes/integrations/runtime.rs#L500-L541)・[request](../../crates/server/src/routes/integrations/mechanical.rs#L271-L339)

agent の責務は既存 index の競合解消と stage、および `ready` / `blocked` と理由を含む JSON 応答である。commit・branch 操作・検証計画・テスト実行・公開は host が担当する。host は成功かつ Current の投影と実 writer の収束を待ち、HEAD / MERGE_HEAD、未解消 index、unstaged / untracked file、canonical OpenWiki の変更を検査してから commit を作る。応答の ready だけで候補を採用しない。[依頼契約](../../crates/server/src/routes/integrations/runtime.rs#L349-L392)・[結果検査](../../crates/server/src/routes/integrations/runtime.rs#L541-L565)・[index 検査](../../crates/server/src/routes/integrations/mechanical.rs#L134-L171)

各 step の自動 merge tree と候補との差分を保存し、conflict または自動結果以外の変更がある場合は candidate/scope に結び付いた human review を要求する。承認後は新たな agent を起動せず merge loop を進める。候補が変われば承認を流用できない。**review は次の source に進む前、受入検証は全 source の最終候補で行う。** prefix step の空の validation は prefix をテスト済みとする証拠ではない。[review と step](../../crates/server/src/routes/integrations/steps.rs#L48-L164)・[承認後の状態](../../crates/db/src/models/integration.rs#L463-L481)

### 保存済み説明から候補をレビューする

レビューの説明は `record_review` がその時点の `semantic_summary` を保存したもので、画面は保存本文を表示する。空白だけなら説明欠落を明示し、理由を生成して補わない。説明の十分性や解消の意味的妥当性は人が判断する。候補識別子と自動 merge tree からの解消差分は native `details` 内に置き、説明、古い未決候補の警告、現在候補に検証記録がない表示は折り畳まない。[説明の保存](../../crates/server/src/routes/integrations/steps.rs#L48-L97)・[表示](../../packages/web-core/src/features/workspace-chat/ui/IntegrationResolutionReview.tsx#L28-L79)

承認ボタンは未決 review が現在の候補と一致し、run が `blocked` / `merge_review` で、取消要求も公開 intent もない場合だけ表示する。同じ mounted UI 内の再送では review ID ごとに同じ request ID を使い、成功・失敗のどちらでも integrations query を無効化して再取得する。これは backend の candidate/scope・取消・公開検査の代替ではない。古い画面から送った承認も DB 側で拒否できる。[UI の条件と再取得](../../packages/web-core/src/features/workspace-chat/ui/IntegrationResolutionReview.tsx#L10-L25)・[操作条件](../../packages/web-core/src/features/workspace-chat/ui/IntegrationResolutionReview.tsx#L70-L105)・[原子的判断](../../crates/db/src/models/integration.rs#L420-L482)

`merge_review` 中は追加指示の入力欄を出さず、承認または Integration 中止を案内する。固定済み候補への非空の指示は backend でも拒否されるため、説明が足りないときは承認せず確認する。一般の Agent approval とこの解消 review は別の判断である。表示元と Session の選び方は [実行専用 Workspace の確認](../operations/workspace-inspection.md#実行専用-workspace-の確認)を参照する。[介入 UI](../../packages/web-core/src/features/workspace-chat/ui/IntegrationInterventionPanel.tsx#L169-L214)・[候補凍結の guard](../../crates/server/src/routes/integrations/runtime.rs#L125-L143)

## 検証方針は受付時に固定する

`skip_validation` は run 全体の選択である。新規受付は明示値を優先し、省略または `null` は保存済み server preference `integration_skip_validation` を引き継ぐ。初期値と旧記録は false。domain 受付の同じ request ID の再送では、値の省略または `null` は既存 run の選択を維持し、明示値が違えば競合になる。後から preference を変えても既存 run の方針は変わらない。[受付](../../crates/server/src/routes/integrations/admission.rs#L242-L265)・[再送](../../crates/server/src/routes/integrations/admission.rs#L325-L359)

外部 MCP では、この domain 受付より前に元の入力全体を照合する。省略と `null` の入替えも同じ request ID の再送には使えない。起動時の指定と保存値の確認は [MCP から Integration の検証方針を選ぶ](../integrations/remote-access.md#mcp-から-integration-の検証方針を選ぶ)を参照する。

検証必須の場合は、受付 target の Git object から `.lvk/integration-validation.json` の `commands` を読み、全 plan を凍結する。作業木・source branch・agent の proposal から選ばない。設定の欠落・不正、空の一覧、32件超、空文字や `true` / `:` / `exit 0` は受付エラーである。全 command は Repo root を cwd とする required check で、前提不足も失敗となる。[plan](../../crates/server/src/routes/integrations/mechanical.rs#L42-L85)

この repository の設定は、`pnpm install --frozen-lockfile` → 実 frontend build → `pnpm run check` → `pnpm run lint` → `pnpm run backend:test` の順である。依存の準備も固定 plan に含めるため、source が設定を書き換えても当該 run の plan は変わらない。[設定](../../.lvk/integration-validation.json)

省略 run は設定自体を読まず、検証を実行せず、summary と Manifest tests に unverified / NotRun、step に `validation_skipped` を保存する。省略 run と検証実行証拠・gate の混在、必須 run と省略済み step の混在は拒否する。clean・祖先・source 順序・target 一致・review・取消と公開の排他は維持する。[省略の検査](../../crates/server/src/routes/integrations/runtime.rs#L1258-L1283)・[Manifest](../../crates/server/src/routes/integrations/runtime.rs#L630-L670)。これは failed check の候補限定例外とは別で、成功した Integration でも受入検証済みとは限らない。UI は次回用 preference と未検証表示を持つ。[UI](../../packages/web-core/src/features/kanban/ui/IntegrationPanel.tsx)

## 開発検証と human gate

host は全 source の merge/review 後、最終候補 R 上で固定 commands を Bash の `IntegrationValidation` ExecutionProcess として逐次実行する。各 prefix では実行しない。cwd は symlink 解決後も Repo 内、前後の commit は同じ clean R、上限は一 command 一時間で、終了後の after-HEAD と process cleanup も待つ。ログの成功宣言ではなく、Session・reason・command / cwd・非 dropped・終了結果・before/after commit を再照合する。[実行](../../crates/server/src/routes/integrations/runtime.rs#L804-L943)・[証拠照合](../../crates/server/src/routes/integrations/runtime.rs#L1258-L1327)。これは固定 plan の実行証明であり、意味的な要件網羅の機械的証明ではない。

required check が通らない場合は blocked とし、失敗証拠とリスクを保存する。利用者は retry・その check と候補に限定した例外 continue・cancel を選べる。例外は合格への書換えではなく、対象候補の保証欠落を明示した判断である。actor、request ID、時刻と scope を保存し、古い判断や別候補への再利用を拒否する。自動 AI 修正ループは開始しない。[gate](../../crates/server/src/routes/integrations/runtime.rs#L945-L972)・[原子的判断](../../crates/db/src/models/integration.rs#L325-L407)

外部 MCP の明示承認もこの UI 共通判断へ入り、取消も既存 handler を使う。新しい承認 engine や自動承認は作らない。MCP 固有の固定証拠・scope・再送と、取消受付から予約解放までの読み方は [MCP から承認・取消する](../integrations/remote-access.md#mcp-から-integration-を承認取消する)を参照する。[共通経路への委譲](../../crates/server/src/remote_mcp/launch.rs#L135-L154)

## 公開・Done・Wiki は別の結果

公開直前に target が B のままであること、固定 source の存在、選択した検証方針と review の完了、実行環境と target writer の idle を確認する。取消と `publication_intent` は DB writer transaction で一方だけが勝つ。この境界を越えた Cancel は Git undo にならない。Git は ref lock 下で B を比較し、追加 merge commit を作らず **固定した R そのもの**を公開する。target checkout は許可された managed path・clean 状態・一つの checkout を要求し、ignored file の衝突も保護する。Git の ref / index / files と DB は一 transaction ではない。[公開](../../crates/server/src/routes/integrations/runtime.rs#L1329-L1407)、[Git publication](../../crates/git/src/publication.rs)。

Board 一覧と Workspace の専用介入欄は、完全な IntegrationRun の `published` を優先し、true なら反映済み、false かつ `publication_intent` があれば「反映結果の照合中」、どちらもなければ未反映と表示する。status 名だけで未反映と推測しない。Git 更新後の receipt 保存が失敗し得るため、intent が残る `blocked` / `recovery_required` でも未反映と断定できない。これは再実行の指示ではなく、下記の Recover による証拠照合を必要とする状態である。[Board の表示](../../packages/web-core/src/features/kanban/ui/IntegrationPanel.tsx#L530-L540)・[Workspace の表示](../../packages/web-core/src/features/workspace-chat/ui/IntegrationInterventionPanel.tsx#L82-L92)・[Git と DB の引渡し](../../crates/server/src/routes/integrations/runtime.rs#L1383-L1406)

Git publication 確認後、選択 Card と Integration Card を Done に移す。可視列では名称 Done を優先し、なければ最後の列を選ぶ。Card の title・description・revision やリンクが途中で変わったことは現在の Done 不適格条件ではない。別 Integration の予約、Card / 移動先不在なら理由を記録して変更しない。Card 更新と receipt は同一 transaction で、一度記録した結果は再適用しない。[Done の実装](../../crates/db/src/models/integration.rs#L540-L610)、[変更後の Card を扱うテスト](../../crates/db/src/models/integration.rs#L1251-L1412)。

Memory 有効時は凍結した event IDs と統合固有 Manifest を outbox へ結ぶ。Memory 無効なら記録を新設しない。semantic outbox や後続 Wiki の失敗は成功した Git / Done を取り消さず、Integration succeeded は Wiki 完了の証明ではない。[後処理](../../crates/server/src/routes/integrations/runtime.rs#L1465-L1582)。記録の寿命と acknowledgement は [Repository Memory](repository-memory.md)に説明する。

## 中断・介入・回復

公開前に blocked となった mechanical run は、保持した Workspace・固定 source・writer の停止・target B を確認して明示 Resume できる。候補が凍結済みなら非空の追加指示を拒否する。まだ候補がなければ前 conflict job の終端を確認し、index を保った新しい Create で解消を続ける。server restart 後の検証 command は自動 replay せず、明示 Resume から未完了の検証を再開する。既存の passed / candidate-bound exception は同じ候補の証拠として扱う。[介入](../../crates/server/src/routes/integrations/runtime.rs#L106-L206)・[中断検出](../../crates/server/src/routes/integrations/runtime.rs#L432-L465)・[検証再開](../../crates/server/src/routes/integrations/runtime.rs#L817-L830)

技術的 hold は conflict job を勝手に再起動・取消しない。writer が収束していなければ予約を保持する。明示 Cancel は受理済み identity の停止と script/host の idle を確認してから解放する。検証失敗や review 待ちでは候補の予約を維持し、技術的失敗が既に idle なら解放する。[hold](../../crates/server/src/routes/integrations/runtime.rs#L1154-L1200)・[取消](../../crates/server/src/routes/integrations/runtime.rs#L994-L1152)

公開 intent 後は Recover が Git 証拠を照合する。R が適用済みなら後処理へ、ref / index / files が B のままなら未適用として止まり、新しい選択を求める。途中状態や未知の ref を推測で reset しない。確定 receipt がある場合のみ、その後の正当な target 前進を許容できる。[回復](../../crates/server/src/routes/integrations/runtime.rs#L1409-L1463)、[Git 証拠](../../crates/git/src/publication.rs#L82-L132)。

standalone と embedded の monitor 配線差は [起動順序](../architecture/system.md#起動時は内側の所有者から回復する)を参照。実行結果・ログ・owner 情報は保持されるが、終了と cleanup の条件を満たす Workspace は一時的な execution-only 予約を解除し通常操作へ戻せる。[用途解除](workspace.md#操作用途と実行所有者)。

成功・公開済み run の Recover は Git / Card の確定結果を再適用せず memory 後処理を再試行する。旧 run の製品 node は親 orchestration 投影も再照合するが、mechanical run の製品完了は無操作であり、adoption された旧 terminal audit も保持する。[成功後回復](../../crates/server/src/routes/integrations/runtime.rs#L62-L93)・[製品完了](../../crates/server/src/routes/integrations/runtime.rs#L1202-L1221)

### 旧 run の checkpoint adoption

旧 payload は `mechanical=false` が既定である。通常 Resume は `LEGACY_INTEGRATION` で拒否し、過去の Create/Steer や degraded projection を自動 replay しない。Cancel と publication recovery は残る。これは [一般 AgentRun Retry の限定投影回復](../architecture/agent-runtime.md#明示-retry-に限る-startup-collision-の回復)とは別の互換境界である。[engine guard](../../crates/server/src/routes/integrations/mechanical.rs#L87-L94)・[monitor の分岐](../../crates/server/src/routes/integrations/runtime.rs#L432-L465)

`POST /api/integrations/{id}/adopt-checkpoint` は汎用移行ではなく、run `c68ce019-9349-4679-976f-326120191c08` と候補 `8d0eda6b75cce0d0930a17c46d43c5d162f58e7b`、固定 payload digest だけを受け付ける。project、安定 request ID、candidate と必須 `dry_run` を指定する。3件の承認済み prefix、親/source ref、clean private checkout、scope と解決 diff、skip 方針、終了証拠、配送済み outbox と既知の失敗 command を照合する。dry-run は DB を変更しない。[受付と照合](../../crates/server/src/routes/integrations/legacy_checkpoint.rs#L19-L174)・[Git / review](../../crates/server/src/routes/integrations/legacy_checkpoint.rs#L223-L304)

実行時は必要な旧 cancelling product の完了を既存 API で収束させ、元 payload・error・Agent identity を履歴へ残し、payload CAS で adoption する。同じ request は冪等に再送できる。状態は blocked のままで、別の明示 Resume が source 4 を再計算する。旧 parent Cancelled / node Succeeded の場合は特定の109/110 event と成功した元 child link を要求し、terminal audit を書き換えない。adoption は Git 公開・古い Agent の再実行・成功への格上げではない。[履歴保全と CAS](../../crates/server/src/routes/integrations/legacy_checkpoint.rs#L131-L220)

## 検証の入口と記録の限界

[機械的 merge の回帰テスト](../../crates/server/src/routes/integrations/runtime_tests.rs#L1340-L1643)は実 temporary Git を使い、重複適用、conflict cleanup 中断、承認の再送と失効、dirty/moved HEAD、固定 plan と skip、Create identity を扱う。[隔離 scenario](../../crates/server/src/routes/integrations/scenario_tests.rs)は production host entrypoint を使い、conflict→review→次 source→publication、startup reconciliation→adoption→Resume を検査する。通常は ignore され、独立した新規 `LVK_INSTANCE_DIR` を要求する。host scenario の実 Codex は追加 opt-in、adoption は private fixture を要する。browser scenario は別の ignored test として用意され、外部 MCP の E2E はこれらの試験範囲に含まれない。

[調査・回復記録](../../docs/development/integration-dispatch-recovery.md)は設計判断と過去の検証結果を残す。旧 per-step handoff の撤去後も、一般 Workflow の配送受理と SQLite writer 予約、一般 Retry の回復は依存先がある。記録中の instance 固定 cutover 手順や live 未実行の記述を、現在の配置状態として扱わない。この Sync は統合 source を調査したもので、過去の binary、live adoption、公表済みテスト成功を再確認したものではない。

[隔離ブラウザ試験](../../tests/integration-review/README.md)は build 済み frontend、実 router / local Access、一時 SQLite・Git、loopback の port 0、Chromium を使う。provider 起動・回答は固定イベントに置換し、server main 全体の monitor ではなく試験が実 `advance` を駆動する。承認から公開、取消後の target 不変、候補差替え後の古い承認拒否、保存説明欠落、reload と Enter/Space による詳細開閉、判断再送を検査する。空説明は保存記録を変える fixture であり、現行 parser が空 summary を受け付ける意味ではない。新規受付 UI、実 AI の解消品質、public Basic/Auth0、外部 MCP の検証ではない。[隔離起動](../../crates/server/src/routes/integrations/scenario_tests.rs#L30-L77)・[候補と公開の確認](../../crates/server/src/routes/integrations/scenario_tests.rs#L300-L438)・[browser 操作](../../tests/integration-review/browser.mjs)・[parser](../../crates/server/src/routes/integrations/mechanical.rs#L342-L356)

公開 intent の有無と published の優先順位は [介入欄の component test](../../packages/web-core/src/features/workspace-chat/ui/IntegrationInterventionPanel.test.tsx#L163-L216)、重複した公開表示をしないことは [inspection test](../../packages/web-core/src/features/workspace-chat/ui/ExecutionInspectionPanel.test.tsx#L160-L188)が扱う。今回の Sync は凍結 source `8cf59511e3919425397fe1963d6d90be69fa48a2` の UI、保存・判断・公開処理、試験定義を静的に照合した。供給された正式 Integration は検証省略であり、開発時の成功記録を統合後の検証成功へ読み替えない。この Sync では製品試験を再実行せず、無関係な既存契約の再検証も主張しない。

MCP の [隔離 host scenario](../../crates/server/src/routes/integrations/scenario_tests.rs#L741-L768) は、実 Git / SQLite と共通 domain に adapter から入り、承認後の公開と、取消受付時の予約保持から解放までを検査する。[adapter の境界試験](../../crates/server/src/remote_mcp/integration_write_tests.rs#L26-L244)は権限不足、別 Project、固定証拠の各要素・live target の変更、同一要求の同時送信、receipt と現在状態の差も扱う。provider は固定イベントであり、新 tool の成功を HTTP transport 全体で確認する E2E や実 Auth0 / ChatGPT の確認とは異なる。[実装時の検証記録と限界](../../docs/development/integration-mcp-decisions.md)を今回の試験実行結果に読み替えない。
