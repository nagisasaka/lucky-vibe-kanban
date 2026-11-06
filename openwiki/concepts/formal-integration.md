---
type: concept
title: 自動マージの選択・検証・公開
description: local Board で採用した Card と Workspace の変更を固定し、固定候補を受付時の検証方針に従って公開する契約。予約、取消、条件付き Done、Wiki と回復の境界。
tags: [integration, git, validation, lifecycle, recovery]
sources:
  - id: openwiki-source-60b1754d1fc954c43279f888
    resource: repo://.lvk/integration-validation.json
  - id: openwiki-source-124913f65e422092e3ffbd4a
    resource: repo://assets/skills/auto-merge/SKILL.md
  - id: openwiki-source-11bd8cfafd506215798951a6
    resource: repo://crates/db/migrations/20261025000000_issue_classification.sql
  - id: openwiki-source-3c57aab40b425fc8b5d0d5c5
    resource: repo://crates/db/migrations/20261027090000_generated_issue_status.sql
  - id: openwiki-source-9bc71fe4f6cbbaa6fa9168e1
    resource: repo://crates/db/src/models/agent_runtime_kanban_tests.rs
  - id: openwiki-source-c01fa644be961cb7ee12d2b5
    resource: repo://crates/db/src/models/integration.rs
  - id: openwiki-source-4f59e07f8850eb04105d5cb8
    resource: repo://crates/db/tests/generated_issue_status.rs
  - id: openwiki-source-d40a52c39b8604505b97c3e9
    resource: repo://crates/git/src/publication.rs
  - id: openwiki-source-83d25ea893da220229c11f8b
    resource: repo://crates/server/src/remote_mcp/launch_tests.rs
  - id: openwiki-source-b5eef61c63c11d3fe10b3ccf
    resource: repo://crates/server/src/remote_mcp/launch.rs
  - id: openwiki-source-6adda10f98b079272f82a4cc
    resource: repo://crates/server/src/remote_mcp/operations.rs
  - id: openwiki-source-54244fc34a5fffa34c37581d
    resource: repo://crates/server/src/routes/integration_validation.rs
  - id: openwiki-source-595d5d0308d670e38ef27fbf
    resource: repo://crates/server/src/routes/integrations/admission.rs
  - id: openwiki-source-ee464e7ed81ae180fcca4c26
    resource: repo://crates/server/src/routes/integrations/conversation_tests.rs
  - id: openwiki-source-5720d865d1c837f0e6b7dab7
    resource: repo://crates/server/src/routes/integrations/conversation.rs
  - id: openwiki-source-ecfa47de2e3842c785820c9b
    resource: repo://crates/server/src/routes/integrations/mod.rs
  - id: openwiki-source-b393e75029374eaf9cba5f7d
    resource: repo://crates/server/src/routes/integrations/runtime.rs
  - id: openwiki-source-e000993e074f00ebc0d68cec
    resource: repo://crates/server/src/routes/integrations/support.rs
  - id: openwiki-source-24838d7108e2888a7fdc2714
    resource: repo://crates/server/src/routes/local_remote/issue_archive.rs
  - id: openwiki-source-79cca50cc03fbe39bcd5a22c
    resource: repo://crates/server/src/routes/openwiki/completion.rs
  - id: openwiki-source-c1a5c9bda8d34af222d811c1
    resource: repo://crates/server/src/routes/repo.rs
  - id: openwiki-source-fa966dc92e5b940e2e3155e3
    resource: repo://crates/services/src/services/openwiki/completion.rs
  - id: openwiki-source-c4df16e7e579927d462857d7
    resource: repo://crates/services/src/services/openwiki/setup/integration.rs
  - id: openwiki-source-9734052c014ed77357487607
    resource: repo://packages/web-core/src/features/workspace-chat/ui/IntegrationFacts.test.tsx
  - id: openwiki-source-dbaab3d3ce907e13656fcbed
    resource: repo://packages/web-core/src/features/workspace-chat/ui/IntegrationFacts.tsx
  - id: openwiki-source-344aff2e68b53276f6be4e40
    resource: repo://packages/web-core/src/shared/dialogs/settings/settings/IntegrationValidationSettings.tsx
generated: { by: "codex", at: "2026-11-06T15:24:11.469Z" }
verified:
  - by: openwiki/0.7.2
    at: 2026-11-06T15:24:11.469Z
---

# 自動マージの選択・検証・公開

自動マージは一つのRepoのlocal targetに採用commitを反映する製品操作である。専用Issue・Workspace・Sessionを作り、同じCodex会話が候補作成、競合修復、検証判断、有効なWiki更新を担当する。hostは実行証拠の保存とwriter収束後の正確なGit公開を担当する。旧mechanical engine、競合だけの子job、解消承認gate、checkpoint adoptionは撤去済みである。[会話と公開の実装](../../crates/server/src/routes/integrations/conversation.rs)・[実行契約](../../assets/skills/auto-merge/SKILL.md)

## 選択するのは commit

Cardごとに採用Workspaceとexpected commitを指定し、Projectとの関連とsource HEAD一致を受付時に確認する。採用するのは固定Git objectであり、source作業木全体のclean/idle、Manifest完成や他Workspaceの停止を要求しない。source eventが発見できれば帰属を保存する。同じrequest IDの再送は選択・設定が同じ場合だけ既存結果を返す。外部MCPはdomain受付前に元入力全体をreceipt照合するため、省略とnullの入替えも再送に使わない。[受付](../../crates/server/src/routes/integrations/admission.rs)・[外部受付](../../crates/server/src/remote_mcp/launch.rs)

候補の準備にはrepository全体のFIFOを置かず、専用Workspaceを予約する。公開時だけcanonical Git storageの短い予約とtarget writerのidle検査を行う。デバイスやDBは別の [共有リソース調停](shared-resource-coordination.md)契約に従う。

## B から一件ずつ R を検証する

受付targetと固定sourceは保持し、候補はprivate branchで作る。`integration_action`は現在runtimeへ束縛され、候補に受付baseと全sourceが祖先として残ること、expected SHAとcleanな作業木を確認する。Wiki有効時の公開受付だけは、真正journalと一致する既知の未stage setup生成差分を許す。新commitはrepositoryの有効なGit identityを使い、元sourceの著者を改変しない。[worker guard](../../crates/server/src/routes/integrations/conversation.rs#L170-L222)・[Git identity](workspace.md#git-commit-identity)

### 競合だけを有限の Agent job にする

この見出しは旧参照用に残す。現在は競合の有無によらず一つの専用会話で作業する。新sourceごとのSession作成や旧checkpoint adoptionを実行しない。追加指示・質問・明示再開は通常のSession機構を使う。[起動と再開](../../crates/server/src/routes/integrations/conversation.rs)

### 保存済み説明から候補をレビューする

候補・検証・省略・公開状態はhost確認のfactsとして通常会話に表示する。modelの成功文だけで公開成功にしない。旧resolution review/validation exception UIと承認toolはなく、外部書込は取消を残す。[facts表示](../../packages/web-core/src/features/workspace-chat/ui/IntegrationFacts.tsx)・[外部取消](../../crates/server/src/remote_mcp/integration_write.rs)

## 検証方針は受付時に固定する

targetの `.lvk/integration-validation.json` は参考コマンドとして凍結される。host強制のrequired planではなく、担当AIが差分・リスク・既存SHAに結び付いた証拠から必要な検証を選ぶ。`skip_validation`や追加指示は判断の入力であり、過去のpassや新しい実行結果と混同しない。このRepoの参考一覧はlocked install、real frontend build、check、lint、backend:testである。[参考設定](../../.lvk/integration-validation.json)・[読取](../../crates/server/src/routes/integrations/support.rs#L42-L65)

`verify`はexpected clean SHAで管理ScriptRequestを開始し、request ID・process ID・command・前後SHA・終了結果を保存する。同じIDと同じ入力の再送は別プロセスを起動しない。自己申告はhost実行を代替せず、exit=0も意味的な網羅性の証明ではない。失敗は履歴に残し、修復した候補には新しい証拠が必要である。[管理検証](../../crates/server/src/routes/integrations/conversation.rs#L73-L170)・[実行と照合](../../crates/server/src/routes/integrations/conversation.rs#L255-L345)

`skip`はuser/agent、理由、残リスクを保存する。新候補で古いSHAの検証を再利用する場合も、差分と省略理由を記録し、旧passを新SHA実行と呼ばない。公開時はcode SHAの証拠を照合し、同commandの後続passまたは理由付きskipのない失敗は拒否する。[証拠検査](../../crates/server/src/routes/integrations/conversation.rs#L674-L763)

## 検証コマンドの編集と反映

検証設定は Repo の `.lvk/integration-validation.json` が正本である。Repository settings のエディターは登録 checkout の path・branch・HEAD を表示し、その作業木を編集する。startup/cleanup の DB 設定とは保存先が異なる。DB へ別の command 設定を複製せず、既存の target Git object 読取を維持する判断が記録されている。保存は commit・stage・branch 切替・push・コマンド実行を行わない。目的の target branch に通常の Git 手順で反映してから、新規 自動マージ の受付で 参考コマンドを固定する。既存 run の参考設定は変わらない。[設定画面への接続](../../packages/web-core/src/shared/dialogs/settings/settings/ReposSettingsSection.tsx#L730-L738)・[保存処理](../../crates/server/src/routes/integration_validation.rs#L133-L171)・[受付時の固定](../../crates/server/src/routes/integrations/admission.rs#L242-L262)・[設計判断](../../tests/integration-validation-settings/README.md#正式integration検証設定の隔離試験)

### 編集内容と下書きの寿命

JSON は `commands` だけを持つ object で、1〜32個の文字列を順序どおりに扱う。一つの要素は複数行でもよく、実行時は一つの Bash script になる。空白だけの文字列と、trim 後の `true`・`:`・`exit 0` は拒否するが、これはコマンドの有用性や成功を保証する検査ではない。保存 API と凍結 target reader は同じ Rust parser を使い、UI にも保存可否の検査がある。[parser](../../crates/server/src/routes/integration_validation.rs#L23-L43)・[UI の判定](../../packages/web-core/src/shared/lib/integrationValidationSettings.ts#L14-L40)・[実行器への引渡し](../../crates/server/src/routes/integrations/conversation.rs#L255-L282)

UI は順序付きの複数行入力と追加・削除、raw JSON の修復欄を持つ。読取は JSON の妥当性を要求しないため、不正な既存内容も修復できる。欠落ファイルは `content: null` として表示し、有効な内容を保存して作成する。編集中は最初に読んだ revision を下書きの base に保持する。保存失敗・再読込失敗でも mounted component の下書きを残し、保存成功時はサーバーの読戻し結果を cache に入れる。明示的な「下書きを破棄して再読込」は取得成功時だけ下書きを捨てる。これは再起動をまたぐ永続 draft ではない。[状態と操作](../../packages/web-core/src/shared/dialogs/settings/settings/IntegrationValidationSettings.tsx#L29-L98)・[JSON と command 入力](../../packages/web-core/src/shared/dialogs/settings/settings/IntegrationValidationSettings.tsx#L108-L204)

### 保存 API の競合と失敗

`GET /api/repos/{repo_id}/integration-validation` は登録 Repo の checkout から `branch`・`head`・`revision`・`content` を返す。`PUT` は `expected_revision` と `content` を受け取る。commit を持つ local branch が必要で、detached HEAD は対象外。`.lvk` と設定ファイルの symlink・非通常型を拒否し、内容は最大1 MiBの UTF-8 に制限する。[route](../../crates/server/src/routes/repo.rs#L377-L383)・[読取](../../crates/server/src/routes/integration_validation.rs#L61-L130)

revision は branch・HEAD・ファイルの存在と正確な内容の組から計算する SHA-256 であり、整形だけの変更も競合になる。PUT は Git common directory 単位の既存 publication guard を使うので、別 Repo ID でも同じ storage の保存を同時に通せない。guard を取れない場合や revision 不一致は409となる。検証後、同じ `.lvk` 内の一時ファイルへ書き、既存 permission を引き継ぎ、sync・再照合して原子的に置換し、読戻す。外部 editor や任意の Git 操作はこの lock に参加しないため、最後の比較以降の外部更新まで完全に排他する契約ではない。保存中は外部編集・branch 切替を避ける。[保存順序](../../crates/server/src/routes/integration_validation.rs#L133-L171)・[storage lock](../../crates/git/src/publication.rs#L134-L161)

保存成功の表示は読戻しまでの成功であり、target への commit や検証成功ではない。また置換後の読戻しにも失敗経路があるので、エラー表示だけから「ファイルは変更されていない」と断定しない。下書きを控えて現在のファイルを確認し、競合時は再読込した版を基に編集し直す。[置換と読戻し](../../crates/server/src/routes/integration_validation.rs#L163-L171)・[UI の失敗処理](../../packages/web-core/src/shared/dialogs/settings/settings/IntegrationValidationSettings.tsx#L64-L98)。一連の操作は [正式統合への手順](../workflows/task-to-integration.md#並列変更を試してから正式統合する)を参照する。


## 開発検証と human gate

通常の開発規則と変更リスクに応じた検証は有効である。現行の隔離試験は実Git/SQLite・管理commandでrepair、safe drift、unsafe drift、Wiki error、cancel、intent/receipt crashを扱う。通常ignoreされ、新規の専用LVK_INSTANCE_DIRを要求し、providerは予約だけで実モデルを起動しない。fixtureと実Auth0/ChatGPTのE2Eを区別する。[隔離会話試験](../../crates/server/src/routes/integrations/conversation_tests.rs)

## 公開・Done・Wiki は別の結果

Wiki有効時はコード検証後、同じ候補branchでOpenWikiの正規ライフサイクルを完了しWikiだけを別commitにする。公開要求はcode_commit・wiki_commit・final candidateを別々に固定する。codeからfinalまでWiki以外が変わっていれば拒否する。Wikiだけの後続commitをコード試験済みと偽らない。別の後続Wiki Workspaceは作らない。[公開入力](../../crates/server/src/routes/integrations/conversation.rs#L346-L402)

### begin 前の原本準備と host の限定復元

コード検証・省略の証拠を確定後、最初の `openwiki_begin` より前に `integration_action prepare_wiki(expected_commit=code SHA)` を成功させる。host は証拠とwriter収束を照合し、integration/repository/storage/Session/Workspace/root/code SHAに結び付く原本journalとdigest receiptを保存する。begin-noopでも事前準備は必要で、後から作ったjournalで先行beginを正当化できない。[準備](../../crates/server/src/routes/integrations/conversation.rs#L526-L627)・[監査との順序照合](../../crates/server/src/routes/integrations/conversation.rs#L644-L713)

同じ候補branchでWikiだけを別commitにし、ユーザー所有 `openwiki/INSTRUCTIONS.md` とsourceは変更しない。生成されたAGENTS.md/CLAUDE.mdをstage・commit・手動restoreしない。公開受付はjournalの原本かOpenWiki 0.7.2の既知の正確な変換だけを認める。履歴の各commit/parent、index、Git操作状態も検査し、後で取り消したsource編集や別parent、管理節内外の後編集、リンク、不明なdirtyを拒否する。[履歴と作業木](../../crates/services/src/services/openwiki/setup/integration.rs#L105-L227)・[限定変換](../../crates/services/src/services/openwiki/setup/integration.rs#L336-L395)

担当Runと全writer終了・Wiki完了proofの確認後、hostはtarget writerもidleであることを確かめ、journalに最終Wiki SHAを固定する。Prepared → Restoring → Restoredを永続化して既知setup差分だけを原状へ戻し、cleanを検査してから既存公開guardへ進む。原本欠損・改変・identity不一致では候補を保全して停止する。[復元](../../crates/services/src/services/openwiki/setup/integration.rs#L230-L258)・[公開順](../../crates/server/src/routes/integrations/runtime.rs#L319-L410)

中断・複数beginは同じ原本を使う。復元済み候補から新writer phaseへ進むときだけ、新request IDのprepareを成功させ、元code SHAと過去Wiki SHA・phase baseの祖先関係を保持する。最終SHA固定後の未復元状態（Preparedのcrash窓を含む）やRestoringでprepareが拒否されたらbeginせず、同じ最終SHAを正式に再publishしてhostへ収束を渡す。原本の後付けやguard回避で回復しない。[phase再開](../../crates/services/src/services/openwiki/setup/integration.rs#L35-L65)・[receipt照合](../../crates/server/src/routes/integrations/conversation.rs#L575-L681)

`publish`の受付はGit適用ではない。担当Runを終え、hostがRun・process registry・scriptsの収束とNative Audit上のWiki完了証拠を確認してから公開する。targetが進んでも候補作業は中断せず、公開時に実targetが完成候補の祖先なら候補を変更せずCASで反映する。祖先でなければ候補・証拠を保持して未公開In Reviewに止め、自動merge/rebase・追従連鎖を行わない。[公開](../../crates/server/src/routes/integrations/runtime.rs#L319-L415)

取消とpublication intentはDB transactionで排他する。Gitはref lockでbaseを比較し、新commitを作らず完成候補そのものを反映する。managed target checkoutのclean・path・一意性とignored file衝突も検査する。ref/index/filesとDBは一transactionではない。保存済みintentから実Gitを照合し、適用済みだけ後処理し、未適用や不明な部分適用を推測で再mergeしない。[exact publication](../../crates/git/src/publication.rs)・[回復](../../crates/server/src/routes/integrations/runtime.rs#L420-L480)

公開後は採用元CardのDone処理とreceiptを記録する。Cancelledや公開後の再オープン、他run予約は上書きしない。新しい実開始によるDoneからの再着手に、旧receiptを再適用しない。生成Issueは進行/再開でIn Progress、停止でIn Review、公開済みでDoneへ投影する。成功後の専用Issue/Workspaceは通常archive契約に従い、採用元Workspaceを削除対象にしない。[完了](../../crates/db/src/models/integration.rs)・[生成状態](../../crates/db/migrations/20261027090000_generated_issue_status.sql)・[archive guard](../../crates/server/src/routes/local_remote/issue_archive.rs)

Wiki有効時の公開済みevent receiptと鮮度はinstance DB の repository_wiki_receipts / repository_wiki_state へ保存する。receipt・鮮度stateとintegrationのWiki結果は同じDB transactionで確定する。Manifestファイルとsetup原本journalは引き続きファイルで保持する。Git適用後のreceipt失敗は `Wiki receipt pending` として回復し、既に適用したGitを取り消さず、Card完了やarchiveを巻き戻さない。[後処理](../../crates/server/src/routes/integrations/runtime.rs#L480-L520)・[Wiki receipt](../../crates/server/src/routes/integrations/conversation.rs#L1048-L1100)

## 中断・介入・回復

明示指示で同じ会話を再開できる。保持したedits/indexをresetせず、以前の失敗や検証履歴を成功へ書き換えない。取消は実writer収束を待ち、公開intent後の取消はGit undoにならない。`get_integration`とoperation receiptは別の観測であり、取消受付を予約解放と同一視しない。[再開](../../crates/server/src/routes/integrations/conversation.rs#L624-L672)・[取消](../../crates/server/src/remote_mcp/integration_write.rs)

## 成功後の専用作業領域

生成Issueの自動archiveはCancelledを拒否し、Done投影receiptと現在のDone列を照合して手動再オープンを保持する。公開成功、Wiki完了、後処理receipt、物理worktree削除はそれぞれ別の結果として読む。[共通archive](../../crates/server/src/routes/local_remote/issue_archive.rs)

## 名称と互換境界

利用者向け名称は自動マージ / Auto-merge。現行のstart/get/cancel_integrationとAPI/DB識別子は維持する一方、旧approve_integration・decision/intervene/adopt-checkpointの実行経路は撤去済みである。名称互換を理由に旧executorを再配送しない。[受付と候補の実装](../../crates/server/src/routes/integrations/admission.rs)

## 同じ会話でWiki更新を再開する

OpenWikiの永続runとCodexのAgentRunは別の寿命を持つ。自動マージの公開前検査は、同一Sessionの全AgentRunとattemptのNative Auditを順序どおりに再生し、同じcorrelation owner・割当Workspace・repo rootに属する、過去に監査されたactive ID/modeだけを後続Runで明示再開できる。過去Runが中断・取消されても、writerの終了が確認でき、所有証拠が連続していれば継続できる。`resumed:true`だけを信用せず、別会話から一致するIDを探さない。[会話proof](../../crates/server/src/routes/openwiki/completion.rs#L115-L186)・[継続状態](../../crates/services/src/services/openwiki/completion.rs)

監査の欠損・改変、未解決tool call、writerの重複、active IDの置換や放棄、閉じたIDの再利用、最終未完了は公開を拒否する。最新Runとその最終attemptの成功も必要である。standalone保守のphase単位の閉鎖条件はこの会話継続へ置き換えない。[再生とidentity検査](../../crates/server/src/routes/openwiki/completion.rs#L188-L315)・[operation proof](../../crates/services/src/services/openwiki/completion.rs#L200-L385)。正規再開からexact publication・生成環境archiveまでの隔離fixtureは実Git/SQLiteを使うが、実外部OpenWikiや実Codexの接続確認を代替しない。[隔離回帰](../../crates/server/src/routes/integrations/conversation_tests.rs)

### 検索・読取と operation の閉鎖

maintenance writer は OpenWiki 0.7.2 の検索・読取・workspace/Wiki一覧を利用できる。読取は lifecycle の前後・重なりを許すが、成功も失敗も active run を閉じず、未完了 writer を隠さない。root/thread・call ID・引数・duplicate の監査は維持され、lifecycle 同士の重なりと未知 tool は拒否する。横断参照先の情報は参考資料であり、書込み先はこの候補の canonical Wiki に限る。[監査分類](../../crates/services/src/services/openwiki/completion.rs#L196-L323)・[保守の正本](../operations/openwiki-maintenance.md)


## 公開事実の継続取得

IntegrationFacts は対象IDがある間5秒周期で取得する。succeeded・blocked・recovery_required でも未公開/収束待ちを表示し、終端statusのみで取得を止めない。公開受付と実Git適用の区別を維持する。[表示](../../packages/web-core/src/features/workspace-chat/ui/IntegrationFacts.tsx)・[既存試験](../../packages/web-core/src/features/workspace-chat/ui/IntegrationFacts.test.tsx)
