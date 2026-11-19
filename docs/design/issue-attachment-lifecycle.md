# 添付をIssueの寿命に揃える：棚卸しと段階移行案

## 結論と確定した制約

調査基準は `86f3058779b7ac032ed2c0c1444f76c06faf598d`。以下の「現行」はこのSHAを指す。今回の実装差分は後段で別記する。稼働DB・添付内容・秘密を調査していないため、既存実データの件数・リンクの曖昧さ・物理的な残存量は不明。

> ユーザーの確定方針：添付はGitに入れない、Issueと同じ寿命の一時資料。長期保存・複数Issueで共有する仕様書はリポジトリへ置く。Project共有ストレージ・全添付一覧・全Workspaceへの暗黙配布は作らない。

推奨は共通File基盤を維持し、Issue→Fileの参照を所有の正本にする段階移行。今回追加するのはIssue詳細からの保存・一覧・取得と参照保護だけであり、旧添付の自動再割当、削除、アーカイブ連動、入口統廃合は行わない。396との同時導入・408のテンプレート実験への合流はせず、単独でレビューする。

> [!DECISION]
> 後続移行で、未リンクWorkspaceの仮所有をいつIssue所有へ昇格するか。推奨は明示的なIssueリンク確定時だが、既存リンクの変更・解除時の扱いも一緒に決める。

> [!DECISION]
> アーカイブ時の中央実体削除を自動化するか、対象確認後の手動操作にするか。推奨は最初に手動の対象プレビュー・参照再確認・結果記録を導入し、自動化は同じ安全条件で後から限定する。

## 現行責務の棚卸し

パスは基準SHAのリポジトリ相対パス。Wikiは案内に用いたが、以下はソースで確認した。

| 責務・入口                 | 現行実装と契約                                                                                                                                                                                                                                                                                                                                                                                     | 移行時の扱い                                                                                                                                                                                                                                                                                                                                                           |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| UI・HTTPの単体アップロード | `routes/attachments.rs`（以下Rustのroutesは`crates/server/src/routes/`）。multipartフィールド名は歴史的な`image`。`/api/attachments/upload`で共通File保存、Workspace未所属もある                                                                                                                                                                                                                   | 保存処理を共通化しつつ、所有先を明示する。未所属を黙ってIssueへ割り当てない                                                                                                                                                                                                                                                                                            |
| 新規チャット添付           | `packages/web-core/src/shared/hooks/useCreateAttachments.ts`。単体保存→ドラフトにID・名前等保持→作成時にIDを渡す。`routes/workspaces/create.rs`とworkspace-managerが関連付け                                                                                                                                                                                                                       | Issue既知ならIssue所有へ。未知なら仮Workspace/ドラフト所有の期限と昇格を設計する。今回変更なし                                                                                                                                                                                                                                                                         |
| 既存チャット添付           | `features/workspace-chat/model/hooks/useSessionAttachments.ts`と`routes/workspaces/attachments.rs`。Workspace＋Sessionを照合、保存・Workspace関連付け・作業コピー。UIのscopeはhost/workspace/session、遅延応答の混入防止あり                                                                                                                                                                       | Sessionは配置先の確認に残し、所有正本はIssueへ。未リンクWorkspace互換が必要                                                                                                                                                                                                                                                                                            |
| Workspace一覧・画像参照    | 同routes。`workspace_attachments`を一覧。metadata/fileはSessionとWorkspaceを確認し、登録済みpathだけを中央から再配置。親ディレクトリとcanonical pathを検査                                                                                                                                                                                                                                         | 作業コピーと所有を分離。Issue全添付を暗黙に配置しない                                                                                                                                                                                                                                                                                                                  |
| UUID直接取得・削除         | `routes/attachments.rs`の`/{id}/file`とDELETE。基準SHAではFile UUIDを検索するだけでProject/Issue所属判定なし。入口認証とは別問題                                                                                                                                                                                                                                                                   | 新しいIssue専用Fileがこの経路で漏れない保護が必要。旧用途との共用は後述                                                                                                                                                                                                                                                                                                |
| remote MCP保存             | `remote_mcp/attachments.rs`→`routes/attachment_transfer.rs`。`lvk:read/write`とconnectionのProject allowlist、Project存在を確認。hostファイルまたはinline base64、request_id付きreceipt。保存してもplaced=false、runは開始しない                                                                                                                                                                   | LOCAL-267の既存契約。将来Issue明示の入口へ段階切替。今回旧schemaを変えない                                                                                                                                                                                                                                                                                             |
| stdio MCP保存・配置        | `crates/mcp/src/task_server/tools/attachments.rs`。inline入力、同じHTTP admissionへ転送。remoteのhost-file入力とは差がある                                                                                                                                                                                                                                                                         | adapter差は維持し、所有・検証は共通サービスへ寄せる                                                                                                                                                                                                                                                                                                                    |
| Project保存→配置           | `attachment_transfer::validate_ids/place_now`。`project_attachments`またはProject配下Workspace参照を許可。WorkspaceのIssue→Project関係とSessionを確認し、明示IDだけコピー、bytes検証後にWorkspace関連付け。部分失敗はper-file結果                                                                                                                                                                  | 恒久的なProject所有を追加しない。Issue所有へ切替後は別Issueからの横取りを拒否する                                                                                                                                                                                                                                                                                      |
| start/continue             | 同transfer、remote launch、stdio task_attempts/sessions→`remote_mcp/stdio.rs`の`/api/mcp-stdio/start-workspace`・`continue`。stdioはinternal actorを検査し、開始時に親IssueとProjectを解決してIDsを検証、継続時は最新終了Run確認・明示配置検証を経てExistingLaunch/ExistingRuntimeを呼ぶ。既存実行admissionとreceiptを利用、必要な配置が失敗したら実行に進まない。保存・配置・実行受理は異なる状態 | receipt/request_id、部分成功、実行との分離を維持                                                                                                                                                                                                                                                                                                                       |
| 中央実体・メタデータ       | `crates/services/src/services/file.rs`。cache/attachments、旧cache/images読取fallback。20MiB、名前空/255bytes超/制御文字/区切り文字拒否、拡張子からMIME。SHA256＋元名＋MIMEでdedup、同bytes別名は別File                                                                                                                                                                                            | FileServiceを再利用。hashだけを所有者にしない。元名とMIMEを保持                                                                                                                                                                                                                                                                                                        |
| DB参照                     | `crates/db/src/models/file.rs`、`20261029160000_project_attachments.sql`。`attachments`とWorkspace/Projectのjunction。旧task参照schemaはmigration履歴に存在するが現在のFile掃除では所有扱いしない                                                                                                                                                                                                  | Issue junctionを追加。既存junctionの破壊・推測移行はしない                                                                                                                                                                                                                                                                                                             |
| 作業コピー                 | FileServiceのcopy系、`crates/local-deployment/src/container.rs::copy_files_and_images`。Workspace rootの`.lvk/attachments`。既存宛先もbytes照合、編集済み・symlink・追跡ファイル等を拒否。Repoコピー設定と添付は別                                                                                                                                                                                 | 必要なWorkspaceへ明示配置。中央参照解除と作業コピー除去は別の操作                                                                                                                                                                                                                                                                                                      |
| 参照・履歴                 | prompt/markdownに`.lvk/attachments/...`、ドラフトにID/名前/サイズ、UI metadata経路と`attachment://`処理（`shared/lib/attachmentUtils.ts`）                                                                                                                                                                                                                                                         | 履歴文字列は保全。名前/サイズの履歴表示とダウンロード可能性を区別し、欠落時に別Fileへ解決しない                                                                                                                                                                                                                                                                        |
| 孤立掃除                   | db Fileの候補SELECT＋FileServiceの条件付きDELETEでWorkspace/Project/Pet参照を再確認。`crates/local-deployment/src/lib.rs`のバックグラウンド呼出あり                                                                                                                                                                                                                                                | 新Issue参照を両方へ追加。候補snapshotだけでは削除しない                                                                                                                                                                                                                                                                                                                |
| Pet                        | `NO_PET_ATTACHMENT_REFERENCE`がUI_PREFERENCESのattachmentIdを参照し保護。File基盤は添付以外にも共用                                                                                                                                                                                                                                                                                                | Issue寿命に巻き込まない。旧generic取得の契約も一律撤去しない                                                                                                                                                                                                                                                                                                           |
| archive/unarchive          | `routes/workspaces/archive.rs`、`routes/local_remote/issue_archive.rs`。確認・revision・idle検査、Managed worktree整理、履歴・branchは残す。直接folderの所有制約あり。解除で消えた実体が戻る保証はない                                                                                                                                                                                             | 現行はarchiveでWorkspace添付参照を解除せず、orphan判定もarchived flagを条件にしないため、その参照が残る中央実体は起動時の孤立掃除でも残る。Managed cleanupの`WorkspaceManager::cleanup_workspace_strict`はRepo worktree整理後にWorkspace rootを除去するため、root配下の作業コピーは別に失われる。中央FileをIssue単位で削除しない。今回も中央削除・所有解除を追加しない |

入口認証は`crates/server-access/src/lib.rs::gate`で`/api/`を一括保護し、web principal取得と書込みのCSRFを確認する。`routes/mod.rs`が新旧local APIを同じaccess layer内へ束ねる。local Boardは信頼されたチーム向けであり、remote MCPのconnection Project allowlistと同じACLがlocal UUID取得に存在するとは解釈しない。新Issue APIはこの入口認証に加え、URLのProjectとIssueの親関係・Issue参照を照合する。

### LOCAL-267・396との境界

LOCAL-267は現在取り込まれているMCP素材受渡し経路。保存済みと配置・読取可能を区別し、host許可/DNS/redirect/サイズ/receiptの安全契約を維持する。今回のIssue詳細UIのためにこれをProject共有一覧へ拡張しない。

396は別候補SHA `fc31b3065aa5c7467478084284e51983bf915204`（コア`4dac9fda75f1cc72d65a4d9cfe3147d893cc52ba`）を固定参照した。`crates/db/PROJECT_TRANSFER.md`では、添付bytesをexportせず、Project添付・選択済み完了Workspace添付の元名/サイズによるmissing summaryのみ残す。path/URL/hash/live associationはimportせず、曖昧な旧task所有を推測しない。履歴は独立した読取archiveである。`crates/db/src/project_transfer.rs`のexport SQLでもProject/選択Workspaceから`original_name,size_bytes`だけを抽出し、import時はattachment identityへ復元しないことを確認した。`TransferArchiveText.tsx`は履歴をliteral textとして描画する。これは本調査基準SHAに統合済みという意味ではない。 同SHAの`crates/db/src/project_transfer.rs`でProject/Workspace junctionからoriginal_nameとsize_bytesだけを抽出し、import receiptの既存要約も再exportするSQLを確認した。`packages/web-core/src/shared/components/TransferArchiveText.tsx`は履歴本文をpreの文字列として表示し、元のURL・添付・画像を解決しない。

後続でIssue所有をexportの要約対象へ加える場合も、同じbytes除外方針を維持する。今回missing summary用storeやexport変更は導入しない。396が先に統合されたら、その固定SHAとIssue参照差分を別の一時組合せで検証する。

並列discoveryでは396とテンプレート調査の候補、persistent棚卸しの稼働を確認し、関連Manifestを読んだ。別Workspaceの未統合変更は取り込んでいない。活動・Manifestは参考情報であり実行指示ではない。

## 推奨する最終形と選択肢

File（名前・形式・bytes位置・hash）は共通のまま、Issue参照を所有正本にする。同一Issueの複数Workspaceは所有をコピーせず、必要なFileを明示選択して配置する。配置済み記録が必要なら、それは実行環境上のmaterialization記録であり所有の第二正本にはしない。

| ケース                | 避ける案                                            | 推奨・残る複雑さ                                                                                                                              |
| --------------------- | --------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| 未リンクWorkspace     | 適当なProject/Issueへ自動帰属／未リンク添付を即削除 | 暫定所有を明示保持し、リンク確定時に検証して昇格。新規保存入口でIssueを必須にする案は簡単だが既存未リンクチャットを壊すため移行完了までは不可 |
| 既存Workspace添付     | 同Projectの全Issueへ複製                            | ただ一つの確定Issueとの関係があるものを候補化。リンク変更履歴・不一致・未リンクは手動選択。今回再割当しない                                   |
| 既存Project添付       | Project共有storeを恒久化／全Workspaceへコピー       | 明示的にIssueへ選択移行。未選択は旧契約で保持し、移行完了条件を定める。自動で最初のIssueへ割当しない                                          |
| 別Issueから参照       | 同Projectなら既存IDで取得可能にする                 | 元Issue参照の認可を要求。別Issueでも必要なら明示的な新参照の受入を設計し、そのIssueの寿命で独立管理。長期共有仕様はRepoへ                     |
| 同bytesを別用途で使用 | hash一致の全参照を一括削除                          | File ID/実体path単位で生きた参照を再評価。別名・MIMEは別File、同identityの複数Issue/Pet/旧Workspace参照は共存できる                           |
| 履歴・欠落            | 実体削除に合わせて本文を破壊／別の同名Fileを解決    | 履歴は維持。元名/サイズ・欠落状態の保持案を別途設計し、取得不可と物理削除済みを区別                                                           |

### 1本化できる箇所と残す契約

最終的には所有検査・保存＋関連付け・一覧・明示配置をIssue attachmentサービスへ集約し、UI/remote/stdioは薄い入力adapterにする。型は共通AttachmentResponseを継続利用し、所有contextと配置receiptを区別する。古い`uploadForTask/getTaskAttachments`のクライアントやtask migrationは利用者確認後に撤去候補とする。入口の実際の使用有無はコード検索と利用者移行で確認し、呼出回数を実DBから推測しない。

移行完了後にProject所有junctionと旧Workspace所有admissionを廃止可能。WorkspaceのSession検査、配置パス安全性、bytes検証、runを開始しないsave/place、receiptの再試行契約、PetのFile参照は残る。共通File、MIME安全応答、中央cache、Repoのcopy_files設定はIssue固有機能ではないため削除対象にしない。

恒久的なdual-writeは避ける。旧読取adapterがある期間を明記し、新入口はIssue正本のみへ書く。今回追加するjunctionは、新UI専用所有の正本であり旧Workspace/Projectへの同時書込みはしない。

### 切替手順（後続実装の案）

1. 配置計画では、schema追加だけを後方互換と呼ばない。新Issue-only添付の作成開始後に旧binaryを起動すると、旧orphan掃除はIssue参照を知らず削除し得る。Issue参照を理解する版だけが中央storeを扱う切替条件を定め、旧版への単純rollbackや新旧混在writerを避ける。今回配置・rollbackは実行しない。
2. 今回の最小スライスを単独レビュー。旧ID/path/型を維持したままIssue詳細の保存・一覧・取得を追加し、両cleanup判定にIssue保護を入れる。
3. データを変更しない分類プレビューを設計。明確な所属、未リンク、複数候補、Pet共用、欠損、同identity共有を区別し、確認対象とrevisionを固定する。実データの分類は別途許可された段階で行う。
4. 新規チャットとMCPにIssue contextを明示し、旧request schema/receipt互換adapterから正本サービスへ接続。未リンク仮所有の昇格・リンク変更契約を先に決める。
5. 再実行可能な明示移行。元参照が変わっていないことをtransactionで照合し、Issue参照が確定してから旧所有を解消。途中失敗のjournalと再試行を用意し、両経路を恒久運用しない。
6. 旧書込みを停止し、旧読取・型・storeを利用者ごとに撤去。期限だけを根拠に未解決データを削除しない。
7. 削除を伴うarchiveは別レビュー。396のmissing summary整合もこの時点で固定SHAによる組合せ検証を行う。

## 削除・archiveの設計比較（今回実行しない）

自動archiveが触れてよい範囲は、そのIssueだけに帰属する中央実体と、その生成Workspaceに明確に所有される作業コピーに限定する。他の生きたIssue、未リンクWorkspace、Project legacy、Pet、稼働runの参照があれば残す。手動確認では曖昧な旧所有の選択もできるが、確認があっても他の生きた参照や所有範囲を無視して削除してよいわけではない。

Issue archiveとWorkspace archiveを同一視しない。複数Workspaceの一つを閉じただけでIssue添付は削除しない。Issueだけarchive・Workspaceを残す選択もあるので、稼働/再開可能環境に必要なbytesは保護する。自動Integration成功後の生成Issue/Workspace整理も同じ候補検査を使い、成功statusのみを削除許可にしない。

履歴の元名/サイズを保持する案は、削除前のimmutable snapshotまたは既存履歴からの表示が候補。新しい二重storeを先に作らず、396の要約との責務を比較する。tombstoneは「取得不可」「削除を試行」「削除確認済み」「失敗」を区別する必要がある。

並行保存・新参照追加・archive解除との競合では、候補抽出は権威ではない。所有revisionとIssue状態を確認し、writer排他下で最終参照判定と削除intentを永続化する。ファイルシステムとSQLiteは一つのtransactionでrollbackできないため、DB行を消してからunlink失敗するとpathを失う現行方式を、archive削除の根拠にはしない。再試行可能なintentと対象path保持、unlink/DB確定の結果記録・確認手順が必要。

削除のretryは元operationと同一対象で行い、別の生きた参照が増えたら停止する。symlinkやDirectFolderの非所有path、他Repoや共有cache全体は対象外。バックアップにbytesが残る可能性は別管理であり、中央unlinkを完全消去と呼ばない。既存backupを今回読んだり消したりしない。

解除時は履歴を再表示できても、削除済みbytesは戻せない。再添付は新たな明示操作とする。部分削除・commit不明は通常の成功/未実行に丸めず、元receiptと実状態を調べる回復手順を用意する。

## 今回の実装範囲と最小試験

追加するIssue参照は親Projectからscopeを判定し、既存FileServiceの名前・サイズ・MIME・dedup契約と入口認証を利用する。Issue AのFile IDをIssue Bや別ProjectのURLへ渡しても取得できないことを確認する。新Issue専用Fileはgeneric UUID取得・削除と旧Workspace関連付けからの迂回を拒否する。すでに旧Workspace/Project/Petで使われる共通Fileは既存取得契約を維持するため、その同じbytesをIssueに添付しても旧経路までIssue認可へ変わるわけではない。この既存公開範囲の縮小は後続の入口移行で扱う。今回Issue API自体は常に親ProjectとIssue参照を検査する。 新Issue-onlyと同じbytes・元名・MIMEを旧チャットへ再アップロードするとdedupで同IDになり、旧Workspace関連付けを拒否する場合がある。既存Workspace添付自体の取得は維持するが、新Issue-onlyから旧経路への新規共有は今回対応しない。所有昇格・明示配置を実装する後続段階で、bytesを再送した場合の受入契約も解決する。

保存とIssue参照追加の間はFileService clone間で共有するmutexで当該サービスの掃除・直接削除と排他する。DBのFKと親再確認で参照消失時は成功を返さない。これは単一server内の窓を守る最小策で、別FileServiceインスタンス/別プロセスwriterを跨ぐ分散排他ではない。後続のarchive削除にこのmutexだけを流用しない。

受入試験は隔離fixtureのみ：Workspace未作成のIssueへ保存→孤立候補/掃除→一覧・取得成功、別Issue/Project拒否、両掃除判定の参照保護、同identityの別参照とPet保持、保存/関連付けと掃除の競合、名前・形式・サイズの既存制約、Issue切替後の遅延応答混入防止、選択・一覧・エラー表示。実DB/実添付の掃除はしない。

本書は設計・調査成果物。個別チェックの実行結果、実装SHA、未実施のブラウザー/実環境検証は完了報告で分けて示す。

### 既存MCPの検証fixtureに残る差分

既存opt-inの`isolated_attachment_remote_stdio_e2e`は、基準SHAのままでは現在のProject Repo設定・Workspace共通root・stdio HTTP routeに追従していない。今回、最初の二点をfixtureのみ修正したが、stdio開始のrouteは未登録のまま。remoteの保存・配置・開始・継続とstdioの保存・配置まで進んだ後、stdio開始でnot_foundとなったため、E2E全体の成功証拠には使わない。現`remote_mcp/stdio.rs`は既存のNativeFixture注入点を持たず、単に本番routerを登録すると実agentを起動し得る。今回はその操作をせず、stdioの開始・継続まで安全に一体検証できるfixture注入の再設計を残す。これを本番経路の互換不具合と断定したり、mockで本番handlerを置き換えて成功と呼んだりしない。
