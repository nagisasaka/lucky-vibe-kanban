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
  - id: openwiki-source-a537edea423e85a3d3d1dd7e
    resource: repo://crates/server-access/src/tests/chatgpt.rs
  - id: openwiki-source-24816571d030e7526cab74f1
    resource: repo://crates/server-access/src/tests/provisioning.rs
generated: { by: "codex", at: "2026-10-18T14:00:51.649Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-18T14:00:51.649Z
---

# サーバー認証・設定・セッション

`server-access` は一つの信頼されたチームのLVKに入るための共通境界である。Auth0がOAuth認可を担当し、LVKは設定、本人とメンバー、Webセッション、要求の受付を管理する。coding providerへのログイン、Workspaceの実行予約、OS隔離とは別である。承認済みメンバーは同じ実行環境を共有し、Projectごとのtenant隔離はない。[責任](../../crates/server-access/src/lib.rs#L1-L6)、[運用の信頼境界](../../docs/self-hosting/server-container.mdx#L6-L16)。本人と歴史的ownerの関係は [Organization・Member](organization-and-membership.md)に置く。

## 配置と認証方式は別の設定

| 配置 | 認証と起動条件 |
| --- | --- |
| `local` | 未設定・未構成のローカル環境ではLocalモード。HTTP Hostはloopbackに限定し、Originがあれば一致を要求 |
| `public` | canonicalなHTTPS originの`LVK_PUBLIC_URL`が必要。初期状態はlockedで、Basic/Auth0を明示設定する。Localへは切り替えられない |

`LVK_ACCESS_DEPLOYMENT`未指定で非loopbackへbindする設定は拒否する。`LVK_AUTH_MODE`は許す方式を制限するが資格情報を作らない。公開URLは末尾slash・path・query・userinfoを許さず、Host/Originを設定済み値と照合する。旧`LVK_MCP_CONFIG`は存在すれば起動拒否となる。[環境と検査](../../crates/server-access/src/lib.rs#L240-L405)

共通層はmain routerを包み、独立preview listenerには専用層を適用する。静的画面、health、access APIの入口は業務APIの認証前にも必要だが、access handler自身がsetup/管理セッションやCSRFを検査する。ログイン画面を取得できることは業務データの利用許可ではない。[要求分類](../../crates/server-access/src/lib.rs#L520-L630)、[router](../../crates/server/src/routes/mod.rs#L83-L95)

## 初回設定と方式変更

operator CLIの`server setup-token`は通常のlistenerやagentを開始せず、15分間有効な一度限りのキーを発行する。キー交換は限定されたsetup sessionを作る。既に設定済みならsetupキーでは変更できず、管理Web sessionまたは明示的な`recovery-token`を使う。[CLI分岐](../../crates/server/src/main.rs#L53-L66)、[キー発行](../../crates/server-access/src/store.rs#L409-L449)、[交換](../../crates/server-access/src/handlers.rs#L71-L121)

BasicはArgon2でpasswordを保存し、対話ログインでWeb sessionへ交換する。再認証では古いcookieだけで延長せずpasswordを再検証する。通常APIのBasic header認証は存在するが、認証設定の管理はsessionを要求する。[Basic認証](../../crates/server-access/src/store.rs#L221-L333)、[対話ログイン](../../crates/server-access/src/handlers.rs#L122-L157)

Auth0設定はdiscovery成功だけで有効化しない。PKCE S256とclient_secret_basicの広告を確認し、state・nonce・PKCE verifierを持つ5分のbrowser-bound transactionで実ログインする。callbackは一度だけ消費され、setup/管理sessionと同じブラウザーを照合する。確認された本人を画面で確かめactivateすると、その本人をadminにして設定を切り替える。[試験・callback・有効化](../../crates/server-access/src/handlers.rs#L267-L529)

設定切替はSQLiteの`BEGIN IMMEDIATE`で期待revisionと管理権限を再検証し、revision更新と旧session・setup key・transactionの削除を同時に行う。並行設定を上書きせず、古い確認結果ならやり直す。Basicへの変更後は再ログイン、Auth0のactivate後は新revisionのWeb sessionを発行する。[切替](../../crates/server-access/src/store.rs#L334-L395)、[activate](../../crates/server-access/src/handlers.rs#L506-L529)

## Auth0 の作成支援と有効化を分ける

設定画面の作成支援は setup / 管理 session と CSRF を要求し、Management API token をその要求内だけで使う。Inspect は既存 Regular Web Application と GitHub / Google connection を列挙し、Preview は実際の差分を示す。Apply は plan digest・session・5分の期限を暗号化した confirmation と再取得した現在の plan を照合する。変更や期限切れなら再 Preview が必要であり、この工程だけでは LVK の認証設定や admin を切り替えない。[支援 handler](../../crates/server-access/src/provisioning.rs#L586-L687)、[既存認証保持のテスト](../../crates/server-access/src/tests/provisioning.rs#L236-L307)

作成対象は Web 用と必要時の ChatGPT 用の別 application、MCP resource と scope、選択 connection の割当てである。既存 callback / metadata / scope 等を保持しつつ必要値を足すが、tenant の issuer 応答・resource parameter 設定も変更する。選択 application の connection 割当てだけを更新し、他 application の接続を一括置換しない。同じ URL / 役割の ownership metadata で作成済み application を再利用する。途中失敗は Auth0 側の作成を rollback せず、再 Preview 後の再適用で収束させる。LVK の実ログイン検証と activate は引き続き別段階である。[plan と apply](../../crates/server-access/src/provisioning.rs#L329-L584)、[部分失敗の再試行](../../crates/server-access/src/tests/provisioning.rs#L309-L332)

### ChatGPT の資格情報を再表示する

ChatGPT 専用 client ID / secret は `access_chatgpt_client` に既存の暗号化鍵で保存する。自動作成時だけでなく手動保存もでき、Web client と同じ ID を拒否する。status は secret を返さず、明示 reveal だけが返す。いずれも setup / 管理 session と CSRF を要し、内部 token や一般 member では取得できない。issuer・Web client ID・現在の MCP resource が一致する組だけを返し、保存と reveal の監査に secret を記録しない。[保存・照合・開示](../../crates/server-access/src/chatgpt.rs)、[有効化・再起動・権限テスト](../../crates/server-access/src/tests/chatgpt.rs)

保存済み secret は外部接続成功の証拠ではない。[外部 MCP の接続境界](../integrations/remote-access.md#認証付き外部-mcp)で scope と membership を別に確認する。Management token と ChatGPT secret、Web secret、LVK Web session を同一の資格情報として扱わない。

## Web・MCP・内部agentを同じ資格情報にしない

Web cookieはHttpOnly / SameSite=Lax、公開配置ではSecureで、Domainを付けない。Web sessionは絶対12時間・idle30分で、DBにはtokenのhashを保存する。cookie重複は曖昧として拒否する。変更要求にはCSRFが必要で、Webの値はsessionに、直接Basic/Localはchannelに対応する。[cookie](../../crates/server-access/src/lib.rs#L488-L518)、[sessionとCSRF](../../crates/server-access/src/store.rs#L171-L333)。UIのgateとfetch補助は [Web初期化](../architecture/web-and-sync.md#認証が製品画面より先に確定する)を参照する。

外部MCPはAuth0のBearer JWTとscopeを使い、Web cookieやBasicで代用できない。Auth0のissuerは末尾slash付きの正規化された`*.auth0.com`に限定される。ネットワーク取得先は設定issuerと同じoriginで、redirectを追わない。discovery/JWKSは有界cacheで、未知kidによる再取得を制限する。JWTはRS256、署名、issuer、`<public-origin>/mcp` audience、exp/nbfとactive membershipを検証する。[provider](../../crates/server-access/src/provider.rs#L12-L194)、[JWT](../../crates/server-access/src/provider.rs#L328-L374)。tool scopeとreceiptは [外部MCP](../integrations/remote-access.md#認証付き外部-mcp)を一次説明先とする。

内部stdio MCPは同じdata directoryのowner-only `access-internal.token`を読み、loopback HTTPにだけ送る。サーバーもloopback Host・Originなし・一致するtokenを要求する。internal principalはadminではなく、設定済みの業務APIに使う能力であり、setup/recoveryや認証管理を開放しない。Runner callbackはこれとも別で、既存installation/token検証を維持する。[内部判定](../../crates/server-access/src/lib.rs#L319-L336)、[受付](../../crates/server-access/src/lib.rs#L553-L607)、[stdio client](../../crates/mcp/src/task_server/mod.rs#L247-L274)。資源の認証が通ることと[共有資源所有](shared-resource-coordination.md)は別条件である。

## 失効は接続と受理済み作業で異なる

新規要求は設定revision・期限・active membershipを照合する。継続WebSocket/SSEはleaseで2秒ごとに再検証し、logout、member無効化、期限切れ、設定変更で終了する。socket taskには資格情報を保持しない。接続終了は既に受理されたagent作業やMCP mutationの取消を意味しない。[lease](../../crates/server-access/src/lib.rs#L161-L186)、[再検証](../../crates/server-access/src/lib.rs#L406-L435)、[SSE](../../crates/server-access/src/lib.rs#L665-L680)、[実WebSocket](../../crates/server/src/middleware/websocket.rs#L43-L61)。previewの委譲も親sessionへ依存するが、別cookieを使う。[preview handoff](../operations/workspace-inspection.md#公開-preview-への認可委譲)

Auth0側のrefresh取消とLVKのmember無効化は異なる。署名済みJWTは期限まで有効な場合があり、LVKは接続単位の即時JWT失効を提供しない。一般的なprovider停止時もBasic/Localへ自動fallbackしない。[運用上の失効区分](../../docs/self-hosting/authentication.mdx#L165-L188)

## 永続化・閉鎖・回復

設定とOIDC中間情報はAES-256-GCMで暗号化し、32byteの`access.key`をDBと分離して保存する。Unixでは鍵がowner-onlyで、symlinkではないことを検査する。`access.configured`は設定履歴のmarkerで、設定rowの消失を未設定Localへ戻す理由にしない。破損設定・鍵不在は閉鎖状態となる。[暗号化と読取](../../crates/server-access/src/store.rs#L28-L139)、[markerと切替](../../crates/server-access/src/store.rs#L362-L395)

DBと鍵・markerを整合した組で保存する。元の鍵がなければ旧暗号化設定は復元できない。operator recoveryは設定の修正用で、業務APIやメンバー一覧を無条件に開かない。具体的な退避・復旧手順は [認証運用](../../docs/self-hosting/authentication.mdx#L176-L199)、volume全体の整合性は [コンテナー回復](../operations/server-container.md)を参照する。

監査はactor/channel/action/target/outcomeを保存し、通常要求のquery・本文・資格情報を記録しない。これを全業務操作のtransaction監査と同一視しない。要求終了時の監査書込には失敗を返さない経路もある。[監査の生成](../../crates/server-access/src/lib.rs#L643-L664)

OIDC callback の失敗診断は固定 reason code と許可した field / category だけを記録し、token・nonce・identity claim・provider の生エラーを出さない。診断用 JWT decode は認証を通す処理ではなく、要求は引き続き401で拒否する。Provisioning も endpoint template・HTTP status・許可した検証ヒントだけを返し、部分失敗を既存認証の有効化成功とは扱わない。[OIDC 診断](../../crates/server-access/src/provider.rs#L422-L637)、[Management API エラー](../../crates/server-access/src/provisioning.rs#L13-L89)

## 検証と残る確認

sourceには設定消失時の閉鎖、SSE失効、並行admin無効化、実socket停止、previewのbrowser/origin束縛・再利用拒否のテストがある。[失効と並行操作](../../crates/server-access/src/tests.rs#L1179-L1320)、[preview](../../crates/server-access/src/tests.rs#L1554-L1737)。これらのfixtureは実Auth0 tenant・Google/GitHub本人確認・ChatGPT OAuth/MCP接続の証明ではない。実サービスの確認項目は [受入範囲](../../docs/self-hosting/authentication.mdx#L201-L224)に残っている。
