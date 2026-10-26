# LOCAL-81 可読性・Workspace状況表示 / Change Manifest

2026-10-25。実装・局所試験・commit承認済み。main merge/deployは実施していない。
独立レビューは上位AI。プレビューBasic設定後の認証済み確認はユーザー操作待ち。


## 2026-10-26 DISK整数表示・runtime復旧準備（最新追記）

- 開始00:55 JST。Issue実読取revision `2026-10-25 15:55:11.822` の末尾にDISK整数丸め承認を確認（指定15:54:28より新しい）。base `0249fc27477498ed4c6f3df0d2621231d75ec90d`。
- ResourceSummaryの容量formatterのみ整数GBへ変更。直接試験するため既存resourceObservation libへformatDiskCapacityを移し、`Math.round(bytes / 1e9)`を使用。900.25GB→900GB、1053.49GB→1053GB。nullは既存の—。内部bytes/使用率/floor%/色閾値/unknown判定に変更なし。
- `node --test tests/resource-observation/model.test.mjs`: 6/6成功。丸め直前/丁度/直後、0、小容量、null、欠落/stale、raw bytes/percent不変、既存70/80閾値を確認。変更formatter/componentへの局所eslint成功。既存browser/full-appの期待文字列だけ更新、node --check成功。Rust/再build/全体E2Eは未実施（不要）。
- UI担当より00:54:59 Stop→00:55:12 Start後に失敗したとの報告。00:56時点で18444/18446 listenerなし、runner構文/実パス/既存DB存在は正常。実ログのエラーはUI担当の共有待ち。
- runtime事前bind probeはTIME_WAITだけでerrno98になり得ることを一時loopbackポートで再現。00:57にLISTEN-onlyのss判定へ局所修正、最終bind競合は各serverが自然失敗。構文確認成功、実ポートは空き。起動API/独自daemonなし。再Start1回を正規UI担当へ依頼済み。DB/認証/keys/seedを変更しない。
- 切替後の外部WSS/API認証/HMR/認証済みLOCAL35タグ/Run終了後持続は未検証のまま。旧launchへの正規UI rollback手順は下記を維持。

## 2026-10-26 追加UI修正とVite切替準備（最新）

- 作業開始00:37 JST、準備完了00:46 JST。base `98a697f5b6d863c5173192d2a65fba862acd57ed`。Issue読取revisionは `2026-10-25 15:37:42.863`（指定15:08より新しい。追加承認と即時反映承認を確認）。
- 製品変更: タグはタグ変更・利用可能幅変更時のResizeObserverで先頭から表示可能数を決定し、残数+Nを確保。単独長タグはtruncate、既存popover triggerを維持。測定用要素はaria-hiddenかつ画面外へ溢れない枠内。新pollなし。
- Workspace Dev Server背景は既存infoテーマ値の淡青、通常背景と排他的に設定。既存Playに翻訳済みtooltip/accessible label。AgentRun状態とは独立。showWorkspaces=trueのときIssue集約のdevServerだけ抑制。
- 試験: `pnpm --dir packages/ui run check`、`pnpm --dir packages/web-core run check`、`pnpm --dir packages/local-web run check`成功。変更4UIファイルへのeslint成功。`node tests/kanban/readability.mjs`で220/300/600px往復、0/1/複数/長タグ/+N、light/dark、popover Enter/Escape/フォーカス、表示ON/OFF、DevServerとrunning/approval/failed共存、既存DnD/本文回帰成功。fixture経路であり実ユーザーDBの検証ではない。
- 開発中に背景class競合をブラウザ試験で発見して修正。test locator修正とpopover focus待機も実施。最終成功を成果物へ保存。不要なRustビルド・全体試験なし。
- プレビュー運用は製品変更と別: `.build/preview2-vite/` に専用Vite設定・foreground親runner・検証手順を保持。`.lvk-preview/start.sh`はrunnerをexecするだけ。共通repo設定・nginx/Compose/DNS・dev/stableには変更なし。運用launcher差分を製品mainへ取り込まないこと。
- 実Vite 7.3.1。root/frontendソース/shared/depsだけfs.allowし、.build/.lvk-preview/.git/.env/DB/鍵/tokenをdeny。publicDir全ファイル名/種別を監査しsymlinkなし（画像/SVG/既存Service Worker/robots/manifest）。publicDirにはfs.denyが効かないため別監査。
- Host許可はpreview2.luckyvibekanban.comのみを指定、loopback/strictPort、HMR wss/clientPort443。既存/api proxyのchangeOrigin:false/ws:trueを保持。Vite18444、backend18446、同じpublic/basic/public URLと既存instanceを指定。envは最小allowlistで渡し、秘密の読取・複製・token生成なし。
- 隔離Vite18447から既存backend18444への非変更試験: root/health200、未認証API401、不正Origin403、未認証API WebSocket401、不許可Host403。非秘密canary /@fs と?rawは403。通常パスはSPA fallback200だがcanary内容なし。source内一時canaryの書換えでHMR before→afterをreloadなしで確認、削除済み。Basic login画面表示、JS pageerrorなし。local routingであり外部WSS確認ではない。
- 現存DBはinstance-FTz2vi/data/db.v2.sqlite、inode31033808を保持。アプリ稼働/ユーザー操作でsize・mtimeは変わるためbyte不変とは主張しない。本作業はDB/認証/key/seedを変更・再作成・コピーしない。
- **切替前ready（00:50 JST）**: launcher/configは準備済み。backendは127.0.0.1:18446（空きを確認）、Viteは127.0.0.1:18444/strictPort。既存preview2のDB/Basic/session設定を同一instanceから継続利用する。まだStop→Startは実施していない。
- **停止方式**: 正規LVK DevServerがforegroundのPython親runnerを所有し、Rust/Viteはその直接子。親はSIGINT/SIGTERMまたは片側終了を検知するとfinallyで両子へterminate、各8秒で終了しなければkillしwaitする。バックグラウンドdetach/nohup/setsidなし。運用停止時の両port解放はUI切替後に別途実測する。
- **rollback**: 正規UI Stopで両子停止を確認後、保存済み旧static launcherを元のWorkspace launcherへ戻し、正規UI Start。元の既存Rust binary/instance/18444を使う。DB/key/authの復元やコピーは不要。共有repo commandとhost/nginx設定は変えない。
- UI担当に正規Stop→Startを依頼済み。切替完了までは旧server PID2143653/18444のまま。外部認証済みLOCAL-35追加タグ/実WSS/CSRF/session/Run終端後持続は切替後の上位AI確認待ち。秘密を要求しない。旧launcherは`.build/preview2-vite/start-static.rollback.sh`に保全、rollbackも正規UIで実施。
- 成果物: `.build/kanban-readability/{results.json,additional.log,tags-devserver-light.png,tags-devserver-dark.png}`、`.build/preview2-vite/{validation.json,validation.log,local-vite.png}`。一時Vite/browser/canaryはfinallyで終了/除去。稼働previewと必要なruntime設定は保持。

以下は前回実装時の履歴であり、旧PID/locked表記は現在状態ではない。

## Revision・SHA照合

- 初期HEAD: `455f42d2c77b3f512a8346be2c7b1a5294ea6d7e`。作業ブランチだけを初期main `271aa60f14dd1ac94b2cf2a9fe0e1878bef3d2ce`へfast-forward。
- 前回応答時点は未commitと報告したが、再開時のgit log/statusでは実装commit `21d9ff05b045249a0ff59d24530adec25a6b5644`、cleanを確認。これを今回のsource/test起点とする。
- 前回UI fixture検証のソースSHA-256と21d9ff05の製品ソースは一致。
- E2E後は検証記録更新とテストのPrettier整形のみ。整形後`node --check tests/kanban/isolated.mjs`成功。実行時test hashはそのまま保持し、製品ソースhashの一致を最終commitでも照合する。
- 今回のRustテスト・serverは21d9ff05の製品ソース。実API E2Eは同SHA＋今回のテスト追加。server build_infoは21d9ff05、development、dirty=true（テスト変更あり）。製品ソース変更なし。
- binary SHA-256: `45571b887be67d162960114b06627d57d1343052fe024422cfbf7cd3f6d907ae`。
- 最終commit SHAは成果物のfinal-revision.jsonと最終応答に記録。tested-source-hashes.jsonで製品/試験ソースを照合する。SHA表示だけのための重複ビルドは行わない。

## 変更内容

- KanbanCardContentの既存Markdown整形を再利用し、タグ後に本文最大2視覚行。空本文は余白なし、長URL折返し。details開閉中もpreviewを維持。
- Workspace一覧をdetails外へ移動。showWorkspaces・非archive範囲・順序・Workflow除外を維持。本文全文はdetails内。
- カードIntegration行と専用3秒polling/focus状態を削除。Navbar/store経由のIntegrationPanel、同Panelのqueryと正式機能は維持。
- BoardのNULL列ではなく既存localWorkspace summaryへ差分を配線。正常0とunknownを区別し、時間はChatElapsedTime、未知は—。既存canonical境界・15秒pollingは変更なし。
- Workspace stats/PRは狭幅で折返し。共有IssueWorkspaceCardの他利用画面にも表示改善が適用される。
- services内部DiffStatsの行数Optionを保ち、任意repo失敗で全体None。binary/内容省略はfilesへ数え、行数不明を伝播。既存nullable WorkspaceSummaryへ直接渡すためAPI/生成型/SQL変更なし。
- Gitの片側内容不明を追加/削除と誤認しない（実Added/Deletedのみその側を0扱い）。merge-baseとworking tree差分の取得方法は維持。
- 4状態、最大2列、文字サイズ、DnD/順序処理、新API/設定/件数上限/masonryは変更なし。大量Workspace時の高さ・隣の空白は合意どおり許容。

## 検証コマンドと結果

Rust共通環境: `CARGO_BUILD_JOBS=2`、`CARGO_TARGET_DIR=$PWD/target`、`RUSTC_WRAPPER=/usr/local/bin/sccache`、`SCCACHE_DIR=$(readlink -f .lvk-shared/cache)/sccache`。
重コマンドは順次実行。共有target/cargo clean/無関係な全体check/lint/backend:testなし。
sccache実際の保存先一致を前後確認。統計は共有利用者込みであり本試験だけのhit率とは扱わない。

| コマンド | 結果 |
| --- | --- |
| `pnpm run web-core:check` / `ui:check` / `local-web:check` | 前回PASS。製品ソースhash一致を今回確認 |
| 変更UI3ファイル・Container/Details2ファイルのESLint | 前回PASS。同ソース |
| `node tests/kanban/readability.mjs` | 前回PASS。同製品ソース・本部品fixture |
| `cargo test -p git --test diff_stats` | PASS 1件。commit済み/未commit/untracked、text↔binary、削除 |
| `cargo test -p git --lib summary_read_failure_tests` | PASS 1件。Modified読取欠損を削除扱いしない |
| `cargo test -p services summary_diff_unknown_and_multi_repo_failure` | PASS 1件。隔離SQLite/2repo、0・正数・binary・部分/全repo失敗 |
| `cargo build --bin server` | PASS。serverまでの消費側コンパイル確認 |
| `pnpm --dir packages/local-web run build` | PASS。Browserslist古さとchunk-size警告あり |
| `LVK_TEST_SERVER=$PWD/target/debug/server LVK_KEEP_TEST_INSTANCE=1 node tests/kanban/isolated.mjs` | 最終PASS。初回追加testは既存Dialogにないroleを参照して失敗、実際のheading参照へ修正 |
| `python3 tests/kanban/seed-preview.py` | PASS。停止済みの自分のfixture DBへ0/1/4/12 Workspaceサンプルを追加 |
| 外部HTTPSとChromium | PASS: root200、mode=locked、public、forced_mode=basic、業務API423、セットアップ画面。異なるoriginのloopbackアクセス403も確認 |
| `git diff --check` | PASS |

## 実APIとfixtureの区別

実API E2Eは変更ソースserver＋ソースVite、新規SQLite、一時Git repos、隔離Chromiumを使用。
実summary APIで3 files/+3/-1（commit済み＋未commit＋untracked）と0 files/+0/-0を取得し、実カードに配線されたことを確認。
実APIでIssue/Workspace1:N、詳細、列間DnD・並べ替え保存、keyboard/focus、Navbar Integration入口、全4状態の配置を確認。

2560×1440 CSS px/DPR1.5/zoom100%（4K・OS150%相当）、1280×900、390×844、各列535/536/537/536/535px往復。
カードタイトル16px・ellipsis維持、body2行、0/1/複数/多数Workspace、折返しと詳細全文は部品fixtureでも確認。
実行中/承認・入力待ち/失敗等はsummary応答overlayと部品fixture。実AgentRun/モデル・認証遷移の実行証拠ではない。
日本語lightと英語darkスクリーンショットを目視確認。

## preview2・operator手順

- URL: `https://preview2.luckyvibekanban.com`（現在locked、アプリ利用はBasic設定後）。
- Project: `47ce1fc7-9795-4787-b616-9380733543de`。
- PID: `1809660`、bind: `127.0.0.1:18444`。`/proc/PID/exe`とssで実binary/portを確認。
- binary: `/var/tmp/lvk-instances/dev/runtime/worktrees/2f33-2/lucky-vibe-kanban/target/debug/server`。
- instance: `/var/tmp/lvk-instances/dev/runtime/worktrees/2f33-2/lucky-vibe-kanban/.build/kanban-validation/instance-FTz2vi`。
- DATA_DIR: 上記instanceの`data/`。SQLiteは`data/db.v2.sqlite`、共有/live DBや認証DBからのコピーなし。
- `LVK_ACCESS_DEPLOYMENT=public`、`LVK_AUTH_MODE=basic`、`LVK_PUBLIC_URL=https://preview2.luckyvibekanban.com`。既存nginx/TLS/DNS/Auth0設定の変更なし。
- 保持DBは4状態のIssue、Workspace0/1/4/12と実Git差分2workspaceを含む。AgentRunは作成しない。状態アニメーションは試験fixtureのスクリーンショットで確認する。

ユーザー/operatorだけがterminalで次を実行する。出力tokenはブラウザへ直接入力し、チャット/ログへ貼らない（teeやログ保存を付けない）。この作業では発行していない。

```sh
LVK_INSTANCE_DIR=/var/tmp/lvk-instances/dev/runtime/worktrees/2f33-2/lucky-vibe-kanban/.build/kanban-validation/instance-FTz2vi \
  /var/tmp/lvk-instances/dev/runtime/worktrees/2f33-2/lucky-vibe-kanban/target/debug/server setup-token
```

1. 上記URLを開き、発行tokenでセットアップを解錠する。
2. Basicのユーザー名と12〜1024 bytesのpasswordをブラウザ内で設定し、ログインする。
3. 認証済み状態でProjectを開き、カード表示を確認する。secretを渡さず、設定完了を上位AIへ知らせる。

根拠: `crates/server/src/main.rs:58`のCLIは指定instanceのDB/鍵へ発行して終了し、serverを追加起動しない。
`crates/utils/src/instance.rs:5`でinstanceを隔離解決。`crates/server-access/src/store.rs:105`の未設定publicはLocalにfallbackせず、`:217`で業務API423。
`handlers.rs:79`でtoken交換、`:214`で管理sessionによるBasic保存、`store.rs:392`でpassword制約。Auth0変更は不要。

## 保持・cleanup・未検証

- 自分のE2E backend/Vite/browserは終了。公開確認browserも終了。preview serverのみユーザー確認まで保持。
- 保持物: 専用instance約2.2MiB、生成frontend約77MiB、Workspace target約18GiB（再ビルドcacheと稼働binary）。削除/上書きしない。プレビュー完了後も自身のPID/exe/portを照合してから自分のserverだけ停止し、専用instanceを整理する。
- スクリーンショット・logs・results/revision/hashを `.build/kanban-validation`、`.build/kanban-readability`、`.build/kanban-rust`、`.build/kanban-preview`とpersistent artifactsに保持。DB/認証鍵/tokenは成果物コピーに含めない。
- Basic設定後の認証済み外部確認は未実施（ユーザー設定待ち）。実AgentRun/有料モデル、実機touch、Firefox/WebKit、全Workspace/untracked大量走査の性能実測、独立レビューは未実施。
- Rust workspace全体試験/clippyはこの局所変更の検証に追加していない。Formal Integration/main merge/deployは未実施・未承認。

## 2026-10-27 追加4項目（LOCAL-81 / Change Manifest）

### 範囲・参照

- authoritative: Issue最新revision `2026-10-26 17:48:24.917`（追加4項目の元revision `2026-10-26 15:44:15.913`）と当run中の確定回答（明示ソート中は裏側の永続順序更新も停止）。追加4項目の実装・局所試験・commitのみ。main merge/deploy/DevServer操作なし。
- 開始HEAD `c456e1062e94d87dc4142722ff9e013ef0b03f64`。cleanを確認し、local main `45fdda98d586940ef45411ac6fcd2872e9143fad` を通常merge。取り込みcommit `63ff87ae98b32599bf3776aed9040e72bef8cd83`（競合なし）。件数表示を保持。main refは操作していない。
- 試験対象は取り込みcommit上の今回の変更ツリー。最終source SHAと時刻は完了報告およびpersistent evidenceのrevision.jsonに記録する。

### 実装とtrigger棚卸し

1. `db::models::issue_order::promote` は当該Issueの現在のproject/status列内の先頭rankを1件だけ更新する。全件再整列なし。既存UI_PREFERENCES scratch（固定ID末尾1）のproject別選択view/filtersを同じtransactionで読む。team既定は手動、personal既定はpriority。title/priority/created_at/updated_at等の非手動sortではUPDATEを行わない。後追いイベント再生もない。手動descendingでは最大rank+1、ascendingでは最小rank-1。新API/設定項目/migration/pollingなし。設定は既存共通scratchの保存済み値が基準であり、ブラウザセッション間でも既存共有設定に従う。
2. UIの本文更新→local_remote::update_local_issue(_checked)。board API/stdio MCPは既存local API client経由、remote MCP issue_writeもchecked関数経由。descriptionフィールドが含まれる保存（同一本文、nullクリアを含む）だけを対象とし、title/priority/tagのみは対象外。保存と順位更新はtransaction内、status同時変更後にpromote。変更されないstatus/orderを古い読み取り値で上書きしない。
3. 新規コメントはUI local_remote::create_issue_comment と remote_mcp::issue_comments::add_issue_comment のINSERTと同じtransaction内でpromote。編集・削除・reactionは対象外。actorによる除外なし。remoteの同request_id再送は既存receiptを再利用しINSERT/promoteを重複させない。順位だけの更新はupdated_atを変更せず、既存の本文review revision契約を保持する。
4. Agent terminalはappend_and_project / append_and_project_host_batch双方が通るsync_kanban_lifecycleで、ReducerApply::AppliedのLifecycleChanged初回終端のみ。succeeded/failed/cancelled/crashed/audit_failedの全5状態、executorによる除外なし。新attemptの初回終端も対象。既存status集約処理は変更せず、その後同transactionで現在列にpromoteするため、開始証拠なしの失敗やcustom列も順序対象となる。重複イベントと同attemptの終端再通知はreducer/previous状態で除外。Message等stream更新、projection再構築、Runの単なるログ更新は対象外。
5. 描画配列の再同期をドラッグ中は保留。ドロップは対象1カードのrankのみ保存し、他カードを一括renumberしない。ドラッグ中に受信した対象カードのpromotionはdrop先で先頭rankにする。キャンセル時は既存位置に戻して保留した同期を再開。既存同期待ち完了時にも再同期する。他カードの昇格でfrozen隣接境界が反転する場合は昇格済みカードを境界から除き、昇格prefixの後ろで残る手動順序への挿入位置を計算する。既存bulk更新に任意expected_sort_orderを添え、transaction内の読取で差異があれば古いrankを上書きしない。列間移動時の差異は新しい列へpromoteする。新endpoint/永続項目なし。explicit sortの選択/view切替は既存scratch保存をdebounceせず即時送信（その他の設定は従来どおり）。
6. DnDはdnd-kit DragOverlayをdocument.bodyへportal。元カードはレイアウト位置を維持して透明にし、列のcontainer containmentにghostが切られない。overlayはaria-hidden/inert、実カード操作とkeyboard focusを維持。
7. Priority/担当者は既存部品をLOCAL-ID行右端へ移動。独立行を削除。IDのみtruncate/title補助、font/列幅は不変。既存操作に明示的なaria-labelを追加。

### 集約文字行の全候補・全経路

削除対象は `KanbanCardDetails` のflags→Object.entries/filter/map→aria-live行のrender全体。候補は `approval`（pending flag/awaiting_approval）、`input`（awaiting_input）、`running`（isRunning）、`failed`（failed/crashed/audit_failed）、`unseen`（hasUnseenActivity）、`devServer`（旧showWorkspaces OFF分岐）の6種。実画面の呼び出しはKanbanContainer、fixture呼び出しはreadability.fixture。両呼び出しからworkspaces/showWorkspaces propも削除し、ON/OFFとも行・空余白を残さない。本文全文detailsとWorkspace数は保持。Workflowカード等の別表示は対象外。

Workspaceでは既存淡青背景とPlay（DevServer）を維持し、状態領域で独立表示。Agentは承認→入力→実行中の既存注意状態を優先し、失敗はTriangle。終端の既読/未読は別Circleで失敗表示とも共存。未知のRunを既読とみなさない。全アイコンにtooltip/aria-labelを用意、文字行は追加しない。

### 局所検証・証拠・限界

- UI: `pnpm run web-core:check`, `pnpm run ui:check`, `pnpm run local-web:check` 成功。UI変更2ファイルのESLint成功。局所Prettier/rustfmt/diff checkを使用。
- `node --test tests/kanban/order.test.mjs`: 最終4件成功（前/中/後/空/降順、ドラッグ中の本人/他カード/複数promotion、隣接境界反転、昇格prefix、別列からの昇格、他カードrank不変）。
- `node tests/kanban/readability.mjs`: 独立Chromium component fixture成功。2560×1440 CSS/DPR1.5（4K150%相当）、1280/390、列535↔536境界、カード幅240/260/340、長ID、Priority/担当者keyboard操作、既存タグpopover、ON/OFF集約行なし、light/dark、Run/approval/input/failed/unseen/read/DevServer同時表示、ghost列間連続・drop・Escape・focusを確認。ブラウザはfinallyで終了、HTTPサーバー不要。
- `node scripts/dev-cargo.mjs test -p db kanban_ --lib -j 4`: 最終14件成功。全5terminal/両保存経路、stream非対象、重複終端、明示sort中のrank不変と通常status遷移を確認。
- `node scripts/dev-cargo.mjs test -p server --lib issue_activity_body_only_and_destination_order -j 4`: 初回1件成功、最終の下記server8件にも含めて再確認。
- CargoはLOCAL-197入口・既存target/共有sccacheを使用。共有build-dirの正規ロック待ちは尊重。観測available RAM約12GB/pressureほぼ0、jobs4で1重commandずつ。clean/共有cache削除なし。
- スクリーンショット: `.build/kanban-readability/tags-devserver-light.png`, `tags-devserver-dark.png`, `cross-column-ghost.png`。component fixtureの証拠であり、実認証previewや実API E2Eの証拠ではない。保持コピーは `persistent/artifacts/59276488-93fa-4092-bdd2-33a86739203c/local81-additional/`。
- preview2は今回起動/停止/変更していない。両port18444/18446待受なし。DB/Basic/session/keys/ユーザータグ/runtime launcher未操作。実previewでの確認、外部認証・HMR・実API E2E、全体check/lint/backend:testは今回未実施。
- 件数表示の実装はc456e106を維持。今回のcomponent fixtureの見出しは実KanbanContainerではないため、実アプリの件数/検索/アーカイブ操作のブラウザ再検証は未実施。
- 同時操作はDB transactionの確定順と既存scratchの保存成功時点が基準。複数ブラウザの異なるsort選択、通信失敗中の設定変更は実ブラウザで未検証。drop計算後の本人promotionは期待rank照合で保護し、DB確定の両順序を局所試験する（実ネットワーク遅延下の同時browser試験は未実施）。SQLiteの同時read→write昇格競合では更新失敗となり得るが、古いrankで成功扱いに上書きしない。新しい分散順序/ロック機構は追加していない。

- 実行分担更新: ユーザーは上位ownerへ統合・配備build・sccache停止・DEV再起動も承認したが、このrunは局所実装/試験/commitまで。専任ownerの操作を代行しない。

- `node scripts/dev-cargo.mjs test -p db --lib models::issue_order::tests -j 4`: 最終2件成功。初回にSQL CASE追加時のbind不足を検出し修正、再試験で正常0件更新を防止した。明示4sort停止・personal既定・manual降順・他Issue順序不変・rollbackを確認。

- 独立確認で指摘されたA0/B1/C2/D3→B=-1中のD挿入は再現して修正。B/A/D/Cとなること、AとBの複数昇格、降順、先頭挿入を純粋rank試験で確認。drop APIの既存readもtransaction内へ寄せ、古い本文などを同時dropで書き戻さない。

- 最終server再検証で一度E0433（ローカルに存在・公開されDB試験済みのissue_orderを依存DBから解決できない）が発生。直前の他Workspaceと共有build-dir利用後にDB compileが省略されていたため中間成果物の取り違えを疑い、自Workspaceのmodels/mod.rsのmtimeだけを更新して依存再評価を要求。cache削除/clean/他Workspace操作は行わない。再試験結果は以下へ記録。

- 同じE0433がmtime再評価後も再現したため再試行を停止。この時点のDnD競合server回帰は未完了として区別し、下記の限定例外で完了。上位調整AIから今回限定のlocal build-dir例外を許可済み（question answer af1942b1-03ae-4dec-88de-839f95f63762）。LOCAL197入口はconfig/target overrideを明示拒否するため、入口本体変更を避け、下記一時コピーでrootとbuild-dirだけを置換して同じprofile/sccache wrapperと検査を維持。

- 例外実行: `node .build/local81-local-cargo/dev-cargo.mjs test -p server --lib -j 4 -- issue_activity_ remote_mcp::issue_comments::tests`。既存LOCAL197入口の一時コピーはroot解決をこのrepoに固定し、build.build-dirだけ `target/local81-validation-59276488` に変更。入口本体/AGENTS/default/共有cacheは未変更、profile/検査/既存sccache wrapperを維持。2行の差分と入口SHAをpersistent証拠へ保存。開始前available RAM約10GB、disk598GB、memory pressure avg10=0、swap out=0を観測してjobs4。server lib局所試験のみ、本体server buildなし。

### 今回の変更パス

- `crates/db/src/models/agent_runtime.rs`
- `crates/db/src/models/agent_runtime_kanban.rs`
- `crates/db/src/models/agent_runtime_kanban_tests.rs`
- `crates/db/src/models/issue_order.rs`
- `crates/db/src/models/mod.rs`
- `crates/server/src/remote_mcp/issue_comments.rs`
- `crates/server/src/routes/local_remote.rs`
- `packages/ui/src/components/IssueWorkspaceCard.tsx`
- `packages/ui/src/components/KanbanCardContent.tsx`
- `packages/web-core/src/features/kanban/ui/KanbanCardDetails.tsx`
- `packages/web-core/src/features/kanban/ui/KanbanContainer.tsx`
- `packages/web-core/src/features/kanban/ui/KanbanGrid.tsx`
- `packages/web-core/src/features/kanban/ui/kanbanOrder.ts`
- `packages/web-core/src/i18n/locales/en/common.json`
- `packages/web-core/src/i18n/locales/ja/common.json`
- `packages/web-core/src/shared/hooks/useUiPreferencesScratch.ts`
- `packages/web-core/src/shared/lib/remoteApi.ts`
- `tests/kanban/READABILITY-VALIDATION.md`
- `tests/kanban/isolated.mjs`
- `tests/kanban/order.test.mjs`
- `tests/kanban/readability.fixture.tsx`
- `tests/kanban/readability.mjs`

### 最終結果（2026-10-27 03:25 JST時点）

- 限定local build-dir例外の上記server局所コマンド: **8 passed / 0 failed**、ビルド7分04秒・試験11.46秒。本文/コメントのexplicit sort中rank不変、comment再送/認可/review revision、遅延promotion→dropとdrop→promotionの両確定順序、移動先top/隣カードrank不変、bulk期待rankのdeserializeを確認。E0433はこの出力先で解消。依存記録にはissue_order.rsを確認。
- DB: issue_order 2件 + kanban_ 14件成功。frontend rank 4件成功。最終web-core型成功、UI/local-web型とUI2ファイルlint成功、Chromium component fixture成功。局所format/diff check成功。
- Rust Clippy/全体check/lint/backend:test、実API E2E、認証済みpreview操作、実ネットワーク遅延での同時DnDは未実施。正式検証と統合/配備は専任ownerへ引継ぎ。
- 実行中の自分のCargo/Chromium/fixtureサーバーなし。通常targetと今回の専用中間出力4.8GiBは、共有build-dir不整合時の追加局所再検証用として保持。一時入口/ログは48KiB、製品Git対象外。共有cache・sccacheサーバー・他Workspaceを変更/削除/停止しない。
- preview2は両port待受なしの状態を維持し、起動/停止なし。launcher inode31273405/size178/mtime1792943153、独立DB inode31033808/size2002944/mtime1792972348は開始時と一致。秘密・タグ・session・DB内容は未操作。
- 新規/変更22パスは上記一覧。ソースcommit後のexact SHA・各source hash・base/取り込みSHA・時刻はpersistent evidence `revision.json`/`source-hashes.json`に記録。スクリーンショットはcomponent fixtureであり実previewの証拠へ読み替えない。
