# 設定UIの整理試作（LOCAL UI Workspace）

基準HEAD: `89c1a09517f293f9409d8ba6f0d4279868cd36e9`。
Workspace: `fc2436ab-1227-4bd6-9ad9-061e5ed48638`。
通常CODEの実装試作であり、main採用・公開・配備は未承認。

## 設計と保存の契約

設定の入口は共通SettingsDialogとし、使い心地／AIと作業の既定値／プロジェクトとコード／接続とアクセス／運用に分類した。
一般に混在していた作業既定値と通知は独立した画面に分けた。旧section IDは改名せず、追加IDはexecution/notifications/access。
隠れていたrepos/mcpも一覧から到達できる。Project内のRepository入口は関連作業の文脈として残す。

認証方式、メンバー、ChatGPT資格、監査は既存ServerAccessGateの管理UIをそのまま共通shellに埋め込む。
localアプリがstatus.user.adminを確認してContextで供給するため、一般メンバーの設定ナビには追加しない。
管理APIのstatus.management・再認証・確認・secret reveal guardは変更しない。
ログイン、初期設定、/?access=settings、/?access=confirm、/?access=recoveryは独立した従来のライフサイクルを維持する。
埋込画面からの復旧は通常のページ遷移であり、未保存時はブラウザの離脱保護が働く。

保存先はカテゴリ上部と一般設定の各カードへ表示する。config.jsonを「個人設定」と呼ばない。
一括保存、端末即時反映、個別DB操作、状態確認・復旧を同じ保存契約に変更しない。
LVK ConfigurationとAgent native User/Project、Agent Profile、Files、Effective Configの関係を上部で説明する。
外部AI→LVKの実行権限／接続認証／Agent→外部MCPの向きは別の入口にした。

カテゴリ変更と閉じる操作に未保存確認を入れ、キャンセルは元のフォームを保持する。
スマホの「一覧へ戻る」はフォームをmountしたまま隠すため下書きを保持する。別項目選択時に初めて破棄を確認する。
ページ離脱保護、focusの復帰・Tab循環、背景のスクロール停止、可変高のPCダイアログ、スマホの保存バーを用意した。
認証画面の下書きは元の値との差分・子フォームの未保存状態で検出し、保存済みの個別操作を一律dirtyとする方式は採らない。

## 旧→新対応（機能・項目は維持）

| 旧入口・項目 | 新しい場所 | 保存・反映 |
| --- | --- | --- |
| 一般：外観、言語 | 使い心地 → 表示・操作とプライバシー | インスタンスconfig.json、一括保存 |
| 一般：モバイル文字サイズ | 同上、モバイル時の外観欄 | 端末localStorage、即時 |
| 一般：外部エディタ一覧・既定・接続設定 | 同上 → 外部エディタ | config.json、一括保存 |
| 一般：メッセージ入力ショートカット案内 | 同上 → メッセージ入力 | 読取専用案内 |
| 一般：テレメトリ | 同上 → プライバシー | config.json、一括保存 |
| 一般：オンボーディング再表示 | 同上 → 安全性 | config.json、個別操作で即時 |
| 一般：既定AgentとConfiguration | AIと作業の既定値 → 作業の既定値 | config.json、一括保存 |
| 一般：Gitブランチ接頭辞・Workspaceディレクトリ | 同上 → Git | config.json、一括保存 |
| 一般：PR自動生成・カスタムプロンプト | 同上 → Pull Request | config.json、一括保存 |
| 一般：コミットリマインダー・カスタムプロンプト | 同上 → コミット | config.json、一括保存 |
| 一般：タスクテンプレート一覧・追加・編集・削除 | 同上 → タスクテンプレート | インスタンスDB、個別操作 |
| 一般：Web Push購読・解除・Project選択 | 使い心地 → 通知 → Web Push | Project×ユーザー×端末購読、個別操作 |
| 一般：通知音on/off・音源・試聴・OS通知on/off | 同上 → 通知 | config.json、一括保存。試聴は実行操作 |
| エージェント：LVK実行Configuration一覧・作成・複製・削除・既定・可視性・編集 | AIと作業の既定値 → AIエージェント | 既存profiles/configのguardを維持 |
| エージェント：native User/Project、Profile、Files、Effective Config、差分確認・適用 | 同上、native設定欄 | AgentネイティブファイルとProfileの既存対象を維持 |
| エージェントツール：MCP/Skills一覧・定義・有効化・追加・編集・削除・コピー | AIと作業の既定値 → MCP・Skillsの項目管理 | Agent native User/Project。native設定内の同機能入口も維持 |
| MCP連携：外部AIからのファイル権限・実行権限・許可root | 接続とアクセス → 外部AI → LVK | インスタンスruntime-access、既存revision/save/guard |
| 内部mcp：Agentの外部MCP接続先・一覧編集・JSON・scope・適用確認 | 接続とアクセス → エージェント → MCP全体JSON | 既存nativeファイルと確認guard |
| プロジェクト：名前・色、既定Workspace、Kanban列（追加・名前・色・順序・表示・削除） | プロジェクトとコード → プロジェクト | 選択Projectと既定Workspace、既存保存契約 |
| 内部repos／Project内Repo：名前・path・既定branch/cwd | プロジェクトとコード → リポジトリ | 選択Repository |
| Repo：GitHub PATの状態・保存・接続確認・削除 | 同上 → GitHub資格情報 | 既存暗号化・秘匿guard |
| Repo：OpenWiki有効化・branch・言語・状態・保守入口 | 同上 → OpenWiki | 既存管理契約。試作開発で保守は実行しない |
| Repo：自動マージ検証、setup/cleanup/archive/copy-files/dev-server scripts・並列setup | 同上 → 検証・scripts | RepositoryとGit管理ファイルの既存契約を維持 |
| 右下：Local/Basic/Auth0・手動／自動Auth0セットアップ・ログイン試験・有効化 | 接続とアクセス → 認証・メンバー | 管理者用。独立した確認と有効化手順 |
| 右下：ChatGPT OAuth資格の保存・コピー・明示表示、MCP接続の説明 | 同上 → ChatGPTへのMCP接続 | 暗号化保存・明示revealを維持 |
| 右下：メンバー承認・有効/無効・admin/member、監査 | 同上 → メンバー／監査 | 既存権限・確認guardを維持 |
| 認証復旧 | 同上の復旧リンク → 独立復旧画面 | 既存operatorキーのライフサイクル |
| 共有リソース：登録・更新・状態確認・操作・調停・取消・復旧 | 運用 → 共有リソース・復旧 | 共有リソースの既存権限・確認・operation契約 |

Kanbanのfilter・並べ替え・ペイン、Run入力は移動していない。
分類見出し以外に既存フォームを再実装せず、値・既定値・API・スキーマ・秘密値表示を維持することで抜けを抑える。

## LOCAL-285との関係

`git show 6823bcc`で通知関連diffを参照した。cherry-pickしていない。
285のGeneralSettingsSection内OsNotificationEventsは新category=notificationsの通知カード内に挿入する。
WebPushSettingsのevent_maskと各購読のイベント編集は同カテゴリのWeb Pushカードにそのまま統合する。
OSイベントはインスタンスconfig、Pushイベントはユーザー・Project・購読ごとの即時保存であり、同じ保存ボタンに束ねない。
285のDB migration・型生成・配送backendは本試作に含めず、別途凍結候補で統合検証する。LOCAL-284/286は未操作。

## preview2引継ぎ（担当側起動）

今回の表示確認は実コンポーネントとbrowser内API fixture。サーバーDB、実Auth0、実Push、Agent nativeファイルは使用しない。
`tests/settings-redesign/fixture/api.ts`の未対応APIは503で拒否する。実認証変更・実行・復旧はfixtureでは無効。サンプル資格の保存応答とテスト用handoffだけを模擬する。
保存はページ内メモリであり再読込でリセット。空のProject/Repo/Agent inventoryでは実設定の全フォーム操作を証明しない。

確認時、PID1の既存LVK_PREVIEW_ROUTESはpreview2.luckyvibekanban.com:18444。
18444と旧backend用18446はlistenerなし、外部HTTPSのHEADは502だった。当Workspaceの正規DevServerはstoppedかつprocess_tracking_settled=true。
resource-coordination snapshotのresources/holders/queueは空で、slot登録・占有権の証拠がない。担当側で割当を確認するまで起動しない。
既存preview2プロセス、ネットワーク、Compose、nginx、既存authを変更していない。

局所準備済み：

```bash
node scripts/settings-preview.mjs --prepare
node scripts/settings-preview.mjs --check
# 担当側がslot割当を確認し、正規DevServerのforeground scriptとして起動する：
node scripts/settings-preview.mjs --run
```

作業ディレクトリはこのWorkspaceのlucky-vibe-kanban。要求portは127.0.0.1:18444のみ。backend portは使用しない。
起動時にportが占有されていれば失敗し、既存listenerを止めない。SIGTERM/SIGINTで自身のViteをcloseし、nohup/detachはしない。
局所資格は `.build/settings-preview/credentials.json`（0600）へ生成済み。担当者はファイルを安全に参照して利用者へ渡す。実資格のコピーではない。
fixture用Basicが全HTTPを保護し、実環境のVITE環境値を公開せず、HTTP /apiを拒否、Vite fsはsource/dependenciesだけを許可し.build/.env/.git/DB/token等を拒否する。
公開URL候補は `https://preview2.luckyvibekanban.com/`。まだ起動しておらず、利用可能URLとしては未検証。
比較用query：`?section=notifications`、`?section=access&auth=auth0`、`?section=access&auth=local`、`?role=member`、`?lang=en&theme=dark`。
サンプルrole/authはfixture内の表示条件を切替えるだけで、preview用Basicの認証を解除しない。

## 検証記録と限界

実際のAuth0/ChatGPT接続、OS通知/Push到達、実管理者の復旧、実backendのデータ保存、公開preview接続は未実行。
本試作はUIの情報設計比較が目的であり、これらの成功をfixtureから推定しない。


初回実装Run（`2de9fd42-64ab-4a6e-becd-3a226ca6e0a6`）の最終ソース検証結果：

| 検査 | 結果・範囲 |
| --- | --- |
| `pnpm run web-core:check` / `pnpm run local-web:check` | 成功 |
| `pnpm run local-web:lint` | 成功 |
| `pnpm --dir packages/local-web run build` | 最終ソースで成功（tsc + Vite）。Browserslist / Tailwind content / chunk sizeのwarningは残るがbuild errorなし |
| `git diff --check` | 成功 |
| local-webのESLint設定を明示して変更したshared settings TSXを検査 | 成功。web-core単独のESLint設定は存在しないため、最初の直接実行は設定未検出で失敗し、この既存設定を指定して検証した |
| `LVK_E2E_CHROMIUM=/opt/lvk-browsers/chromium-1217/chrome-linux64/chrome pnpm exec playwright test -c tests/settings-redesign/playwright.config.ts` | 19件成功。360/390px、PC、日英、dark、保存・破棄・保存失敗・カテゴリ変更・Escape、旧認証link、各認証mode、member非露出、既存ChatGPT資格の更新と個別保存、認証成功handoff、監査focusを含む |
| web-core Vitest: RuntimeAccessSettingsSection / settingsHostPolicy / agentSettingsModel | 3ファイル7件成功 |
| `node --test tests/settings-redesign/inventory.test.mjs` | 1件成功。基準commitの一般設定12カードと比較し、scopeの案内と整形以外のコントロール・既定値・handlerが保持されていることを検査 |
| `node scripts/settings-preview.mjs --check` | 成功。config load・無認証/誤認証拒否・API拒否・認証済みsource入口・fs制限を検査。listenerは開始しない |

19件の最終browser試験は、独立レビュー後に修正したコードで実行した。開発途中のfixture CSS/config・テストselector・サンプルenvironment不足による失敗を合格扱いしていない。
レビューでは、復旧入口の欠落、保存済み入力とdirtyの混同、summaryのfocus対象漏れ、既存ChatGPT資格更新時のdirty漏れを検出・修正した。
成功した認証handoffは、その画面のleave guardを同期的に解除してから既存のページ遷移を行う。未保存の通常移動と混同しない。

実表示の証拠（Git管理外、Workspaceに保持）：

- `test-results/settings-redesign/desktop-notifications.png`
- `test-results/settings-redesign/mobile-360.png` / `mobile-390.png`
- `test-results/settings-redesign/mobile-access-basic.png` / `mobile-access-auth0.png` / `mobile-access-local.png`
- `test-results/settings-redesign/english-dark.png`

`packages/local-web/dist`は通常のfrontend build成果物として保持する。`.build/settings-preview`は局所資格・build logを保持するため残す。
fixture以外の実backend検証とslot割当は引継ぎ事項。本採用の分類・説明の妥当性はpreviewでの利用者評価を待つ。


## 完了回復と正規DevServerの局所接続（2026-10-31）

回復Run: `9caa1737-da59-46cd-a5f6-532d5cf8d71d`。
前Runの親終了時に未確定だった `/root/settings_review` へ終了確認だけを依頼し、
「レビュー作業は完了済み。追加作業なし」の最終回答と `completed` 状態を確認した。
レビューの成果と既存の試作diff・画像・build成果物はすべて保持した。他processの停止は行っていない。

このRunではアプリ実装を変更せず、既存buildのsource mapと現在の変更TSX全10ファイルが一致することを照合した。
保存済みbrowser reportの `.last-run.json` はpassed、build logは成功終端である。
上表の19件・7件を今回の再実行とは扱わず、UI suite・型/lint/buildの不要な再実行は省略した。
新たに実行したのは局所launcherの構文・dispatch検査、previewのlistenerなしcheck、source map一致照合、差分空白検査。

### 実際の管理経路

`GET /api/workspaces/fc2436ab-1227-4bd6-9ad9-061e5ed48638/repos` の構成だけを読取確認した。
共有Repositoryの既存 `dev_server_script` は `exec bash .lvk-preview/start.sh`。
managerはWorkspaceに関連するRepoのこのscriptを読み、当該Repo名をworking_dirとして起動する。
MCPのstart引数にshell commandやscript overrideは存在しない。

このWorkspaceでは `.lvk-preview/start.sh` が欠けていたため、新規に局所launcherを用意した。
既存Git exclude対象であり、共有Repo設定・本番・他Workspaceのlauncherには変更を加えていない。
局所launcherは自身の所在からRepo rootへcdし、次の一つのforeground processにexecする：

```bash
exec node scripts/settings-preview.mjs --run
```

保管ファイル：

- `.lvk-preview/start.sh`：0700、Git管理外。既存managed DevServer入口が読む局所設定。
- `.build/settings-preview/managed-start.json`：0600、秘密値なし。対象ID・cwd・command・port・資格保管先の引継ぎmanifest。
- `.build/settings-preview/credentials.json`：0600、局所サンプル資格。本文・ログ・公開artifactへ転記しない。

担当側は既存preview2 slotの割当を確認した後、正規MCP `start_dev_server` を以下の対象で呼ぶ：

```json
{
  "project_id": "2e066b19-db27-4b8a-943e-ea097aa601c3",
  "workspace_id": "fc2436ab-1227-4bd6-9ad9-061e5ed48638"
}
```

stdioの引数はこの2件。外部HTTP MCPではさらに担当側が新規UUIDの `request_id` を付け、retry時は同じIDと入力を使う。
必要scopeは `lvk:read` / `lvk:runtime:read` / `lvk:runtime:execute`。
外部MCPの受付receiptは `get_operation` で完了を確認し、`get_dev_server_status` とHTTP readinessを別途確認する。
共有scriptを直接 `node ...` に書き換える必要はない。node単体のrun_process/nohup起動も不要。

現時点の当Workspace managed statusはstopped、process_tracking_settled=true、processes空。
資源台帳もresources/holders/queue空であり、公開slotの所有者確認を代替しない。今回start/stopは呼んでいない。

### 既存preview2認証との関係と受渡し

既存のnginx生成設定 `/tmp/lvk-server/nginx.conf` のpreview2 server blockを構成として確認した。
現在の設定は `auth_basic off`、Authorization headerを上流に渡し、127.0.0.1:18444へproxyする。
これは既存の独立アプリ経路の仕様であり、このRunで認証を外したものではない。
proxyがLVK login/sessionを検証して守る仕組みはこの経路にない。

以前のpreviewアプリ側の認証を、このbrowser-onlyサンプルfixtureが検証する機能はない。
同じhostnameであることやAuthorizationが転送されることだけでは認証済みと判断できない。
既存backendの秘密値・cookie・資格DBは読み出さず、コピー・再利用・権限変更も行っていない。
既存credentialやsessionを安全に流用できることは確認できないため、サンプル用Basicを維持する。追加ログインが必要。

資格の受渡しは、担当者が本人認証済みのSSH/管理端末で当Workspaceの0600ファイルを直接参照し、
組織で承認済みのパスワードマネージャーの限定共有等で対象ユーザーへ渡す。
このチャット本文・DevServerログ・公開preview・Git・一般公開artifactには値を載せない。
担当者が利用できる承認済み共有経路がなければ、受渡しは未完了として扱い、認証を弱めて回避しない。

この局所launcherとサンプル資格はWorkspace固有のため、main統合や他Workspaceへの複製対象ではない。


## 視覚品質の再確認（Run 8430c518、2026-10-31）

旧画像の灰色被りは、撮影タイミングと本実装の面色・透過の複合だった。
同じ初期表示手順を隔離fixtureで再現すると、フォームがvisibleになった直後もentry animationがrunningで、opacityは約0.41。
アニメーション終了後はopacity=1になるが、面はなおrgba(227,227,227,0.95)、backdrop-filter=blur(4px)だった。
fixtureだけのCSS不具合として隠さず、設定shell・header・save barを不透明なbg-primaryに変更した。
sidebarと保存scope説明は不透明なbg-secondary。暗色テーマも既存トークンに追従する。
埋込認証では冗長な灰色カードだけを除き、独立login/setup画面のカードは維持する。
PCナビを少し詰め、全カテゴリが一覧で見えるようにした。mobileの行高は維持する。

保存成功後の再編集・破棄でsuccess表示が残る処理も修正した。dirty時は成功表示を出さない。
項目・既定値・保存先・認証guardは変更していない。

撮影はdocument.fonts.ready、有限アニメーションのfinished、2回のrequestAnimationFrameを待ち、
初期値、opacity=1、ぼかしなし、成功・未保存表示なしを確認して行う。
PCは外側のfixture説明を除いた実dialog要素そのものを撮影し、画像加工や撮影用の色上書きはしていない。
最終PNG4枚は実際に開いて目視確認した。文字の透け、フォームの灰色被り、横はみ出し、操作部品の重なりは見られない。

| キャプチャ（Git管理外） | PNG寸法 | 確認した内容 |
| --- | --- | --- |
| `.build/settings-visual-review/pc-categories-notifications.png` | 1120×820 | PCの全カテゴリ、通知の初期値、保存範囲 |
| `.build/settings-visual-review/pc-authentication.png` | 1120×820 | 認証フォームと空password欄、背景文字の透けなし |
| `.build/settings-visual-review/mobile-categories.png` | 390×844 | 全カテゴリ、最後の運用項目への到達 |
| `.build/settings-visual-review/mobile-permissions.png` | 390×844 | 権限の初期値、説明文、未保存barなし |

E2Eは今回の最終sourceで25件成功（従来19件＋視覚/状態回帰6件）。型検査、local-webと変更shared設定のlint、12カード保持検査、diff checkも成功。
追加視覚testの最初の失敗は、成功文言selectorから既存の✓が抜けていたためで、selector修正後の全suite成功と区別する。
Playwright passを目視合格の代わりにせず、computed style・撮影状態・PNGの目視を併せて確認した。
再現のbefore/after数値と最終確認は `.build/settings-visual-review/review.json`、旧比較画像は同directoryのbefore/に保持する。

認証再利用の調査・sample資格・共有設定は触っていない。局所launcherのexit78を維持し、外部previewも起動していない。
このRunは隔離loopback E2Eのみを使用し、終了後にそのlistenerが残っていないことを確認した。

## アカウント導線とp2 HMR準備（2026-11-01）

画面下の独立した設定・ログアウトを撤去し、既存の歯車から開く設定の左メニューへ「アカウント」を追加した。
ログイン状態・認証方式と既存logout APIを配置し、Basic/Auth0はログアウト可能、Localは接続状態の表示だけとする。
アカウント画面は全利用者へ供給し、認証・メンバー画面は従前の権限条件を保持する。新しい役割概念・アイコン・保存先は追加しない。
旧access/settings・初期設定・復旧の入口、設定の未保存保護も維持する。

隔離ブラウザ34件、型・lint成功。既存歯車コンポーネントをfixtureに接続後、アカウント9件を再確認した。
PC/360px、Local/Basic/Auth0、権限別表示、logout成功/失敗、日本語/英語、旧入口を検証。logoutはfixture内のみで実ユーザーに対して実行していない。
画像は`.build/settings-account-review/account-basic-{1440,360}.png`等。PC/スマホの配置・折返しを目視確認済み。

稼働p2は18444のbackendから静的frontendを配信している（rootに/@vite/clientなし）。このRunでは既存processを停止・再起動していない。
局所`.lvk-preview/start.sh`を`.build/settings-preview/hmr-run.py`へ接続し、親による次回正規startでVite18444/backend18446をforeground親子として管理する準備をした。
backend実行物・専用instance・public/basic・公開URLは同じ。DB/key/credentialの変更・新たな秘密の読取/コピー、共有Repo script・nginx・network・Auth0の変更はない。
片側終了または停止signal時は両子をterminate/waitし、8秒で終了しない子だけkill/waitする。
静的配信への局所rollback入口は`.build/settings-preview/start-static.sh`。操作は正規管理のstop/startに限る。

隔離loopback4197では、本番用React/Vite plugins・実設定モジュールでHMR update frame、ページtimeOrigin保持、開いたアカウント画面のDOM更新を確認した。
canary私有pathと不許可Hostは403、fixture APIは401、browser pageerrorなし。
初回harnessのcwdによるBabel解決、次のharnessのHTML preamble不足はharnessを修正して解消。fixtureのbuild差替えをHMR成功とは扱わない。
証拠は`.build/settings-account-review/hmr-proof.json`と`hmr-account-pc.png`。
**公開p2へのHMR反映はまだ未完了**。親が当Workspaceの正規DevServerをstopしsettlement/port解放を確認後startし、外部WSSと実画面を確認する必要がある。
詳細引継ぎは`.build/settings-account-review/handoff.json`、最新局所構成は`.build/settings-preview/managed-start.json`。
