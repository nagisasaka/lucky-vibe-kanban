---
type: guide
title: 単一サーバーコンテナーの運用境界
description: local LVK をサーバー上で動かす配布構成。HTTPS、認証、preview origin、永続化、停止と回復の契約。
tags: [server, docker, deployment, preview, persistence]
sources:
  - id: openwiki-source-c16d91d3eae6ee7bafc2593e
    resource: repo://crates/server-access/src/preview.rs
  - id: openwiki-source-f771a8041ae618a5a49437b9
    resource: repo://deploy/developer/STAGING.md
  - id: openwiki-source-353b77936aed6c3d486f5cb9
    resource: repo://deploy/developer/updater.py
  - id: openwiki-source-eaf96aacdc40d6db8dae4f8e
    resource: repo://deploy/server/acme.py
  - id: openwiki-source-394060232279d0ae0be019c2
    resource: repo://deploy/server/compose.acme.yaml
  - id: openwiki-source-ffb1a00d9048c7ef4369674b
    resource: repo://deploy/server/compose.local.yaml
  - id: openwiki-source-df6b7819295f53fc2381f23d
    resource: repo://deploy/server/compose.yaml
  - id: openwiki-source-eb00c8fb4075578ba5a7c691
    resource: repo://deploy/server/healthcheck.mjs
  - id: openwiki-source-078e9842bd26cfd05bb35acc
    resource: repo://deploy/server/nginx.conf.template
  - id: openwiki-source-89d0fb22262f0a0ac1df7d1e
    resource: repo://deploy/server/server.mjs
  - id: openwiki-source-229e22cb29e176ae1fe594e1
    resource: repo://deploy/server/server.test.mjs
  - id: openwiki-source-bb1ebe868e35e9e500714501
    resource: repo://Dockerfile
  - id: openwiki-source-4815b99ed5b9f7632484ef04
    resource: repo://docs/integrations/github-integration.mdx
  - id: openwiki-source-2e1d91f21691cd271afffb6f
    resource: repo://docs/self-hosting/server-container.mdx
  - id: openwiki-source-e4b6c8db09a6d8887192b13e
    resource: repo://packages/web-core/src/shared/lib/previewProxyUrl.ts
generated: { by: "codex", at: "2026-10-17T03:50:48.298Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-17T03:50:48.298Z
---

# 単一サーバーコンテナーの運用境界

server 配布は local LVK と coding agent を常時稼働サーバーへ置き、利用者のブラウザーから操作するための構成である。SQLite の local DB・Workspace・AgentRun を使う。[外部 MCP](../integrations/remote-access.md) は、この同じ local アプリに対する opt-in の入口である。ブラウザーを閉じてもサーバーの実行は続くが、コンテナー停止やホスト再起動はプロセスを止める。[製品の用途](../../docs/self-hosting/server-container.mdx)

信頼された利用者が共有する開発環境であり、Basic/Auth0認証がtenantごとのOS隔離を作るわけではない。LVK・nginx・agent は同じ非 root user `appuser`（UID/GID 10001）で動く。read-only secret mount は上書きを防ぐだけで、同じユーザーの agent からの読取を防がない。[信頼境界](../../docs/self-hosting/server-container.mdx)、[runtime user](../../Dockerfile)

## 配布 binary と開発 checkout

root Dockerfile の最終 `server` target は prebuilt server・隣接 `agent-process-host`・MCP CLI と開発 toolchain を含む。稼働 binary は `/usr/local/bin` に置かれ、`/repos` の LVK checkout を編集・build しても、それを管理しているアプリを置換しない。小さい `runtime` target は別の起動入口を持ち、server target の認証付き ingress と同じではない。[target と起動入口](../../Dockerfile)

Node 22、固定版 Codex/pnpm、repository の Rust toolchain などを image に置き、user の認証・cache は永続領域へ分ける。追加 SDK は派生 image の `/opt` や `/usr/local` に置くと、既存 home volume に隠されにくい。公開版・digest と対応する配布 kit を使い、文書の例の tag を公開済み成果物と推定しない。[image の構成](../../Dockerfile)、[派生 image の契約](../../docs/self-hosting/server-container.mdx)

LVK 自身の開発向けに、`server-tools` は sccache 0.18.0 を locked dependencies・`--no-default-features` で導入し、`/usr/local/bin/sccache` に置く。local disk cache 用の executable 同梱であり、global Cargo wrapper・cache directory・incremental 設定を自動変更しない。home volume に隠されない配置と、各 repository が利用を設定する境界を維持する。[同梱と意図](../../Dockerfile#L173-L180)。実 image での利用可否と cache hit は別途の検証対象である。

## HTTPS と起動前検証

Compose は host の `443` だけを nginx の `8443` へ公開する。Rust backend は `127.0.0.1:3000`、preview proxy は `3001` を使い、supervisor は継承された HOST/PORT を上書きする。許可 origin は設定したアプリの HTTPS origin である。[port と mount](../../deploy/server/compose.yaml)、[backend 環境](../../deploy/server/server.mjs)

`LVK_HOST` は IP アドレスまたは scheme/port/path を含まない小文字 DNS 名を受け付ける。任意の `LVK_PREVIEW_DOMAIN` を使う場合、アプリ名を preview wildcard と同じかその配下に置けない。起動前に TLS 証明書の期間、アプリ名と preview の代表 host 名への対応、秘密鍵との一致を検証する。既存のnginx Basicを維持する配置ではbcrypt htpasswdも検証する。新規公開配置はhtpasswdなしで起動し、LVKの初期設定を完了するまで業務APIをロックする。volume 書込権と `nginx -t` も通ってからサービスを開始する。[設定](../../deploy/server/server.mjs)、[起動順](../../deploy/server/server.mjs)

基本 Compose は既存証明書を使う。任意の compose.acme.yaml は専用 Certbot companion と acme volume を追加し、TCP 80 を証明書検証専用に公開する。公開 IP または DNS 名のアプリ証明書を扱い、wildcard preview は別管理となる。[ACME 構成](../../deploy/server/compose.acme.yaml)・[発行と更新](../../deploy/server/acme.py)。起動検査はブラウザーによる trust chain 検証の代わりではない。証明書・鍵・htpasswd は `/run/lvk-secrets` へ read-only mount し、欠けた host path を Compose が自動作成しない。更新・認証情報変更の具体手順は [運用文書](../../docs/self-hosting/server-container.mdx#renew-certificates-and-rotate-credentials)に置く。[mount](../../deploy/server/compose.yaml)、[証明書の責任](../../docs/self-hosting/server-container.mdx)

公開配置の認証はLVKのserver-accessが担う。管理者が `docker compose exec lvk server setup-token` で15分間有効な一度限りのキーを発行し、HTTPS画面でBasic/Auth0を設定する。Auth0は実ログイン試験と本人確認後に有効化する。既存htpasswdがある場合はnginx Basicも維持し、LVK側の設定後に運用者が `LVK_PROXY_AUTH=backend` へ切り替える。新規配置では事前のhtpasswd作成は不要。旧 `LVK_MCP_CONFIG` は拒否される。[認証手順](../../docs/self-hosting/authentication.mdx)、[MCPのscope](../integrations/remote-access.md)、[生成されるingress](../../deploy/server/server.mjs)。管理認証のingressはWeb資格情報をbackendへ転送する一方、内部agent用のヘッダーを外部から受け渡さない。WebSocket upgrade を通し、proxy buffering/cache は無効にする。access log は query string・認証情報・agent 本文を含めず method/URI/status を記録する。[ingress 契約](../../deploy/server/nginx.conf.template)

## Preview は別 origin

開発サーバー `5173` の公開 URL は `https://5173.<preview-domain>` になる。公開認証層が許すportは1024–65535から3000・3001・4020・4021・8443・8444を除いたもの。UIやnginxの古い除外集合だけで許可を判断しない。main origin の `/api/preview` と `/api/host/<id>/preview` は 403 にし、project-controlled HTML を LVK の origin へ持ち込まない。[nginx の分離](../../deploy/server/nginx.conf.template)、[preview host の生成](../../deploy/server/server.mjs)

UI も公開 preview domain がある場合は HTTPS wildcard URL を使い、relay host を組み合わせない。preview 無効なら wildcard server は作らない。backend認証ではログイン済みLVKからoriginごとに認可を委譲する。第三者cookieを拒否するブラウザーでは独立タブで認可・表示する。親sessionの失効、cookieの分離、資格情報除去は [preview handoff](workspace-inspection.md#公開-preview-への認可委譲)に集約する。HTML/JavaScript に埋め込まれた絶対 localhost URL を全て書き換える保証はない。[URL 構築](../../packages/web-core/src/shared/lib/previewProxyUrl.ts)、[preview の運用制限](../../docs/self-hosting/server-container.mdx)。ローカル表示との関係は [Workspace preview](workspace-inspection.md#preview-と開発サーバー)を参照する。

`compose.local.yaml`は公開presetとは別で、nginxを経ずserverを直接起動しhost loopbackに3000を公開する。コンテナー内部のbindは0.0.0.0でも、配置を明示LocalにしてHostのloopback検査を残す。[Local preset](../../deploy/server/compose.local.yaml)、[配置の制約](../concepts/server-access.md#配置と認証方式は別の設定)

運用文書の初期段落に残る「Auth0ではpublic previewを拒否」という記述は、現在のhandoff実装および同文書のProject previews節と一致しない。実装は任意のpreview domainに対する限定認可を提供する。[handoff](../../crates/server-access/src/preview.rs)、[現在の運用節](../../docs/self-hosting/server-container.mdx#project-previews)

## 永続化と実行の寿命

| named volume | mount | 維持するもの |
| --- | --- | --- |
| home | `/home/appuser` | SQLite・設定・認証・agent Session・user cache |
| repos | `/repos` | 元の Git repository と未追跡ファイル |
| work | `/var/tmp` | worktree・共有資源・run 関連ファイル |

Compose project 名の変更は別 volume の選択になる。同じ named volume の再利用はデータを維持するが、実行中プロセスは復元しない。custom worktree root を mount 外へ置けばこの永続化の対象外になる。[保存先](../../deploy/server/compose.yaml)、[永続データの契約](../../docs/self-hosting/server-container.mdx)

`.lvk-shared` の意味は [Workspace の共有領域](../concepts/workspace.md#共有ディレクトリの意味)と同じで、Cargo target 等を自動設定するものではない。停止した Goal や保守 run は保存状態と所有者の規則に従って確認する。コンテナー再起動だけで任意の処理を自動継続する保証はない。[再起動後の確認](../../docs/self-hosting/server-container.mdx)

supervisor は server と nginx を process group として起動する。一方の終了・spawn 失敗ではもう一方へ SIGTERM を送り、既定 120 秒後に残存 group へ SIGKILL を送る。外部 SIGTERM/SIGINT も同じ停止手順を使い、Compose の停止猶予は150秒である。これは個々の AgentRun の [監査付き取消](../architecture/agent-runtime.md#取消と実プロセスの終了)とは別のコンテナー停止境界である。[supervision](../../deploy/server/server.mjs)、[猶予](../../deploy/server/compose.yaml#L21)

healthcheckは内部`/api/health`の200と、公開identity・CA/chain・残存6時間超を含むTLS検査を行う。公開rootの期待値はmaintenance時503、既存proxy Basicありなら401、backend認証なら200である。新規公開配置のロック状態も正常な初期状態であり、healthyだけでは初回認証やagent認証の完了を示さない。運用文書は unhealthy のみでは再起動しないことを明示する。サービス終了時の restart policy と health status を分けて診断する。[probe](../../deploy/server/healthcheck.mjs)、[再起動契約](../../docs/self-hosting/server-container.mdx)

## 更新・回復と検証限界

### サーバー内ブラウザー E2E と開発者向け更新サービス

標準 server image は `/opt/lvk-browsers` に固定版 Playwright の Chromium と
OS 依存ライブラリーを含む。home volume に隠されない場所に配置し、Compose の
共有メモリーは256MBとする。`pnpm run server:e2e` は指定した実アプリのURLへ接続して
画面描画・reload・実API・匿名HTTPS拒否を確認する。接続先は `LVK_E2E_BASE_URL`、
Basic認証は `LVK_E2E_CREDENTIALS_FILE` のJSONから読み、TLS検証は無効化しない。
既存 `workflow:e2e` は4175のUI fixtureであり、8443の実アプリE2Eとは区別する。
[ブラウザー構成](../../deploy/browser/package.json)、[実アプリE2E](../../tests/server/playwright.config.ts)

CIとホスト更新時の内部readiness検査は、loopback限定の内部capabilityでAPIへ接続する。
Chromiumはmodule/CORSリクエストで`Origin`を再付与するため、検査用通信は
Playwrightの`route.fetch`で取得し、ブラウザーへ返す。リダイレクト追跡を無効にし、
capabilityを別originへ転送しない。公開側のHost/Origin検証は変更しない。
これは実利用者のBasic/Auth0ログインの証明ではなく、実ブラウザーの起動・画面描画と
内部APIのreadiness検査である。[検査用通信](../../deploy/browser/smoke.mjs)

任意導入の `deploy/developer` は Linux/systemd ホスト向けで、標準imageには含めない。
ホストへ設置したroot所有のコードがDockerを操作する。agentには要求ディレクトリーへの
書込みと状態の読取りだけを提供し、Docker socket・rootコード・ホスト用GitHub tokenを
渡さない。公開forkは `client.py release --repository OWNER/REPO --tag VERSION` で
CI結果とdigestを取得し更新を要求できる。要求元agentはそのrunを終了する必要がある。
[導入と運用手順](../../deploy/developer/README.md)、[非特権client](../../deploy/developer/client.py)

公開CI結果の読取りとGit push認証は別である。サーバーからcommit/tagをpushするには、
forkだけに制限した書込み認証を別途設定する。repository専用Deploy keyを使う場合、
秘密鍵は安定版の永続homeに保存し、Gitや配布imageには含めない。公開releaseの解決・
更新要求だけならGitHub API tokenは不要である。[認証と開発手順](../../deploy/developer/README.md)

LVK の画面から行う Git/PR には [リポジトリ別 PAT](../integrations/github-review.md#リポジトリ別-pat-の所有と管理)も利用できる。agent terminal の push やホスト updater の認証とは適用範囲が異なるため、画面で token を保存したことを任意 shell の認証済み状態とみなさない。[適用範囲](../../docs/integrations/github-integration.mdx#repository-scoped-fine-grained-tokens)

ホストは許可 repository・source SHA・digest・公開 workflow 成功を照合して候補を検査する。現在の自己開発手順は card ごとの独立 instance → main の固定 staging 候補 → release → stable 更新である。staging は built frontend と debug binary を使い、source watcher を走らせない。標準 updater の対象は stable の lvk のみで、旧 development サービスは互換オプションとなる。詳しい操作と一度限りのホスト移行は [STAGING.md](../../deploy/developer/STAGING.md)、独立状態の契約は [開発と検証](development.md) を参照する。

更新は管理対象の DB と実プロセスが idle になるまで待つ。旧構成で development も管理する場合は両方が対象となる。Goal active、未完了AgentRun/
attempt、共有資源所有、未完了workflow、未知process、有効な定期実行は更新を妨げる。
HTTPのreadiness barrierで既存handlerの終了を待ち、新しい変更リクエスト・WebSocket
upgradeを503で拒否する。[HTTP barrier](../../crates/server/src/middleware/maintenance.rs)
その後、停止で失われるin-memory follow-upを共通admission mutex付きのqueue GETで
検査する。旧版にこの能力がなければ、まずSSHから対応版へbootstrapする。
既定 debug DB は checkout の dev_assets、release DB は永続 home にある。instance/build process も busy とみなすため staging を終了してから stable を更新する。
idle検査から停止までの競合を閉じるため、両コンテナーをfreezeして再検査し、仕事が
増えていれば再開して待つ。idleを確認できた場合だけfreezeしたまま終了する。
これはgraceful shutdownではなくSQLite WAL回復を使う停止であり、active agentを
強制終了する手段にはしない。停止後、全永続volumeと設定を保存し、再生成可能な
development Cargo targetだけを除外する。[状態機械と検査](../../deploy/developer/updater.py)

新imageの起動・ブラウザー検査中は任意のmaintenance mountによってHTTPSを503にする。
検査失敗時はchecksum検証済みのimage/データの組へ戻す。ホスト処理自体が中断された場合は
`recovery_required` を永続記録して後続更新を止め、SSHから確認・回復する。再起動は
実行中processの自動復元を意味しない。更新サービス自身のコード変更は別途ホストで反映する。
[競合・WAL・失敗時のテスト](../../deploy/developer/test_updater.py)

Codex のログイン済み表示と container の healthy は、カードからの実行成功を保証しない。
配置後は使い捨てのカードから通常の Code 実行を開始し、応答まで確認する。
特に `invalid transport in mcp_servers.openwiki` は認証エラーではなく設定エラーである。
Codex は無効な MCP サーバーにも transport を要求するため、通常 agent が OpenWiki を
無効化するときは effective config に存在する場合だけ override する。未登録のサーバーへ
`enabled = false` だけを追加してはならない。既存の stdio / HTTP transport は保持し、
保守 agent の明示的な MCP 起動設定とは区別する。
[設定の生成と回帰テスト](../../crates/executors/src/executors/codex.rs)がこの境界を扱う。

canonical 運用手順は agent を停止・完了させ、LVK を止めて三つの volume と設定・secrets を整合した組としてバックアップする。rollback は古い image だけを新 DB に向けず、対応する全 volume のバックアップを同じ内部パスの別 recovery deployment へ戻して検証する。schema migration と Git worktree metadata を分離して復旧しない。[バックアップと rollback](../../docs/self-hosting/server-container.mdx)

回帰検証は設定・証明書・停止を Node tests、TLS/認証/preview 分離を native nginx fixture、公開 URL を Rust/TypeScript tests で扱う。これらは実サーバー配置成功や公開 registry の存在を証明しない。native nginx が無い場合の skip と必須化は [開発時の検証](development.md#サーバー配布-kit-の検証)に置く。[認証・分離の test](../../deploy/server/nginx.test.mjs)、[停止の test](../../deploy/server/server.test.mjs)、[記録された未検証範囲](../../docs/self-hosting/server-container.mdx)
