---
type: guide
title: 単一サーバーコンテナーの運用境界
description: local LVK をサーバー上で動かす配布構成。HTTPS、認証、preview origin、永続化、停止と回復の契約。
tags: [server, docker, deployment, preview, persistence]
sources:
  - id: openwiki-source-b50585f67dac89cd8eecc75d
    resource: repo://crates/executors/default_mcp.json
  - id: openwiki-source-0a300791bc056c6eee6a577a
    resource: repo://crates/mcp/src/bin/vibe_kanban_mcp.rs
  - id: openwiki-source-c16d91d3eae6ee7bafc2593e
    resource: repo://crates/server-access/src/preview.rs
  - id: openwiki-source-d4130322a85c68ce2ddc220a
    resource: repo://crates/server/build_metadata.rs
  - id: openwiki-source-f72d699e94c301e4e47d097c
    resource: repo://crates/server/build.rs
  - id: openwiki-source-a9b6d4be264787a78357c9d6
    resource: repo://crates/server/src/routes/config.rs
  - id: openwiki-source-f771a8041ae618a5a49437b9
    resource: repo://deploy/developer/STAGING.md
  - id: openwiki-source-45785e038b483538ab5c9894
    resource: repo://deploy/developer/test_updater.py
  - id: openwiki-source-353b77936aed6c3d486f5cb9
    resource: repo://deploy/developer/updater.py
  - id: openwiki-source-eaf96aacdc40d6db8dae4f8e
    resource: repo://deploy/server/acme.py
  - id: openwiki-source-394060232279d0ae0be019c2
    resource: repo://deploy/server/compose.acme.yaml
  - id: openwiki-source-fd822232d9aac9030c892c92
    resource: repo://deploy/server/compose.development.yaml
  - id: openwiki-source-ffb1a00d9048c7ef4369674b
    resource: repo://deploy/server/compose.local.yaml
  - id: openwiki-source-df6b7819295f53fc2381f23d
    resource: repo://deploy/server/compose.yaml
  - id: openwiki-source-eb00c8fb4075578ba5a7c691
    resource: repo://deploy/server/healthcheck.mjs
  - id: openwiki-source-078e9842bd26cfd05bb35acc
    resource: repo://deploy/server/nginx.conf.template
  - id: openwiki-source-8b0f6773cd6f7a2a989ad880
    resource: repo://deploy/server/nginx.test.mjs
  - id: openwiki-source-c5de8230bc8ac4c0c0000287
    resource: repo://deploy/server/release.test.mjs
  - id: openwiki-source-89d0fb22262f0a0ac1df7d1e
    resource: repo://deploy/server/server.mjs
  - id: openwiki-source-229e22cb29e176ae1fe594e1
    resource: repo://deploy/server/server.test.mjs
  - id: openwiki-source-8d57c3092adc3c7e7de33c02
    resource: repo://deploy/server/smoke-image.sh
  - id: openwiki-source-bb1ebe868e35e9e500714501
    resource: repo://Dockerfile
  - id: openwiki-source-b548a221f2376a10ae5d152c
    resource: repo://docs/development/preview-routes-validation.md
  - id: openwiki-source-4815b99ed5b9f7632484ef04
    resource: repo://docs/integrations/github-integration.mdx
  - id: openwiki-source-2e1d91f21691cd271afffb6f
    resource: repo://docs/self-hosting/server-container.mdx
  - id: openwiki-source-5b54a58d1b51cd490b0e7162
    resource: repo://package.json
  - id: openwiki-source-e4b6c8db09a6d8887192b13e
    resource: repo://packages/web-core/src/shared/lib/previewProxyUrl.ts
generated: { by: "codex", at: "2026-10-24T11:38:17.046Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-24T11:38:17.046Z
---

# 単一サーバーコンテナーの運用境界

server 配布は local LVK と coding agent を常時稼働サーバーへ置き、利用者のブラウザーから操作するための構成である。SQLite の local DB・Workspace・AgentRun を使う。[外部 MCP](../integrations/remote-access.md) は、この同じ local アプリに対する opt-in の入口である。ブラウザーを閉じてもサーバーの実行は続くが、コンテナー停止やホスト再起動はプロセスを止める。[製品の用途](../../docs/self-hosting/server-container.mdx)

信頼された利用者が共有する開発環境であり、Basic/Auth0認証がtenantごとのOS隔離を作るわけではない。LVK・nginx・agent は同じ非 root user `appuser`（UID/GID 10001）で動く。read-only secret mount は上書きを防ぐだけで、同じユーザーの agent からの読取を防がない。[信頼境界](../../docs/self-hosting/server-container.mdx)、[runtime user](../../Dockerfile)

## 配布 binary と開発 checkout

root Dockerfile の最終 `server` target は prebuilt server・隣接 `agent-process-host`・MCP CLI と開発 toolchain を含む。稼働 binary は `/usr/local/bin` に置かれ、`/repos` の LVK checkout を編集・build しても、それを管理しているアプリを置換しない。小さい `runtime` target は別の起動入口を持ち、server target の認証付き ingress と同じではない。[target と起動入口](../../Dockerfile)

Node 22、固定版 Codex/pnpm、repository の Rust toolchain などを image に置き、user の認証・cache は永続領域へ分ける。追加 SDK は派生 image の `/opt` や `/usr/local` に置くと、既存 home volume に隠されにくい。公開版・digest と対応する配布 kit を使い、文書の例の tag を公開済み成果物と推定しない。[image の構成](../../Dockerfile)、[派生 image の契約](../../docs/self-hosting/server-container.mdx)

LVK 自身の開発向けに、`server-tools` は sccache 0.18.0 を locked dependencies・`--no-default-features` で導入し、`/usr/local/bin/sccache` に置く。local disk cache 用の executable 同梱であり、global Cargo wrapper・cache directory・incremental 設定を自動変更しない。home volume に隠されない配置と、各 repository が利用を設定する境界を維持する。[同梱と意図](../../Dockerfile#L173-L180)。実 image での利用可否と cache hit は別途の検証対象である。

`less` は `server-tools` の apt package として導入され、`server-browser` を経て最終 `server` image に継承される。小型 `runtime` は `runtime-base` から分岐するため、この開発 toolchain の追加を継承しない。[stage の境界](../../Dockerfile#L136-L156)、[server への継承](../../Dockerfile#L182-L201)。任意の旧 development Compose も `LVK_IMAGE` を参照するため、利用できる toolchain は選択した image に依存する。checkout の変更だけで稼働中 image に追加されるわけではない。[開発 Compose の image 選択](../../deploy/server/compose.development.yaml#L1-L8)

image smoke は UID 10001 のコンテナー内で `less --version` と、非 TTY の pipe 入力が同じ文字列として出力されることを検査し、失敗すれば `sh -ec` により検査を止める。この検査は対話端末でのページ操作を扱わない。[実行条件と検査](../../deploy/server/smoke-image.sh#L24-L38)。[配布 kit の静的・native 検証](development.md#サーバー配布-kit-の検証)と image の実行確認は別である。この Sync では検査定義をソースで確認しており、Docker build/run や `less` の実行成功は確認していない。

### 旧 npx ランチャーの撤去と移行

旧 LVK npx ランチャーの npm packaging・download・展開・cache 管理は撤去済みで、互換ランチャーは提供しない。Docker のみを配布する方針に対応した変更であり、source build の入口は残る。[移行契約](../../docs/self-hosting/server-container.mdx#legacy-npx-cli-migration)・[現行 package](../../package.json)

| 以前の入口 | 現行の経路 |
| --- | --- |
| `npx lucky-vibe-kanban` | このページの Docker 配置、またはソース開発の `pnpm run dev` |
| `lucky-vibe-kanban mcp` / `--mcp` | 直接 build した `vibe-kanban-mcp --mode global`、または認証付き外部 HTTP MCP |
| `lucky-vibe-kanban --desktop` | Docker server または source 開発 server をブラウザーで開く。Tauri shell と build コマンドは廃止済み |

直接 build では frontend を用意し、`server`・`agent-process-host`・`vibe-kanban-mcp` を同じ checkout から作る。[具体的な build 手順](../../docs/self-hosting/server-container.mdx#legacy-npx-cli-migration)を使い、[開発時の検証範囲](development.md#検証の対象を取り違えない)と配布 image の検証を区別する。Docker もこの三つの binary を同梱する。[build と配置](../../Dockerfile#L98-L101)・[server image](../../Dockerfile#L199-L202)

stdio binary は既存 HTTP backend に接続するため、ランチャーの代わりに backend を自動起動するものではない。`--mode` 省略時は global、orchestrator は明示指定する。接続先の `VIBE_BACKEND_URL` / host・port 解決、内部資格情報と router の制約は [MCP client の接続境界](../integrations/agent-providers.md#外部-mcp-client--lvk)を参照する。[起動処理](../../crates/mcp/src/bin/vibe_kanban_mcp.rs#L23-L49)・[mode の検証](../../crates/mcp/src/bin/vibe_kanban_mcp.rs#L56-L97)。起動方法の移行は [HTTP MCP の認証・scope](../integrations/remote-access.md#認証付き外部-mcp)を免除しない。

撤去は既存アプリデータ・download 済み binary・`~/.vibe-kanban/bin` 等の cache の削除や、既存 launcher の遠隔無効化を行わない。agent CLI や第三者 MCP の npx 設定も別の責任範囲である。[保持する範囲](../../docs/self-hosting/server-container.mdx#legacy-npx-cli-migration)・[直接 LVK binary と第三者 preset](../../crates/executors/default_mcp.json)。古いローカル成果物が残ることと、現行 source がその launcher を提供することを混同しない。

Tauri 撤去も既存 desktop data や download 済み application を削除・移行しない。残る [browser の貼り付け・音・editor 接続](../architecture/web-and-sync.md#ブラウザー操作と廃止された-native-bridge)、[Web Push の同意と配送](../integrations/web-push.md)、[server OS notifier](../concepts/issue-collaboration.md#実行時の注意喚起)を個別に確認する。native shell の削除から共有 runtime の廃止を推定しない。[維持する範囲](../../docs/self-hosting/server-container.mdx#L330-L330)

### ビルド時ソース識別と API の契約

ビルド情報は「この server binary を作ったソース」を説明する。現在の Workspace / 起動先 checkout、package version、image digest、配布の成功状態とは別の情報である。`build.rs` が取得値を `cargo:rustc-env` へ固定し、`BuildInfo::current()` がコンパイル時の `env!` から読むため、起動時の `LVK_BUILD_*` で上書きされない。`/api/info` の `version` は従来の `CARGO_PKG_VERSION` を維持し、別の任意 `build_info` に revision と nullable な kind / release_tag / dirty を返す。revision がなければ `build_info` 自体を省略する。[固定の境界](../../crates/server/build.rs#L18-L36)・[API 型と応答](../../crates/server/src/routes/config.rs#L93-L156)

入力の優先順位は一つの契約として扱う。

- `LVK_BUILD_REVISION` の明示値は空文字でも authoritative であり、Git fallback を無効にする。この経路では `LVK_BUILD_KIND`、`LVK_BUILD_RELEASE_TAG`、`LVK_BUILD_DIRTY` の未指定項目を checkout から補わない。
- revision が未指定の直接 Cargo build だけは、source root と Git top-level の canonical path が一致することを確かめて HEAD と未追跡ファイルを含む dirty を取得し、kind を `development` にする。別 repository 内に展開した source archive を親 repository の版と扱わない。
- 有効な revision は40桁または64桁の16進値で、小文字化する。不正 revision は情報全体を不明にする。kind は `development`、または有効な tag を伴う `release` のみ。tag は `v` 始まりで英数字・`.`・`-` のみを許し、release 以外では除去する。dirty は文字列 `true` / `false` だけを既知とし、それ以外は不明にする。nearest tag や package version から補完しない。

[優先順位と Git root](../../crates/server/build_metadata.rs#L25-L55)・[正規化と未知値](../../crates/server/build_metadata.rs#L56-L86)。取得不能はビルド情報を失うが、resolver がそれ自体を build error にする経路ではない。

直接 Cargo build は、存在しない refresh path を `rerun-if-changed` に指定して毎回 Git 状態を再評価する。HEAD / index の監視だけでは未追跡変更と dirty → clean を見落とす、という理由がコードに記録されている。明示注入では環境変数の変更を監視する。metadata の解決は root `.env` の読込より前なので、この build script の dotenv 読込に `LVK_BUILD_*` の注入を期待しない。[再評価と読込順](../../crates/server/build.rs#L5-L36)

instance runner は `build.json` と同じ revision / dirty を注入する。既存の image 配布経路は source SHA と release tag を build 引数へ渡し、tag build を release、それ以外を development、dirty を false とする契約である。Docker builder の引数既定は空なので、注入しないローカル image に Git fallback や sidebar identity を期待しない。これらはビルド入力であり runtime 設定ではない。[instance の注入](../../deploy/developer/instance.mjs#L131-L195)・[Docker builder](../../Dockerfile#L80-L100)・[配布引数の静的回帰検査](../../deploy/server/release.test.mjs#L37-L58)・[運用契約](../../deploy/developer/STAGING.md#isolated-state)

UI はこの optional 情報を使い、短い SHA のボタンから詳細を開く。欠落時の非表示、Update との優先関係、mobile drawer の寿命は [共有 shell の表示契約](../architecture/web-and-sync.md#稼働バイナリのビルド情報を表示する)に置く。source identity の表示だけで公開済み image・実配布・validation 成功を保証しない。実 Git、API、SSR、keyboard / touch と release fixture の検証範囲および今回の静的調査の限界は [ビルド情報の検証境界](development.md#ビルド情報の検証境界)を参照する。

## HTTPS と起動前検証

Compose は host の `443` だけを nginx の `8443` へ公開する。Rust backend は `127.0.0.1:3000`、preview proxy は `3001` を使い、supervisor は継承された HOST/PORT を上書きする。許可 origin は設定したアプリの HTTPS origin である。[port と mount](../../deploy/server/compose.yaml)、[backend 環境](../../deploy/server/server.mjs)

`LVK_HOST` は IP アドレスまたは scheme/port/path を含まない小文字 DNS 名を受け付ける。任意の `LVK_PREVIEW_DOMAIN` を使う場合、アプリ名を preview wildcard と同じかその配下に置けない。起動前に TLS 証明書の期間、アプリ名・独立 CSV 経路の全 host 名・legacy preview の代表 host 名への対応、秘密鍵との一致を検証する。既存のnginx Basicを維持する配置ではbcrypt htpasswdも検証する。新規公開配置はhtpasswdなしで起動し、LVKの初期設定を完了するまで業務APIをロックする。volume 書込権と `nginx -t` も通ってからサービスを開始する。[設定](../../deploy/server/server.mjs)、[起動順](../../deploy/server/server.mjs)

基本 Compose は既存証明書を使う。任意の compose.acme.yaml は専用 Certbot companion と acme volume を追加し、TCP 80 を証明書検証専用に公開する。公開 IP または DNS 名のアプリと独立 CSV 経路の証明書を扱い、wildcard preview は別管理となる。[ACME 構成](../../deploy/server/compose.acme.yaml)・[発行と更新](../../deploy/server/acme.py)。起動検査はブラウザーによる trust chain 検証の代わりではない。証明書・鍵・htpasswd は `/run/lvk-secrets` へ read-only mount し、欠けた host path を Compose が自動作成しない。更新・認証情報変更の具体手順は [運用文書](../../docs/self-hosting/server-container.mdx#renew-certificates-and-rotate-credentials)に置く。[mount](../../deploy/server/compose.yaml)、[証明書の責任](../../docs/self-hosting/server-container.mdx)

公開配置の認証はLVKのserver-accessが担う。管理者が `docker compose exec lvk server setup-token` で15分間有効な一度限りのキーを発行し、HTTPS画面でBasic/Auth0を設定する。Auth0は実ログイン試験と本人確認後に有効化する。既存htpasswdがある場合はnginx Basicも維持し、LVK側の設定後に運用者が `LVK_PROXY_AUTH=backend` へ切り替える。新規配置では事前のhtpasswd作成は不要。旧 `LVK_MCP_CONFIG` は拒否される。[認証手順](../../docs/self-hosting/authentication.mdx)、[MCPのscope](../integrations/remote-access.md)、[生成されるingress](../../deploy/server/server.mjs)。管理認証のingressはWeb資格情報をbackendへ転送する一方、内部agent用のヘッダーを外部から受け渡さない。WebSocket upgrade を通し、proxy buffering/cache は無効にする。access log は query string・認証情報・agent 本文を含めず method/URI/status を記録する。[ingress 契約](../../deploy/server/nginx.conf.template)

## 独立アプリの CSV ルート

`LVK_PREVIEW_ROUTES=dev.example.com:4020,preview1.example.com:18443` は、名前ごとの外部 HTTPS を **LVK コンテナー内**の `127.0.0.1:<port>` の HTTP アプリへ転送する静的設定である。ホストや別コンテナーの loopback へは届かない。名前付き slot、preview UI、DB、動的ルート管理は作らず、アプリ起動・DNS・公開 URL/HMR・認証は各アプリの運用者が担う。[生成される経路](../../deploy/server/server.mjs#L248-L276)・[運用契約](../../docs/self-hosting/server-container.mdx#independent-application-routes)

### 入力と経路の境界

空または未設定なら無効。各項目は小文字の完全な DNS 名と10進 port で、項目の外側の ASCII space/tab だけを許す。末尾 dot・scheme・path・wildcard・IP literal・改行・空要素・重複 host・`LVK_HOST` との衝突は拒否する。DNS 名は最大253文字、port は1024–65535で先頭ゼロを許す。同じ port への別名は許可する。3000・3001・4021・4022・8443・8444は LVK/nginx 用に予約するが、4020は普通のアプリ port として使える。`dev` に特別な処理はない。[JS parser](../../deploy/server/server.mjs#L18-L68)・[Python parser](../../deploy/server/acme.py#L25-L55)

非空の `LVK_PREVIEW_DOMAIN` と CSV は同時に使えず、起動時に拒否する。legacy の wildcard preview と独立経路では port の除外集合も認証責任も違う。既存の UI preview は引き続き wildcard domain から URL を生成し、この CSV を UI の preview 一覧へ変換しない。[設定の排他](../../deploy/server/server.mjs#L34-L90)・[UI の URL](../../packages/web-core/src/shared/lib/previewProxyUrl.ts#L4-L35)・[legacy の port 検査](../../crates/server-access/src/preview.rs#L29-L48)

独立経路では nginx Basic を無効化し、LVK の server-access/handoff を経由せず upstream へ渡す。Host・Authorization・Cookie・Origin・WebSocket の情報はアプリへ保持し、HTTPS の forwarded 情報を設定する。`Proxy-Authorization` と `X-LVK-Internal-Token` は除去する。nginx の header 定義をこの server block で置き換えることが、親の LVK 用資格情報除去を継承しないための重要な境界である。LVK 側が認証済みでも、このアプリが認証を実装しなければ匿名で到達できる。cookie は sibling host 間へ資格情報を広げない host-only 運用を使う。[転送設定](../../deploy/server/server.mjs#L252-L274)・[認証境界の回帰](../../deploy/server/nginx.test.mjs#L332-L377)・[cookie の運用](../../docs/self-hosting/server-container.mdx#independent-application-routes)。LVK の認証委譲が必要な既存経路は、下記 [別 origin の preview](#preview-は別-origin)を参照する。

### 証明書と設定変更の寿命

同じ TLS 証明書が `LVK_HOST` と全 route 名を満たす必要があり、起動時と証明書 reload 前に全件を検査する。ACME companion は両 Compose から渡される同じ CSV を解析し、`LVK_HOST` が IP なら IP 識別子、各 route は DNS 識別子として、既存の `shortlived` profile と単一 `lvk` lineage へ注文する。HTTP-01 のため各名の DNS と port 80 到達性が必要で、wildcard preview は引き続き拒否する。[証明書検査](../../deploy/server/server.mjs#L296-L319)・[注文の生成](../../deploy/server/acme.py#L58-L96)・[companion 配線](../../deploy/server/compose.acme.yaml#L1-L42)

この実装は `LVK_HOST` を含む25識別子まで、つまり最大24 route を Certbot 呼出し前に許可する。これは既存 shortlived 運用を維持するための ACME 側の上限で、parser 自体に件数上限はない。超える場合は全名を満たす外部管理証明書と基本 Compose を使う。複数証明書への自動分割はない。[上限と単一注文](../../deploy/server/acme.py#L68-L96)・[101経路の parser 回帰](../../deploy/server/server.test.mjs#L483-L491)・[記録された判断](../../docs/development/preview-routes-validation.md#独立レビューと判断)

route と Certbot 引数は process 起動時に固定するため、`.env` 変更後は対象 Compose service の再作成が必要である。restart や nginx reload だけでは変更した Compose 環境を取り込まない。ACME では先に `certificates` を再作成し、新しい全名を満たす証明書を待ってから `lvk` を再作成する。外部管理なら証明書を先に用意する。この再作成では稼働アプリ・agent の停止を見込む。[運用順序](../../docs/self-hosting/server-container.mdx#independent-application-routes)・[起動時設定](../../deploy/server/server.mjs#L457-L508)・[Certbot 引数の固定](../../deploy/server/acme.py#L159-L176)

証明書だけの更新は既存 route 設定を保ち、30秒周期で検出して nginx を reload する。名前不足・key 不一致・nginx 検査失敗なら新 fingerprint を採用せず、失敗を記録して次周期に再試行する。backend/agent の再起動は行わない。[検査と採用順序](../../deploy/server/server.mjs#L352-L374)・[reload と再試行](../../deploy/server/server.mjs#L508-L523)

JS/Python に parser を置く構造は、新しい配布 runtime 依存や mount を増やさず既存入口を使うという凍結 Change Manifest の判断と一致する。両者の差異は [共有 fixture と native nginx の検証](development.md#独立アプリの-csv-ルートを変更するとき)で監視する。長い有効 DNS 名が nginx の既定 hash bucket では失敗した記録に対応し、template は512を指定する。[設定](../../deploy/server/nginx.conf.template#L6-L11)・[再現と修正の記録](../../docs/development/preview-routes-validation.md#独立レビューと判断)。実 ACME 発行・DNS・配布 image の成功はこの source 照合では確定しない。

## Preview は別 origin

legacy `LVK_PREVIEW_DOMAIN` を使う開発サーバー `5173` の公開 URL は `https://5173.<preview-domain>` になる。公開認証層が許すportは1024–65535から3000・3001・4020・4021・8443・8444を除いたもの。UIやnginxの古い除外集合だけで許可を判断しない。main origin の `/api/preview` と `/api/host/<id>/preview` は 403 にし、project-controlled HTML を LVK の origin へ持ち込まない。[nginx の分離](../../deploy/server/nginx.conf.template)、[preview host の生成](../../deploy/server/server.mjs)

UI も公開 preview domain がある場合は HTTPS wildcard URL を使い、relay host を組み合わせない。preview 無効なら wildcard server は作らない。backend認証ではログイン済みLVKからoriginごとに認可を委譲する。第三者cookieを拒否するブラウザーでは独立タブで認可・表示する。親sessionの失効、cookieの分離、資格情報除去は [preview handoff](workspace-inspection.md#公開-preview-への認可委譲)に集約する。HTML/JavaScript に埋め込まれた絶対 localhost URL を全て書き換える保証はない。[URL 構築](../../packages/web-core/src/shared/lib/previewProxyUrl.ts)、[preview の運用制限](../../docs/self-hosting/server-container.mdx)。ローカル表示との関係は [Workspace preview](workspace-inspection.md#preview-と開発サーバー)を参照する。

`compose.local.yaml`は公開presetとは別で、nginxを経ずserverを直接起動しhost loopbackに3000を公開する。コンテナー内部のbindは0.0.0.0でも、配置を明示LocalにしてHostのloopback検査を残す。[Local preset](../../deploy/server/compose.local.yaml)、[配置の制約](../concepts/server-access.md#配置と認証方式は別の設定)

運用文書の初期段落に残る「Auth0ではpublic previewを拒否」という記述は、現在のhandoff実装および同文書のLegacy project previews節と一致しない。実装は任意のpreview domainに対する限定認可を提供する。[handoff](../../crates/server-access/src/preview.rs)、[現在の運用節](../../docs/self-hosting/server-container.mdx#legacy-project-previews)

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

host updater の活動 blocker は管理対象 DB の `agent_runs`、`agent_run_attempts`、`agent_process_registry` に限定される。run / attempt が終端、registry が exited であることを要求し、null・未知状態や読めない DB / schema は idle と扱わない。旧構成で development も管理する場合は両方の DB が対象になる。[判定](../../deploy/developer/updater.py#L28-L37)、[DB 読取](../../deploy/developer/updater.py#L98-L111)

Goal の状態、queued follow-up、Workflow / orchestration の投影、共有 resource holder、定期実行設定、dev server・terminal・instance/build といった非 agent process は、この updater の blocker ではない。コンテナー置換でこれらの process は停止し、メモリ内 queue も失われ得る。これは [資源調停の所有・回復](../concepts/shared-resource-coordination.md)を解除する証拠ではなく、updater の起動可否に限った契約である。[除外対象のテスト](../../deploy/developer/test_updater.py#L94-L176)

maintenance gate を閉じ、HTTP readiness barrier で受理済み handler を drain してからコンテナーを freeze し、同じ agent 活動を再検査する。競合で新しい agent が見つかれば再開して待つ。queue GET による待機検査や Docker の全 process allowlist は行わない。idle を確認した場合だけ freeze したまま停止し、全永続 volume と設定を保存する。この停止は graceful shutdown ではなく SQLite WAL 回復を使う。[順序と失敗](../../deploy/developer/updater.py#L346-L387)、[HTTP barrier](../../crates/server/src/middleware/maintenance.rs)

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
