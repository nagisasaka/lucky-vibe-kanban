---
type: concept
title: サーバー認証・設定・セッション
description: Local/Public配置、Basic/Auth0の有効化、Webセッションと内部capability、設定破損時の閉鎖と復旧の契約。
tags: [authentication, authorization, sessions, security, recovery]
sources:
  - id: openwiki-source-93e2f3655b94fb7077d67788
    resource: repo://crates/server-access/src/chatgpt.rs
  - id: openwiki-source-20b72970921fabf5b536668c
    resource: repo://crates/server-access/src/handlers.rs
  - id: openwiki-source-7d94e7454f63f16057833046
    resource: repo://crates/server-access/src/lib.rs
  - id: openwiki-source-d9fe6f50a365b8b34a14b1e4
    resource: repo://crates/server-access/src/provider.rs
  - id: openwiki-source-04f2df3990d4aad9d1cac9a2
    resource: repo://crates/server-access/src/provisioning.rs
  - id: openwiki-source-9d15c737f5e087237387893e
    resource: repo://crates/server-access/src/store.rs
  - id: openwiki-source-a6cad3d930b299ebbbd793a3
    resource: repo://crates/server-access/src/tests.rs
  - id: openwiki-source-a537edea423e85a3d3d1dd7e
    resource: repo://crates/server-access/src/tests/chatgpt.rs
  - id: openwiki-source-24816571d030e7526cab74f1
    resource: repo://crates/server-access/src/tests/provisioning.rs
  - id: openwiki-source-a13fe4db1eee073d0a7e2c4d
    resource: repo://crates/server/src/main.rs
  - id: openwiki-source-6cb471b32e19ce921b78d86e
    resource: repo://crates/server/src/remote_mcp/environment_settings.rs
  - id: openwiki-source-7d788a7c8b1c5bf5122bab53
    resource: repo://crates/utils/src/assets.rs
  - id: openwiki-source-394060232279d0ae0be019c2
    resource: repo://deploy/server/compose.acme.yaml
  - id: openwiki-source-df6b7819295f53fc2381f23d
    resource: repo://deploy/server/compose.yaml
  - id: openwiki-source-eb00c8fb4075578ba5a7c691
    resource: repo://deploy/server/healthcheck.mjs
  - id: openwiki-source-89d0fb22262f0a0ac1df7d1e
    resource: repo://deploy/server/server.mjs
  - id: openwiki-source-bb1ebe868e35e9e500714501
    resource: repo://Dockerfile
  - id: openwiki-source-d236349bbead901a9455b33c
    resource: repo://packages/local-web/src/app/auth/Auth0Setup.tsx
  - id: openwiki-source-5d3aad6c180293a86853f6ec
    resource: repo://packages/local-web/src/app/auth/ServerAccessGate.tsx
  - id: openwiki-source-0d0dadc1962a96804bdd02da
    resource: repo://packages/web-core/src/shared/dialogs/settings/settings/SettingsAccessContext.tsx
  - id: openwiki-source-b42d4696e9f82a515195aa22
    resource: repo://tests/auth0-setup/setup.spec.ts
generated: { by: "codex", at: "2026-11-04T20:43:36.614Z" }
verified:
  - by: openwiki/0.7.2
    at: 2026-11-04T20:43:36.614Z
---

# サーバー認証・設定・セッション

`server-access` は一つの信頼されたチームのLVKに入るための共通境界である。Auth0がOAuth認可を担当し、LVKは設定、本人とメンバー、Webセッション、要求の受付を管理する。coding providerへのログイン、Workspaceの実行予約、OS隔離とは別である。承認済みメンバーは同じ実行環境を共有し、Projectごとのtenant隔離はない。[責任](../../crates/server-access/src/lib.rs#L1-L6)、[同じ OS user の配布構成](../operations/server-container.md)。本人と歴史的ownerの関係は [Organization・Member](organization-and-membership.md)に置く。

## 配置と認証方式は別の設定

| 配置 | 認証と起動条件 |
| --- | --- |
| `local` | 未設定・未構成のローカル環境ではLocalモード。HTTP Hostはloopbackに限定し、Originがあれば一致を要求 |
| `public` | canonicalなHTTPS originの`LVK_PUBLIC_URL`が必要。初期状態はlockedで、Basic/Auth0を明示設定する。Localへは切り替えられない |

`LVK_ACCESS_DEPLOYMENT`未指定で非loopbackへbindする設定は拒否する。`LVK_AUTH_MODE`は許す方式を制限するが資格情報を作らない。公開URLは末尾slash・path・query・userinfoを許さず、Host/Originを設定済み値と照合する。旧`LVK_MCP_CONFIG`は存在すれば起動拒否となる。[環境と検査](../../crates/server-access/src/lib.rs#L240-L405)

共通層はmain routerを包む。旧preview専用listener・handoffは廃止済みで、[独立CSV route](../operations/server-container.md#独立アプリの-csv-ルート)にはLVKログインを委譲しない。静的画面、health、access APIの入口は業務APIの認証前にも必要だが、access handler自身がsetup/管理セッションやCSRFを検査する。ログイン画面を取得できることは業務データの利用許可ではない。[要求分類](../../crates/server-access/src/lib.rs#L520-L630)、[router](../../crates/server/src/routes/mod.rs#L83-L95)

## 初回設定と方式変更

operator CLIの`server setup-token`は通常のlistenerやagentを開始せず、15分間有効な一度限りのキーを発行する。キー交換は限定されたsetup sessionを作る。既に設定済みならsetupキーでは変更できず、管理Web sessionまたは明示的な`recovery-token`を使う。[CLI分岐](../../crates/server/src/main.rs#L53-L66)、[キー発行](../../crates/server-access/src/store.rs#L409-L449)、[交換](../../crates/server-access/src/handlers.rs#L71-L121)

BasicはArgon2でpasswordを保存し、対話ログインでWeb sessionへ交換する。再認証では古いcookieだけで延長せずpasswordを再検証する。通常APIのBasic header認証は存在するが、認証設定の管理はsessionを要求する。[Basic認証](../../crates/server-access/src/store.rs#L221-L333)、[対話ログイン](../../crates/server-access/src/handlers.rs#L122-L157)

Auth0設定はdiscovery成功だけで有効化しない。PKCE S256とclient_secret_basicの広告を確認し、state・nonce・PKCE verifierを持つ5分のbrowser-bound transactionで実ログインする。callbackは一度だけ消費され、setup/管理sessionと同じブラウザーを照合する。確認された本人を画面で確かめactivateすると、その本人をadminにして設定を切り替える。[試験・callback・有効化](../../crates/server-access/src/handlers.rs#L267-L529)

設定切替はSQLiteの`BEGIN IMMEDIATE`で期待revisionと管理権限を再検証し、revision更新と旧session・setup key・transactionの削除を同時に行う。並行設定を上書きせず、古い確認結果ならやり直す。Basicへの変更後は再ログイン、Auth0のactivate後は新revisionのWeb sessionを発行する。[切替](../../crates/server-access/src/store.rs#L334-L395)、[activate](../../crates/server-access/src/handlers.rs#L506-L529)

## Auth0 の作成支援と有効化を分ける

設定画面の作成支援は setup / 管理 session と CSRF を要求し、Management API token をその要求内だけで使う。Inspect は既存 Regular Web Application と GitHub / Google connection を列挙し、Preview は実際の差分を示す。Apply は plan digest・session・5分の期限を暗号化した confirmation と再取得した現在の plan を照合する。変更や期限切れなら再 Preview が必要であり、この工程だけでは LVK の認証設定や admin を切り替えない。[支援 handler](../../crates/server-access/src/provisioning.rs#L586-L687)、[既存認証保持のテスト](../../crates/server-access/src/tests/provisioning.rs#L236-L307)

作成対象は Web 用と必要時の ChatGPT 用の別 application、MCP resource と scope、選択 connection の割当てである。既存 callback / metadata / scope 等を保持しつつ必要値を足すが、tenant の issuer 応答・resource parameter 設定も変更する。選択 application の connection 割当てだけを更新し、他 application の接続を一括置換しない。同じ URL / 役割の ownership metadata で作成済み application を再利用する。途中失敗は Auth0 側の作成を rollback せず、再 Preview 後の再適用で収束させる。LVK の実ログイン検証と activate は引き続き別段階である。[plan と apply](../../crates/server-access/src/provisioning.rs#L329-L584)、[部分失敗の再試行](../../crates/server-access/src/tests/provisioning.rs#L323-L347)

MCP resource の必要 scope は `lvk:read` / `lvk:write` / `lvk:merge` に `lvk:runtime:read` / `lvk:runtime:execute` を加えた5種類である。作成支援が scope を定義することは runtime grant の有効化ではない。管理者専用の設定API、DB優先の保存と取消後の寿命は [実行環境へのアクセス](runtime-access.md#管理設定の保存適用取消)を正本とする。MCP bearer や内部agentはこの設定を拡張できない。[scope](../../crates/server-access/src/lib.rs#L33-L39)・[追加と保持](../../crates/server-access/src/provisioning.rs#L466-L472)・[管理者境界](../../crates/server/src/remote_mcp/environment_settings.rs#L62-L81)

### 更新トークンの選択と適用済み案内

作成支援の「更新トークンを設定する」は初期 ON で、ChatGPT 向け MCP 設定自体は初期 OFF である。同じフォーム内で明示 OFF にすると、接続・アプリの再取得や MCP 欄の非表示／再表示でも選択を保持する。これは React フォームの初期値であり、API の `refresh_tokens` 省略時は引き続き false となる。フォームを開くだけで既存 tenant / client を更新する仕組みではなく、上記の Preview / Apply を経て変更する。[UI の状態と送信](../../packages/local-web/src/app/auth/Auth0Setup.tsx#L63-L134)・[API の省略値](../../crates/server-access/src/provisioning.rs#L83-L103)・[適用の境界](../../crates/server-access/src/provisioning.rs#L605-L648)

MCP を設定する場合、ON は ChatGPT application に `refresh_token` grant と rotating / expiring の設定（最大30日・未使用7日）を付け、MCP API の `allow_offline_access` を true にする。OFF は ChatGPT の grant を `authorization_code` のみにし、rotation 設定を送らない。新規 API なら Offline Access を false にするが、既存 API ではその field を変更対象から除く。他 client が利用する更新能力を無効化しないため、という理由がコードに記録されている。Web application は選択によらず `authorization_code` のみである。[client plan](../../crates/server-access/src/provisioning.rs#L386-L448)・[既存 API の保持](../../crates/server-access/src/provisioning.rs#L464-L500)

選択値は Preview の入力照合に含まれ、現在の入力が確認時と異なる間は Apply を表示しない。Apply 成功時にはその要求で使った選択を結果に保持し、ChatGPT の完了案内はこの値が ON の場合に `offline_access` の指定を案内する。後からフォームの選択を編集しても、表示中の適用済み案内は変わらない。[入力の照合と結果保持](../../packages/local-web/src/app/auth/Auth0Setup.tsx#L81-L133)・[完了案内](../../packages/local-web/src/app/auth/Auth0Setup.tsx#L353-L370)。この更新設定は [外部 MCP 接続](../integrations/remote-access.md#認証付き外部-mcp)に関係し、LVK の Web session（絶対12時間・idle30分）を延長する設定ではない。[独立した Web 期限](../../crates/server-access/src/lib.rs#L40-L41)

回帰確認の入口は、desktop light / mobile dark と ON / OFF の4通りで初期値・選択保持・Preview の入力不一致・送信値・適用後案内を検査する [browser spec](../../tests/auth0-setup/setup.spec.ts#L3-L105)と、ON / OFF の生成 plan、既存 API の保持、Preview 中の外部書込ゼロを検査する [server fixture](../../crates/server-access/src/tests/provisioning.rs#L693-L743)である。browser spec は API を fixture で置換しており、実 Auth0 / ChatGPT の接続成功を証明しない。今回の Sync では統合済み source とこれらのテスト定義を静的照合した。凍結入力にある開発テストの成功報告と、正式 Integration の検証省略は別の記録であり、今回のテスト再実行結果ではない。

### ChatGPT の資格情報を再表示する

ChatGPT 専用 client ID / secret は `access_chatgpt_client` に既存の暗号化鍵で保存する。自動作成時だけでなく手動保存もでき、Web client と同じ ID を拒否する。status は secret を返さず、明示 reveal だけが返す。いずれも setup / 管理 session と CSRF を要し、内部 token や一般 member では取得できない。issuer・Web client ID・現在の MCP resource が一致する組だけを返し、保存と reveal の監査に secret を記録しない。[保存・照合・開示](../../crates/server-access/src/chatgpt.rs)、[有効化・再起動・権限テスト](../../crates/server-access/src/tests/chatgpt.rs)

保存済み secret は外部接続成功の証拠ではない。[外部 MCP の接続境界](../integrations/remote-access.md#認証付き外部-mcp)で scope と membership を別に確認する。Management token と ChatGPT secret、Web secret、LVK Web session を同一の資格情報として扱わない。

## Web・MCP・内部agentを同じ資格情報にしない

Web cookieはHttpOnly / SameSite=Lax、公開配置ではSecureで、Domainを付けない。Web sessionは絶対12時間・idle30分で、DBにはtokenのhashを保存する。cookie重複は曖昧として拒否する。変更要求にはCSRFが必要で、Webの値はsessionに、直接Basic/Localはchannelに対応する。[cookie](../../crates/server-access/src/lib.rs#L488-L518)、[sessionとCSRF](../../crates/server-access/src/store.rs#L171-L333)。UIのgateとfetch補助は [Web初期化](../architecture/web-and-sync.md#認証が製品画面より先に確定する)を参照する。

外部MCPはAuth0のBearer JWTとscopeを使い、Web cookieやBasicで代用できない。Auth0のissuerは末尾slash付きの正規化された`*.auth0.com`に限定される。ネットワーク取得先は設定issuerと同じoriginで、redirectを追わない。discovery/JWKSは有界cacheで、未知kidによる再取得を制限する。JWTはRS256、署名、issuer、`<public-origin>/mcp` audience、exp/nbfとactive membershipを検証する。[provider](../../crates/server-access/src/provider.rs#L12-L194)、[JWT](../../crates/server-access/src/provider.rs#L328-L374)。tool scopeとreceiptは [外部MCP](../integrations/remote-access.md#認証付き外部-mcp)を一次説明先とする。

内部stdio MCPは同じdata directoryのowner-only `access-internal.token`を読み、loopback HTTPにだけ送る。サーバーもloopback Host・Originなし・一致するtokenを要求する。internal principalはadminではなく、設定済みの業務APIに使う能力であり、setup/recoveryや認証管理を開放しない。Runner callbackはこれとも別で、既存installation/token検証を維持する。[内部判定](../../crates/server-access/src/lib.rs#L319-L336)、[受付](../../crates/server-access/src/lib.rs#L553-L607)、[stdio client](../../crates/mcp/src/task_server/mod.rs#L247-L274)。資源の認証が通ることと[共有資源所有](shared-resource-coordination.md)は別条件である。

## 失効は接続と受理済み作業で異なる

新規要求は設定revision・期限・active membershipを照合する。継続WebSocket/SSEはleaseで2秒ごとに再検証し、logout、member無効化、期限切れ、設定変更で終了する。socket taskには資格情報を保持しない。接続終了は既に受理されたagent作業やMCP mutationの取消を意味しない。[lease](../../crates/server-access/src/lib.rs#L161-L186)、[再検証](../../crates/server-access/src/lib.rs#L406-L435)、[SSE](../../crates/server-access/src/lib.rs#L665-L680)、[実WebSocket](../../crates/server/src/middleware/websocket.rs#L43-L61)。旧preview handoffは廃止済みである。[外部アプリの認証境界](../operations/server-container.md#埋込みpreviewの廃止)を確認する。

Auth0側のrefresh取消とLVKのmember無効化は異なる。署名済みJWTは期限まで有効な場合があり、LVKは接続単位の即時JWT失効を提供しない。一般的なprovider停止時もBasic/Localへ自動fallbackしない。[新規要求の再認証](../../crates/server-access/src/lib.rs#L455-L484)

[Web Push の device consent](../integrations/web-push.md#通知同意と所有権)は Web session と別の寿命を持つ。`server-access` は local/basic/web actor の owner・認証 revision・用途を暗号化 envelope に固定し、配送時に membership / revision を再検証する。logout や idle expiry を越えて通知できるが、login token を保存せず API access や session 延長を認めない。内部 agent / MCP channel は登録できない。[seal/open](../../crates/server-access/src/lib.rs#L394-L436)。期限・OFF・再登録の契約は通知の説明先に置く。

## 永続化・閉鎖・回復

設定とOIDC中間情報はAES-256-GCMで暗号化し、32byteの`access.key`をDBと分離して保存する。Unixでは鍵がowner-onlyで、symlinkではないことを検査する。`access.configured`は設定履歴のmarkerで、設定rowの消失を未設定Localへ戻す理由にしない。破損設定・鍵不在は閉鎖状態となる。[暗号化と読取](../../crates/server-access/src/store.rs#L28-L139)、[markerと切替](../../crates/server-access/src/store.rs#L362-L395)

DBと鍵・markerを整合した組で保存する。元の鍵がなければ旧暗号化設定は復元できない。operator recoveryは設定の修正用で、業務APIやメンバー一覧を無条件に開かない。具体的な退避・復旧手順は下記に置く。volume全体の整合性は [コンテナー回復](../operations/server-container.md#更新前のバックアップと復元検証)を参照する。

監査はactor/channel/action/target/outcomeを保存し、通常要求のquery・本文・資格情報を記録しない。これを全業務操作のtransaction監査と同一視しない。要求終了時の監査書込には失敗を返さない経路もある。[監査の生成](../../crates/server-access/src/lib.rs#L643-L664)

OIDC callback の失敗診断は固定 reason code と許可した field / category だけを記録し、token・nonce・identity claim・provider の生エラーを出さない。診断用 JWT decode は認証を通す処理ではなく、要求は引き続き401で拒否する。Provisioning も endpoint template・HTTP status・許可した検証ヒントだけを返し、部分失敗を既存認証の有効化成功とは扱わない。[OIDC 診断](../../crates/server-access/src/provider.rs#L422-L637)、[Management API エラー](../../crates/server-access/src/provisioning.rs#L13-L89)

## 検証と残る確認

sourceには設定消失時の閉鎖、SSE失効、並行admin無効化と実socket停止のテストがある。[認証と閉鎖](../../crates/server-access/src/tests.rs#L176-L350)、[鍵喪失](../../crates/server-access/src/tests.rs#L464-L476)。これらのfixtureは実Auth0 tenant・Google/GitHub本人確認・ChatGPT OAuth/MCP接続の証明ではない。実サービスでは、公開TLS経由の本人確認、未承認memberの拒否、MCP audience一致、scope別操作、token更新と取消、member無効化後の既存接続停止を別々に確認する。fixtureの成功を実サービスの結果と扱わない。

## 設定内のアカウントと本人情報

設定のアカウント画面は接続情報・方式と既存logoutをまとめる。本人名はstatusの任意 `user.display_name` を使い、Basicは認証済みユーザー名、Auth0は既存memberの表示名となる。statusは現在のweb principalとid/issuer/subjectが一致するactive memberのみを参照し、管理用member一覧を使わない。Localは本人名を返さずLocal接続として表示する。欠落・空白・旧backendでは取得不可を示し、UUIDや仮名で代用しない。[status](../../crates/server-access/src/handlers.rs)・[表示とlogout](../../packages/local-web/src/app/auth/ServerAccessGate.tsx)・[認証回帰](../../crates/server-access/src/tests.rs)

認証設定は既存管理権限で供給する別画面であり、共有設定側が権限を推定しない。組込み認証・作成支援・ChatGPT資格情報のdirty状態を設定shellへ伝え、保存済み情報の存在だけで未保存扱いにしない。認証成功の遷移は明示的にleave guardを解除して行う。[context境界](../../packages/web-core/src/shared/dialogs/settings/settings/SettingsAccessContext.tsx)・[組込み画面](../../packages/local-web/src/app/auth/ServerAccessGate.tsx)・[作成支援](../../packages/local-web/src/app/auth/Auth0Setup.tsx)

## 運用者が初期設定と回復を行う

前提は対象サーバーの管理権限と、その稼働binaryに対応したDB/data directoryである。標準公開Composeでは `docker compose exec lvk server setup-token` を実行し、公開HTTPS画面へ一度限りのキーを入力する。独立instanceでは同じ `LVK_INSTANCE_DIR` をCLIにも指定する。debug/releaseの既定data directoryを混同しない。キーをURL・チャット・通常ログへ貼らず、15分以内に交換する。再発行は旧キーと未完了のsetup/recovery sessionを無効にする。[CLI入口](../../crates/server/src/main.rs#L54-L72)・[発行と交換](../../crates/server-access/src/store.rs#L408-L450)

Basicでは任意のユーザー名と12〜1024byteのpasswordを設定し、保存後に再ログインする。共通の初期passwordや読戻しAPIはない。Auth0では作成支援の適用後も、実ログイン試験でissuer/subjectを確認してからactivateする。MCP用clientはWeb用と分け、設定画面のresourceとJWT audienceを完全一致させる。[password検査](../../crates/server-access/src/store.rs#L390-L406)・[設定手順の状態遷移](../../crates/server-access/src/handlers.rs)

Auth0停止・誤設定でログインできない場合もBasic/Localへ自動fallbackしない。次は認証設定だけを回復する手順であり、image更新やDBのrollbackとは分ける。対象の配置directory、元のCompose project名・Composeファイル選択、稼働していたimage digest/binary、volume名・内部path、配置設定を記録し、この回復中は変更しない。新しいagent・Runner作業を開始せず、中断した作業もここでは再開しない。

1. agentを完了/停止させ、Goal・定期実行・共有resource operationと外部Runnerを確認する。接続断を停止完了と扱わず、実processの停止・containmentの状態を確認する。元の配置directory・projectで `docker compose stop` を行い、停止中にhome/repos/workの全volume、DBと存在するWAL/SHM、`access.key`、`access.configured`、設定・secret・証明書・ACME volumeを同じ時点の組として保全する。具体的なarchiveコマンドは [停止バックアップ手順](../operations/server-container.md#更新前のバックアップと復元検証)にある。各コマンドのexit status、`tar -tzf` によるarchive読取、SHA-256と設定・秘密の退避を確認する。一つでも失敗したら先へ進まない。この時点ではLVKは停止したままである。
2. 退避を検証できたら、**同じ既存deploymentを同じimage/binary・project・volume・設定で起動し直す**。停止に使った配置directoryと同じComposeファイル選択・project指定で次を実行する。

   ```bash
   docker compose start
   docker compose ps
   docker compose logs --tail=100 lvk
   ```

   `start` は停止した既存serviceを戻すために使う。全serviceを停止したACME配置では `certificates` も戻す。ここでimageのpull/build、`.env` の更新、serviceの再作成、別projectでの起動や空volumeの初期化を行わない。既存containerがない、image/volume/configの対応が不明、または起動に失敗した場合は、この手順を止めて配置の対応関係とログを確認する。source配置でも停止前と同じbinary・起動設定・`LVK_INSTANCE_DIR` で対象を戻す。[Composeのserviceとmount](../../deploy/server/compose.yaml#L1-L37)・[ACMEの依存](../../deploy/server/compose.acme.yaml#L1-L6)・[起動前検査](../../deploy/server/server.mjs#L434-L512)
3. `lvk` が稼働し、内部healthとTLSの検査を満たすことを確認する。標準server imageでは `docker compose exec lvk node /opt/lvk-server/healthcheck.mjs` のexit statusが0となることを確認できる（[同梱healthcheck](../../Dockerfile#L221-L233)）。既存の公開HTTPS URLで `/?access=recovery` を開き、証明書検証が成功して復旧キー入力画面が表示されることを確認してからキーを発行する。残存nginx Basicがある配置ではその既存のproxy認証も必要になる。health成功や画面表示は認証設定の修復成功ではなく、設定破損中のlocked表示や業務API拒否を解除する理由にならない。到達できなければログ・TLS・proxy・起動設定を直す段階で止め、token発行で迂回しない。[healthと期待status](../../deploy/server/healthcheck.mjs#L34-L74)・[復旧フォーム](../../packages/local-web/src/app/auth/ServerAccessGate.tsx#L371-L405)・[認証前の入口と業務APIの分離](../../crates/server-access/src/lib.rs#L570-L616)
4. 起動したその対象のdata directoryで `server recovery-token` を発行する。標準Composeでは同じ配置directory・project指定で `docker compose exec lvk server recovery-token` を使う。CLIはDBと鍵の保存先をlistenerと共有する必要があり、独立instanceでは同じ `LVK_INSTANCE_DIR` をCLIにも指定する。キーは既に確認したHTTPS復旧画面へ15分以内に入力し、URL・チャット・通常ログに残さない。CLI自体はlistenerやagentを起動しないため、停止したserviceの代わりにはならない。[CLIの早期分岐](../../crates/server/src/main.rs#L54-L68)・[同じdata保存先](../../crates/utils/src/assets.rs#L6-L38)・[一度限りのキー](../../crates/server-access/src/store.rs#L408-L450)
5. 限定復旧sessionで、配置の `LVK_AUTH_MODE` が許すBasicを設定し直す、またはAuth0の設定・実ログイン試験・本人確認・activateを行う。復旧sessionは認証設定用で、Kanban・ファイル・member一覧・MCPを開放しない。設定変更成功後は旧sessionが失効するため、新しい方式でログインし、権限と拒否も確認する。その後に既存Goal・定期実行・資源所有を照合し、必要な作業だけを意図的に再開する。再起動自体を作業の完了・再開成功と扱わない。[方式の制約](../../crates/server-access/src/lib.rs#L349-L359)・[再起動後の確認](../operations/server-container.md#更新前のバックアップと復元検証)

[限定sessionの試験](../../crates/server-access/src/tests.rs#L306-L350)・[設定切替のtransaction](../../crates/server-access/src/store.rs#L311-L375)

元の正しい `access.key` を保全してある場合は、まず現在の組を退避し、対象を停止した状態でそのDBに対応する元鍵の復元を検討する。これは元鍵を失って新しい認証設定を登録する回復とは異なる。復元後も上記の同一配置の再起動・到達確認から進み、DBと鍵・markerを異なる時点の組へ置換しない。

`access.key` の形式・permission不正でCLI自体が拒否する場合は、運用者がまず元ファイルとDBを保全し、破損した鍵を退避してから同じdata directoryで復旧キーを発行する。存在する不正鍵をCLIは上書きせず、鍵がない場合だけ新しい32byte鍵を作る。新しい鍵は旧暗号化設定を復号しないため、認証設定を入力し直す必要がある。元の鍵がなければ旧secretは復元できず、ChatGPT用など同じ鍵で暗号化された情報も再登録が必要になる。業務DBや `access.configured` を削除して未設定扱いへ戻さない。Unix鍵permissionは0600相当とし、backupも同等に保護する。[鍵生成と復号](../../crates/server-access/src/store.rs#L28-L105)・[再発行だけでは復号しない試験](../../crates/server-access/src/tests.rs#L464-L476)・[ChatGPT secretの暗号化](../../crates/server-access/src/chatgpt.rs)
