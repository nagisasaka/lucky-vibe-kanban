---
type: guide
title: 単一サーバーコンテナーの運用境界
description: local LVK をサーバー上で動かす配布構成。HTTPS、認証、preview origin、永続化、停止と回復の契約。
tags: [server, docker, deployment, preview, persistence]
sources:
  - id: openwiki-source-b28e4ff9d0efb53d7fe91df4
    resource: repo://.github/workflows/publish-server.yml
  - id: openwiki-source-b50585f67dac89cd8eecc75d
    resource: repo://crates/executors/default_mcp.json
  - id: openwiki-source-d254ecaf5a9cc6c431ef182a
    resource: repo://crates/git-host/src/github/cli.rs
  - id: openwiki-source-040941dd1546f1e0a90b6c5f
    resource: repo://crates/git/src/cli.rs
  - id: openwiki-source-b688f8904e0728c38535a521
    resource: repo://crates/git/src/repository_read.rs
  - id: openwiki-source-0a300791bc056c6eee6a577a
    resource: repo://crates/mcp/src/bin/vibe_kanban_mcp.rs
  - id: openwiki-source-a6cad3d930b299ebbbd793a3
    resource: repo://crates/server-access/src/tests.rs
  - id: openwiki-source-d4130322a85c68ce2ddc220a
    resource: repo://crates/server/build_metadata.rs
  - id: openwiki-source-f72d699e94c301e4e47d097c
    resource: repo://crates/server/build.rs
  - id: openwiki-source-a9b6d4be264787a78357c9d6
    resource: repo://crates/server/src/routes/config.rs
  - id: openwiki-source-65c0f36670a161dc05040e69
    resource: repo://crates/server/src/routes/mod.rs
  - id: openwiki-source-ed8c84278dba8a1f45af40e9
    resource: repo://crates/server/src/startup.rs
  - id: openwiki-source-5837e5316279163dd8bfbfd3
    resource: repo://crates/utils/src/github_credentials.rs
  - id: openwiki-source-a854af1812d166ff4c31b521
    resource: repo://deploy/developer/instance.mjs
  - id: openwiki-source-f771a8041ae618a5a49437b9
    resource: repo://deploy/developer/STAGING.md
  - id: openwiki-source-45785e038b483538ab5c9894
    resource: repo://deploy/developer/test_updater.py
  - id: openwiki-source-353b77936aed6c3d486f5cb9
    resource: repo://deploy/developer/updater.py
  - id: openwiki-source-63e502af7564c9aae1780fd6
    resource: repo://deploy/developer/verify-access.mjs
  - id: openwiki-source-149ec24c22379256388dd413
    resource: repo://deploy/developer/verify-image.sh
  - id: openwiki-source-eaf96aacdc40d6db8dae4f8e
    resource: repo://deploy/server/acme.py
  - id: openwiki-source-83ada096aaf457a8ce0ab3cb
    resource: repo://deploy/server/bundle-source.sh
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
  - id: openwiki-source-c5de8230bc8ac4c0c0000287
    resource: repo://deploy/server/release.test.mjs
  - id: openwiki-source-89d0fb22262f0a0ac1df7d1e
    resource: repo://deploy/server/server.mjs
  - id: openwiki-source-229e22cb29e176ae1fe594e1
    resource: repo://deploy/server/server.test.mjs
  - id: openwiki-source-5fe6396817bf95b4040eebdb
    resource: repo://deploy/server/SOURCE.md
  - id: openwiki-source-26f2b454d009a5c3d05be35a
    resource: repo://deploy/server/workflow-source-protection.md
  - id: openwiki-source-bb1ebe868e35e9e500714501
    resource: repo://Dockerfile
  - id: openwiki-source-5b54a58d1b51cd490b0e7162
    resource: repo://package.json
  - id: openwiki-source-d64d3c5fa2f722c3a06ba2e6
    resource: repo://tests/embedded-browser-removal/controls.test.mjs
generated: { by: "codex", at: "2026-11-12T15:26:52.299Z" }
verified:
  - by: openwiki/0.7.2
    at: 2026-11-12T15:26:52.299Z
---

# 単一サーバーコンテナーの運用境界

server 配布は local LVK と coding agent を常時稼働サーバーへ置き、利用者のブラウザーから操作するための構成である。SQLite の local DB・Workspace・AgentRun を使う。[外部 MCP](../integrations/remote-access.md) は、この同じ local アプリに対する opt-in の入口である。ブラウザーを閉じてもサーバーの実行は続くが、コンテナー停止やホスト再起動はプロセスを止める。[製品の用途](#公開配置を準備して起動する)

信頼された利用者が共有する開発環境であり、Basic/Auth0認証がtenantごとのOS隔離を作るわけではない。LVK・nginx・agent は同じ非 root user `appuser`（UID/GID 10001）で動く。read-only secret mount は上書きを防ぐだけで、同じユーザーの agent からの読取を防がない。[信頼境界](#公開配置を準備して起動する)、[runtime user](../../Dockerfile)

## 配布 binary と開発 checkout

root Dockerfile の最終 `server` target は prebuilt server・隣接 `agent-process-host`・MCP CLI と開発 toolchain を含む。稼働 binary は `/usr/local/bin` に置かれ、`/repos` の LVK checkout を編集・build しても、それを管理しているアプリを置換しない。小さい `runtime` target は別の起動入口を持ち、server target の認証付き ingress と同じではない。[target と起動入口](../../Dockerfile)

Node 22、固定版 Codex/pnpm、repository の Rust toolchain などを image に置き、user の認証・cache は永続領域へ分ける。追加 SDK は派生 image の `/opt` や `/usr/local` に置くと、既存 home volume に隠されにくい。公開版・digest と対応する配布 kit を使い、文書の例の tag を公開済み成果物と推定しない。[image の構成](../../Dockerfile)、[派生 image の契約](#公開配置を準備して起動する)

配布用 Rust compile は chef cook と server build の両方で command-local の stable release 設定を使う。これにより依存と最終 binary の設定を揃え、runtime や子 Project へ profile 環境を追加しない。値と preview / 常用 dev との違いは [用途と profile の選択](development.md#用途と-profile-の選択)を参照する。[両 compile command](../../Dockerfile#L73-L107)

### Claude Code の同梱と認証

`server-tools` は native Claude Code を version と SHA-256 で固定し、amd64 を確認して公式配布物を取得し、checksum 検証後に実行可能にする。配置先は `/usr/local/bin/claude` で、永続 home mount に隠れない。小型 runtime target への追加ではない。CLI の更新は image 更新と結び付く。[構成と検証順](../../Dockerfile#L162-L173)

CLI 同梱はアカウント認証を含まない。利用者はコンテナー内で `claude --version` を確認し、`claude auth login` の browser 案内に従う。設定・認証情報は永続 home に保持する。LVK の Basic/Auth0 と provider 認証は別であり、[Claude profile の実行前診断](../integrations/agent-providers.md#claude-のサブスクリプション専用-profile)も CLI の存在だけでは成功しない。[利用手順](#provider-の実行アカウントを準備する)

公開CIのbuild後image smokeは撤去済みであり、通常shell・login shell・永続home mount後のClaude起動を公開前に検査しない。CLIの同梱と実アカウントlogin・モデル実行の成功は別に確認する。[現在のworkflow](../../.github/workflows/publish-server.yml)

LVK 自身の開発向けに、`server-tools` は sccache 0.18.0 を locked dependencies・`--no-default-features` で導入し、`/usr/local/bin/sccache` に置く。local disk cache 用の executable 同梱であり、global Cargo wrapper・cache directory・incremental 設定を自動変更しない。home volume に隠されない配置と、各 repository が利用を設定する境界を維持する。[同梱と意図](../../Dockerfile#L173-L180)。実 image での利用可否と cache hit は別途の検証対象である。

`less` は `server-tools` の apt package として導入され、`server-browser` を経て最終 `server` image に継承される。小型 `runtime` は `runtime-base` から分岐するため、この開発 toolchain の追加を継承しない。[stage の境界](../../Dockerfile#L136-L156)、[server への継承](../../Dockerfile#L182-L201)。任意の旧 development Compose も `LVK_IMAGE` を参照するため、利用できる toolchain は選択した image に依存する。checkout の変更だけで稼働中 image に追加されるわけではない。[開発 Compose の image 選択](../../deploy/server/compose.development.yaml#L1-L8)

公開CIには `less --version` や非TTY pipe出力を確認する後段smokeはない。配布kitの静的・native検証と完成imageの実行確認は別であり、今回Docker build/runや対話端末でのページ操作は確認していない。[配布kit検証](development.md#サーバー配布-kit-の検証)

LVKのsource開発入口は同梱sccacheを探索・起動・設定せず、rustcを直接使う。明示compiler wrapperは拒否し、既存tool・daemon・共有cacheは変更しない。image同梱やCIの利用方針とは別である。[現行入口](../../scripts/dev-cargo.mjs)・[廃止範囲](../../scripts/dev-cargo.mjs)。

### 旧 npx ランチャーの撤去と移行

旧 LVK npx ランチャーの npm packaging・download・展開・cache 管理は撤去済みで、互換ランチャーは提供しない。Docker のみを配布する方針に対応した変更であり、source build の入口は残る。[移行契約](#旧-npx-ランチャーの撤去と移行)・[現行 package](../../package.json)

| 以前の入口 | 現行の経路 |
| --- | --- |
| `npx lucky-vibe-kanban` | このページの Docker 配置、またはソース開発の `pnpm run dev` |
| `lucky-vibe-kanban mcp` / `--mcp` | 直接 build した `vibe-kanban-mcp --mode global`、または認証付き外部 HTTP MCP |
| `lucky-vibe-kanban --desktop` | Docker server または source 開発 server をブラウザーで開く。Tauri shell と build コマンドは廃止済み |

直接 build では `pnpm install --frozen-lockfile`、`pnpm --dir packages/local-web run build` の後、`node scripts/dev-cargo.mjs --purpose stable build --locked --bin server --bin agent-process-host --bin vibe-kanban-mcp` で同じcheckoutから三binaryを作る。`target/release/server` を意図したinstance設定で起動し、stdio clientには `target/release/vibe-kanban-mcp --mode global` の絶対pathを指定する。[開発時の検証範囲](development.md#検証の対象を取り違えない)と配布 image の検証を区別する。Docker もこの三つの binary を同梱する。[build と配置](../../Dockerfile#L98-L101)・[server image](../../Dockerfile#L199-L202)

stdio binary は既存 HTTP backend に接続するため、ランチャーの代わりに backend を自動起動するものではない。`--mode` 省略時は global。LOCAL-365で `--mode orchestrator` は廃止され、移行案内付きの明示エラーになる。旧設定を見直し、適切な場合だけ `--mode global` を指定する。globalには旧Workspace制限がないため黙った置換はしない。接続先の `VIBE_BACKEND_URL` / host・port 解決、内部資格情報と router の制約は [MCP client の接続境界](../integrations/agent-providers.md#外部-mcp-client--lvk)を参照する。[起動処理](../../crates/mcp/src/bin/vibe_kanban_mcp.rs#L23-L49)・[mode の検証](../../crates/mcp/src/bin/vibe_kanban_mcp.rs#L56-L97)。起動方法の移行は [HTTP MCP の認証・scope](../integrations/remote-access.md#認証付き外部-mcp)を免除しない。

撤去は既存アプリデータ・download 済み binary・`~/.vibe-kanban/bin` 等の cache の削除や、既存 launcher の遠隔無効化を行わない。agent CLI や第三者 MCP の npx 設定も別の責任範囲である。[保持する範囲](#旧-npx-ランチャーの撤去と移行)・[直接 LVK binary と第三者 preset](../../crates/executors/default_mcp.json)。古いローカル成果物が残ることと、現行 source がその launcher を提供することを混同しない。

Tauri 撤去も既存 desktop data や download 済み application を削除・移行しない。残る [browser の貼り付け・音・editor 接続](../architecture/web-and-sync.md#ブラウザー操作と廃止された-native-bridge)、[Web Push の同意と配送](../integrations/web-push.md)、[server OS notifier](../concepts/issue-collaboration.md#実行時の注意喚起)を個別に確認する。native shell の削除から共有 runtime の廃止を推定しない。[維持する範囲](#埋込みpreviewの廃止)

### ビルド時ソース識別と API の契約

ビルド情報は「この server binary を作ったソース」を説明する。現在の Workspace / 起動先 checkout、package version、image digest、配布の成功状態とは別の情報である。`build.rs` が取得値を `cargo:rustc-env` へ固定し、`BuildInfo::current()` がコンパイル時の `env!` から読むため、起動時の `LVK_BUILD_*` で上書きされない。`/api/info` の `version` は従来の `CARGO_PKG_VERSION` を維持し、別の任意 `build_info` に revision と nullable な kind / release_tag / dirty を返す。revision がなければ `build_info` 自体を省略する。[固定の境界](../../crates/server/build.rs#L18-L36)・[API 型と応答](../../crates/server/src/routes/config.rs#L93-L156)

入力の優先順位は一つの契約として扱う。

- `LVK_BUILD_REVISION` の明示値は空文字でも authoritative であり、Git fallback を無効にする。この経路では `LVK_BUILD_KIND`、`LVK_BUILD_RELEASE_TAG`、`LVK_BUILD_DIRTY` の未指定項目を checkout から補わない。
- revision が未指定の直接 Cargo build だけは、source root と Git top-level の canonical path が一致することを確かめて HEAD と未追跡ファイルを含む dirty を取得し、kind を `development` にする。別 repository 内に展開した source archive を親 repository の版と扱わない。
- 有効な revision は40桁または64桁の16進値で、小文字化する。不正 revision は情報全体を不明にする。kind は `development`、または有効な tag を伴う `release` のみ。tag は `v` 始まりで英数字・`.`・`-` のみを許し、release 以外では除去する。dirty は文字列 `true` / `false` だけを既知とし、それ以外は不明にする。nearest tag や package version から補完しない。

[優先順位と Git root](../../crates/server/build_metadata.rs#L25-L55)・[正規化と未知値](../../crates/server/build_metadata.rs#L56-L86)。取得不能はビルド情報を失うが、resolver がそれ自体を build error にする経路ではない。

Cargo build は明示identityの場合も含め、存在しない refresh path を `rerun-if-changed` に指定して毎回 Git 状態を再評価する。HEAD / index の監視だけでは未追跡変更と dirty → clean を見落とす、という理由がコードに記録されている。明示注入では環境変数の変更を監視する。metadata の解決は root `.env` の読込より前なので、この build script の dotenv 読込に `LVK_BUILD_*` の注入を期待しない。[再評価と読込順](../../crates/server/build.rs#L5-L36)

instance runner は `build.json` と同じ revision / dirty を注入する。既存の image 配布経路は source SHA と release tag を build 引数へ渡し、tag build を release、それ以外を development、dirty を false とする契約である。Docker builder の引数既定は空なので、注入しないローカル image に Git fallback や sidebar identity を期待しない。これらはビルド入力であり runtime 設定ではない。[instance の注入](../../deploy/developer/instance.mjs#L131-L195)・[Docker builder](../../Dockerfile#L83-L107)・[配布引数の静的回帰検査](../../deploy/server/release.test.mjs#L37-L58)・[運用契約](../../deploy/developer/STAGING.md#isolated-state)

UI はこの optional 情報を使い、短い SHA のボタンから詳細を開く。欠落時の非表示、Update との優先関係、mobile drawer の寿命は [共有 shell の表示契約](../architecture/web-and-sync.md#稼働バイナリのビルド情報を表示する)に置く。source identity の表示だけで公開済み image・実配布・validation 成功を保証しない。実 Git、API、SSR、keyboard / touch と release fixture の検証範囲および今回の静的調査の限界は [ビルド情報の検証境界](development.md#ビルド情報の検証境界)を参照する。

## HTTPS と起動前検証

Compose は host の `443` だけを nginx の `8443` へ公開する。Rust backend は `127.0.0.1:3000` を使い、supervisor は継承された HOST/PORT を上書きする。許可 origin は設定したアプリの HTTPS origin である。[port と mount](../../deploy/server/compose.yaml)、[backend 環境](../../deploy/server/server.mjs)

`LVK_HOST` は IP アドレスまたは scheme/port/path を含まない小文字 DNS 名を受け付ける。旧 `LVK_PREVIEW_DOMAIN` は警告して無視する。起動前に TLS 証明書の期間、アプリ名・独立 CSV 経路の全 host 名への対応、秘密鍵との一致を検証する。既存のnginx Basicを維持する配置ではbcrypt htpasswdも検証する。新規公開配置はhtpasswdなしで起動し、LVKの初期設定を完了するまで業務APIをロックする。volume 書込権と `nginx -t` も通ってからサービスを開始する。[設定](../../deploy/server/server.mjs)、[起動順](../../deploy/server/server.mjs)

基本 Compose は既存証明書を使う。任意の compose.acme.yaml は専用 Certbot companion と acme volume を追加し、TCP 80 を証明書検証専用に公開する。公開 IP または DNS 名のアプリと独立 CSV 経路の証明書を扱い、旧wildcard previewは提供しない。[ACME 構成](../../deploy/server/compose.acme.yaml)・[発行と更新](../../deploy/server/acme.py)。起動検査はブラウザーによる trust chain 検証の代わりではない。証明書・鍵・htpasswd は `/run/lvk-secrets` へ read-only mount し、欠けた host path を Compose が自動作成しない。更新・認証情報変更の具体手順は [運用文書](#証明書の更新を確認する)に置く。[mount](../../deploy/server/compose.yaml)、[証明書の責任](#公開配置を準備して起動する)

公開配置の認証はLVKのserver-accessが担う。管理者が `docker compose exec lvk server setup-token` で15分間有効な一度限りのキーを発行し、HTTPS画面でBasic/Auth0を設定する。Auth0は実ログイン試験と本人確認後に有効化する。既存htpasswdがある場合はnginx Basicも維持し、LVK側の設定後に運用者が `LVK_PROXY_AUTH=backend` へ切り替える。新規配置では事前のhtpasswd作成は不要。旧 `LVK_MCP_CONFIG` は拒否される。[認証手順](../concepts/server-access.md#運用者が初期設定と回復を行う)、[MCPのscope](../integrations/remote-access.md)、[生成されるingress](../../deploy/server/server.mjs)。管理認証のingressはWeb資格情報をbackendへ転送する一方、内部agent用のヘッダーを外部から受け渡さない。WebSocket upgrade を通し、proxy buffering/cache は無効にする。access log は query string・認証情報・agent 本文を含めず method/URI/status を記録する。[ingress 契約](../../deploy/server/nginx.conf.template)

## 独立アプリの CSV ルート

`LVK_PREVIEW_ROUTES=dev.example.com:4020,preview1.example.com:18443` は、名前ごとの外部 HTTPS を **LVK コンテナー内**の `127.0.0.1:<port>` の HTTP アプリへ転送する静的設定である。ホストや別コンテナーの loopback へは届かない。名前付き slot、preview UI、DB、動的ルート管理は作らず、アプリ起動・DNS・公開 URL/HMR・認証は各アプリの運用者が担う。[生成される経路](../../deploy/server/server.mjs#L248-L276)・[運用契約](#独立アプリの-csv-ルート)

### 入力と経路の境界

空または未設定なら無効。各項目は小文字の完全な DNS 名と10進 port で、項目の外側の ASCII space/tab だけを許す。末尾 dot・scheme・path・wildcard・IP literal・改行・空要素・重複 host・`LVK_HOST` との衝突は拒否する。DNS 名は最大253文字、port は1024–65535で先頭ゼロを許す。同じ port への別名は許可する。3000・3001・4021・4022・8443・8444は LVK/nginx 用に予約するが、4020は普通のアプリ port として使える。`dev` に特別な処理はない。[JS parser](../../deploy/server/server.mjs#L18-L68)・[Python parser](../../deploy/server/acme.py#L25-L55)

`LVK_PREVIEW_DOMAIN` は廃止済みで警告して無視し、CSVとの併用で旧preview機能が復活することはない。CSVは独立アプリへの静的転送として残る。[設定](../../deploy/server/server.mjs)・[移行](#埋込みpreviewの廃止)。

独立経路では nginx Basic を無効化し、LVK の server-access/handoff を経由せず upstream へ渡す。Host・Authorization・Cookie・Origin・WebSocket の情報はアプリへ保持し、HTTPS の forwarded 情報を設定する。`Proxy-Authorization` と `X-LVK-Internal-Token` は除去する。nginx の header 定義をこの server block で置き換えることが、親の LVK 用資格情報除去を継承しないための重要な境界である。LVK 側が認証済みでも、このアプリが認証を実装しなければ匿名で到達できる。cookie は sibling host 間へ資格情報を広げない host-only 運用を使う。[転送設定](../../deploy/server/server.mjs#L252-L274)・[認証境界の回帰](../../deploy/server/nginx.test.mjs#L332-L377)・[cookie の運用](#独立アプリの-csv-ルート)。旧previewからの移行は[廃止した機能](#埋込みpreviewの廃止)を参照する。

### 証明書と設定変更の寿命

同じ TLS 証明書が `LVK_HOST` と全 route 名を満たす必要があり、起動時と証明書 reload 前に全件を検査する。ACME companion は両 Compose から渡される同じ CSV を解析し、`LVK_HOST` が IP なら IP 識別子、各 route は DNS 識別子として、既存の `shortlived` profile と単一 `lvk` lineage へ注文する。HTTP-01 のため各名の DNS と port 80 到達性が必要で、旧preview domainは警告して無視する。[証明書検査](../../deploy/server/server.mjs#L296-L319)・[注文の生成](../../deploy/server/acme.py#L58-L96)・[companion 配線](../../deploy/server/compose.acme.yaml#L1-L42)

この実装は `LVK_HOST` を含む25識別子まで、つまり最大24 route を Certbot 呼出し前に許可する。これは既存 shortlived 運用を維持するための ACME 側の上限で、parser 自体に件数上限はない。超える場合は全名を満たす外部管理証明書と基本 Compose を使う。複数証明書への自動分割はない。[上限と単一注文](../../deploy/server/acme.py#L68-L96)・[101経路の parser 回帰](../../deploy/server/server.test.mjs#L483-L491)・[記録された判断](../../deploy/server/nginx.test.mjs)

route と Certbot 引数は process 起動時に固定するため、`.env` 変更後は対象 Compose service の再作成が必要である。restart や nginx reload だけでは変更した Compose 環境を取り込まない。ACME では先に `certificates` を再作成し、新しい全名を満たす証明書を待ってから `lvk` を再作成する。外部管理なら証明書を先に用意する。この再作成では稼働アプリ・agent の停止を見込む。[運用順序](#独立アプリの-csv-ルート)・[起動時設定](../../deploy/server/server.mjs#L457-L508)・[Certbot 引数の固定](../../deploy/server/acme.py#L159-L176)

証明書だけの更新は既存 route 設定を保ち、30秒周期で検出して nginx を reload する。名前不足・key 不一致・nginx 検査失敗なら新 fingerprint を採用せず、失敗を記録して次周期に再試行する。backend/agent の再起動は行わない。[検査と採用順序](../../deploy/server/server.mjs#L352-L374)・[reload と再試行](../../deploy/server/server.mjs#L508-L518)

JS/Python に parser を置く構造は、新しい配布 runtime 依存や mount を増やさず既存入口を使うという凍結 Change Manifest の判断と一致する。両者の差異は [共有 fixture と native nginx の検証](development.md#独立アプリの-csv-ルートを変更するとき)で監視する。長い有効 DNS 名が nginx の既定 hash bucket では失敗した記録に対応し、template は512を指定する。[設定](../../deploy/server/nginx.conf.template#L6-L11)・[再現と修正の記録](../../deploy/server/nginx.test.mjs)。実 ACME 発行・DNS・配布 image の成功はこの source 照合では確定しない。

<a id="preview-は別-origin"></a>

## 埋込みpreviewの廃止

旧authenticated wildcard preview、handoff、組込みproject browserは撤去済みである。外部ブラウザーとprivate tunnelを使うか、独立CSVルートで各アプリ自身の認証を設定する。旧previewの認証を、匿名の独立アプリへ自動で引き継ぐ保証はない。main-origin preview pathはingressで403のまま拒否する。[移行契約](#埋込みpreviewの廃止)・[ingress](../../deploy/server/nginx.conf.template)。

## 永続化と実行の寿命

| named volume | mount | 維持するもの |
| --- | --- | --- |
| home | `/home/appuser` | SQLite・設定・認証・agent Session・user cache |
| repos | `/repos` | 元の Git repository と未追跡ファイル |
| work | `/var/tmp` | worktree・共有資源・run 関連ファイル |

Compose project 名の変更は別 volume の選択になる。同じ named volume の再利用はデータを維持するが、実行中プロセスは復元しない。custom worktree root を mount 外へ置けばこの永続化の対象外になる。[保存先](../../deploy/server/compose.yaml)、[永続データの契約](#公開配置を準備して起動する)

`.lvk/shared/` の意味は [Workspace の共有領域](../concepts/workspace.md#共有ディレクトリの意味)と同じで、Cargo target 等を自動設定するものではない。停止した Goal や保守 run は保存状態と所有者の規則に従って確認する。コンテナー再起動だけで任意の処理を自動継続する保証はない。[再起動後の確認](#公開配置を準備して起動する)

supervisor は server と nginx を process group として起動する。一方の終了・spawn 失敗ではもう一方へ SIGTERM を送り、既定 120 秒後に残存 group へ SIGKILL を送る。外部 SIGTERM/SIGINT も同じ停止手順を使い、Compose の停止猶予は150秒である。これは個々の AgentRun の [監査付き取消](../architecture/agent-runtime.md#取消と実プロセスの終了)とは別のコンテナー停止境界である。[supervision](../../deploy/server/server.mjs)、[猶予](../../deploy/server/compose.yaml#L21)

healthcheckは内部`/api/health`の200と、公開identity・CA/chain・残存6時間超を含むTLS検査を行う。公開rootの期待値はmaintenance時503、既存proxy Basicありなら401、backend認証なら200である。新規公開配置のロック状態も正常な初期状態であり、healthyだけでは初回認証やagent認証の完了を示さない。運用文書は unhealthy のみでは再起動しないことを明示する。サービス終了時の restart policy と health status を分けて診断する。[probe](../../deploy/server/healthcheck.mjs)、[再起動契約](#公開配置を準備して起動する)

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

LVK の画面から行う Git/PR には [リポジトリ別 PAT](../integrations/github-review.md#リポジトリ別-pat-の所有と管理)も利用できる。agent terminal の push やホスト updater の認証とは適用範囲が異なるため、画面で token を保存したことを任意 shell の認証済み状態とみなさない。[適用範囲](../integrations/github-review.md#リポジトリ別-pat-の所有と管理)

ホストは許可 repository・source SHA・digest・公開 workflow 成功を照合して候補を検査する。現在の自己開発手順は card ごとの独立 instance → main の固定 staging 候補 → release → stable 更新である。staging は built frontend と既定 preview 用途の debug binary を使い、source watcher を走らせない。preview は `instance.mjs` から専用 Cargo 入口を呼び、source checkout の target から候補へコピーする。常用 dev bundle の release 最適化とは別であり、共有 cache の有無と fallback を含む [build 契約](development.md#用途と-profile-の選択)を参照する。標準 updater の対象は stable の lvk のみで、旧 development サービスは互換オプションとなる。詳しい操作と一度限りのホスト移行は [STAGING.md](../../deploy/developer/STAGING.md)、独立状態の契約は [開発と検証](development.md) を参照する。

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

canonical 運用手順は agent を停止・完了させ、LVK を止めて三つの volume と設定・secrets を整合した組としてバックアップする。rollback は古い image だけを新 DB に向けず、対応する全 volume のバックアップを同じ内部パスの別 recovery deployment へ戻して検証する。schema migration と Git worktree metadata を分離して復旧しない。[バックアップと rollback](#公開配置を準備して起動する)

回帰検証は設定・証明書・停止を Node tests、TLS/認証/preview 分離を native nginx fixture、公開 URL を Rust/TypeScript tests で扱う。これらは実サーバー配置成功や公開 registry の存在を証明しない。native nginx が無い場合の skip と必須化は [開発時の検証](development.md#サーバー配布-kit-の検証)に置く。[認証・分離の test](../../deploy/server/nginx.test.mjs)、[停止の test](../../deploy/server/server.test.mjs)、[記録された未検証範囲](#公開配置を準備して起動する)

## 公開配置を準備して起動する

Docker Engine/Compose v2、固定されたpublic IPまたはDNS名、image/repository/build用の容量を準備する。公開Composeはhostの443だけをアプリへ渡し、ACME利用時の80はchallenge専用にする。raw backendやproject portを追加公開しない。使用imageと同じsource tagの `deploy/server/compose.yaml`、`compose.acme.yaml`、`acme.py`、`.env.example`、`.gitignore` を専用配置directoryへ揃える。[Compose](../../deploy/server/compose.yaml)・[ACME配線](../../deploy/server/compose.acme.yaml)

`.env` に実在を確認した `LVK_IMAGE=ghcr.io/OWNER/REPOSITORY@sha256:DIGEST`、`LVK_HOST`、ACME利用なら `COMPOSE_FILE=compose.yaml:compose.acme.yaml` を設定する。Let's Encrypt利用規約に同意した場合だけ `LVK_ACME_AGREE_TOS=yes` とし、emailは任意。Compose project名を保持し、意図しない空volumeの選択を避ける。IPv6のLVK_HOSTには括弧を付けず、browser URLでは括弧を使う。ChatGPT接続にはdomain名を使い、IP指定によるOAuth検出の可否をTLSのIP対応から推定しない。[設定例](../../deploy/server/.env.example)・[設定検査](../../deploy/server/server.mjs)

配置directoryで `secrets/tls` を作り、UID/GID 10001が読めるdirectory mode 750とする。既存の証明書や秘密を上書きしない。外部issuerを使う場合は `COMPOSE_FILE=compose.yaml` とし、同directory内へ全名対応の `fullchain.pem` と `privkey.pem` をUID/GID 10001・mode 640で配置する。symlinkはmount内で解決できなければならない。ACME volumeにはaccount/renewal設定が入り、agentからはmountされない。[secret検査](../../deploy/server/server.mjs#L272-L326)・[export](../../deploy/server/acme.py)

`docker compose config --quiet`、`docker compose pull`、`docker compose up -d`、`docker compose ps`、`docker compose logs --tail=100 lvk` の順で確認する。HTTPS画面を開き、[初回設定](../concepts/server-access.md#運用者が初期設定と回復を行う)を完了する。新規配置で業務APIがlockedなのは初回設定前の正常状態である。既存htpasswdは自動撤去せず、LVKへのログイン確認後に `LVK_PROXY_AUTH=backend` を設定してserviceを再作成する。旧development serviceは `LVK_DEV_PROXY_AUTH` を使い、backend認証のみではVite dev配信を使わずbuilt frontend previewを使う。[起動とproxy選択](../../deploy/server/server.mjs#L434-L512)・[開発用起動](../../deploy/server/development.mjs)

## Provider の実行アカウントを準備する

LVK認証とprovider loginを別に行う。標準Composeでは `docker compose exec lvk codex login --device-auth` と `docker compose exec lvk codex login status` で実行accountを確認し、Claudeは `docker compose exec lvk claude auth login` を行う。資格情報・native sessionは永続homeの秘密として扱う。CLIはimageの固定版に従い、login statusだけでモデル実行成功を主張しない。実行前条件とsubscription専用profileは [provider連携](../integrations/agent-providers.md)を参照する。

Git identityは運用者自身の値を実行userへ設定し、repositoryを `/repos` へcloneする。host上だけの `gh`/SSH認証はコンテナーuserへ自動伝搬しない。LVK-owned Git/PRにはrepository別PATを、任意agent shellにはそのaccount固有の認証を使う。Docker socketやhost home、cloud管理者credentialをmountして解決しない。nested sandboxエラーは個別に診断し、一律のprivileged化で回避しない。

## 証明書の更新を確認する

ACME companionは成功時1時間、失敗時15分で再確認し、更新不要でもexportを修復する。port 80とacme volumeを維持し、`docker compose logs certificates` と外部TLS監視を確認する。残存6時間以下はhealth不良になるが、Dockerのhealth statusだけでは通知・再起動を保証しない。[renewal loop](../../deploy/server/acme.py#L151-L179)・[health probe](../../deploy/server/healthcheck.mjs)

既存lineageの更新経路をdry-runする場合、配置directoryで `docker compose stop certificates` により競合Certbotを止め、次を実行する。

```bash
docker compose run --rm --no-deps --service-ports --entrypoint certbot \
  certificates renew --cert-name lvk --dry-run --no-random-sleep-on-renew
```

成功・失敗のどちらでも `docker compose start certificates` で定期確認を戻す。通常確認でforce-renewalを繰り返さない。外部issuerではmatching fullchain/keyを所定permissionで置換し、LVKの検証とreload成功を確認する。route/host自体を変更した場合は上記のservice再作成順序に従い、単なる証明書reloadと区別する。

## 更新前のバックアップと復元検証

手動運用ではagentを完了/停止させ、未解決のresource operation・Runner・Goalを確認してからLVKを停止する。コンテナー外のRunnerを含む実processの停止は別に確認する。停止中にSQLite本体と存在するWAL/SHM、Git repository/worktree metadataを同じ時点で退避する。DBだけのcopyや異なる時点のhome/repos/workの組合せを復元用backupにしない。[mount範囲](../../deploy/server/compose.yaml#L18-L37)・[WALを含めるupdater](../../deploy/developer/updater.py#L239-L258)

保存対象はhome/repos/workの三volume、custom mount、配置 `.env` とCompose一式、`secrets`（TLS・残存htpasswd）、ACME volume、`access.key` / `access.configured`、provider/Git資格情報、設定、native sessionとjournalである。Runner data rootはWindows側で別に保全する。secretを含むためbackup destinationをprivateにし、暗号化backupを使用する。Compose project名とvolume名、実image digest/source identity、内部path、DBの版を一緒に記録する。[認証鍵](../concepts/server-access.md#永続化閉鎖回復)・[Runner journal](../concepts/shared-resource-coordination.md#runner-の結果が不明になったとき)

配置directoryから手動退避する例を示す。各commandの成功を確認し、一つでも失敗したarchiveを更新可の証拠としない。

```bash
umask 077
docker compose stop
backup_dir="backups/$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -p "$backup_dir"
docker compose run --rm --no-deps -T --entrypoint tar lvk -C /home/appuser -czf - . > "$backup_dir/home.tar.gz"
docker compose run --rm --no-deps -T --entrypoint tar lvk -C /repos -czf - . > "$backup_dir/repos.tar.gz"
docker compose run --rm --no-deps -T --entrypoint tar lvk -C /var/tmp -czf - . > "$backup_dir/work.tar.gz"
```

`.env` / Compose / secretsとacme volumeは同じ停止期間中に秘密backup手順で別に退避する。各archiveのexit status・`tar -tzf` による読取・SHA-256を確認し、後でchecksumと展開を検証できるようにする。`down -v`、volume prune、hostの `/var/tmp` 一括cleanupを使わない。named volumeとworktreeの内部pathを維持する。[checksumを保存・復元前検査する実装](../../deploy/developer/updater.py#L239-L258)・[復元guard](../../deploy/developer/updater.py#L298-L339)

退避確認後に新image digestを選び、pull/up後のhealthとログ、既存Card・会話・Repo・共有file、使い捨てCardでの通常agent応答を確認する。割込みGoalは状態を調べて意図的に再開し、再起動だけでの継続を約束しない。

手動rollbackでは新コンテナーを止め、対応する旧image/binaryと**同じ時点の全volume・設定**を、同じ内部pathを持つ別のrecovery deploymentへ復元する。元volumeを同時にwriterへmountしない。そこでDB整合性、認証、Repo/worktree、会話と小さいagent実行を確認してからtrafficを切り替える。migration済みDBへ旧binaryだけを向けない。復元確認が済むまで元deploymentとbackupを保持する。任意host updaterの自動rollbackは閉じたmaintenance gateと保存済みbackupを使う独自transactionであり、この手動回復手順と混同しない。[自動rollbackの順序](../../deploy/developer/updater.py#L298-L339)


## 固定Workflow報告工程の配布依存

LOCAL-372のserver-tools stageはbubblewrap/socatを導入し、/usr/bin/bwrap --versionとsocat -Vで存在を確認する。server imageがこれを継承する。稼働中containerへのbinary配置やbundle置換だけで依存を導入したことにはならず、新しいimage構築・container切替は別の明示許可を必要とする。[Dockerfile](../../Dockerfile#L145-L178)

実行時には既存hostのuser/PID/IPC/UTS/mount namespace、native sandboxのnested/network条件も成立しなければならない。binaryの存在はnamespace成立の証明ではない。承認済み環境で使う前に[ソース保護とtmp/cacheの同時検証手順](../../deploy/server/workflow-source-protection.md)を実adapterで確認し、拒否時はerrorを保持して停止する。原因未確定の拒否を繰り返したり、seccomp/AppArmor/capabilityを緩和して通したりしない。今回の統合ではimage構築・container起動・実provider完走は未実施である。[検証範囲](development.md#local-372の統合検証と未実証の保護成功経路)


## 稼働版ヘルプ用の公開source同梱（LOCAL-375）

Dockerの `bundled-source` stageは固定された公開upstreamから `LVK_BUILD_REVISION` の完全SHAをfetchし、そのcommitの全祖先とtree/blobを同梱する。全branch/tagの同梱や先行shallow制限は行わない。取得不能、非完全SHA、版不一致、欠損objectは失敗にし、最新HEADへfallbackしない。`.dockerignore` の `.git` 除外とCI shallow checkoutは維持し、callerのcredentials/hooks/configや外部object依存をコピーしない。[取得helper](../../deploy/server/bundle-source.sh)・[shallow/no-.git回帰](../../deploy/server/source.test.mjs)

runtime-baseに `/opt/lvk/source` を `10001:10001` 所有で配置し、serverとagentのGit CLI/libgit2が同じ非root実行userで読む。HOME・`/repos`・`/var/tmp` volume外なので新imageがsourceを置き換える。wildcard safe.directoryや所有者検査の一律無効化は追加しない。`LVK_SOURCE_PATH` はserver側の参照path overrideであり、APIから任意rootを受けるものではない。[配置](../../Dockerfile#L120-L166)・[serverの版検査](../../crates/server/src/bundled_source.rs)

binaryとsourceは同じrevision引数を使い、CIはimage build直前にcheckout HEADと `GITHUB_SHA` の一致、tracked/index差分の不在を検査する。manual Docker buildではclean contextと公開SHAの対応を利用者が保つ必要があり、helperだけでcontext全体の一致を証明できない。ローカルmainにしかない未公開SHAは公開upstreamから取得できないため、今回の統合完了からimage build可能とは推定しない。[配布workflow](../../.github/workflows/publish-server.yml)・[契約と容量測定](../../deploy/server/SOURCE.md)

公開CIはbuild前の翻訳・設定・source照合とbuildを維持し、成功後は既存のtag push条件で直ちにpublishする。build後のimage smoke・fresh install・format検査と公開CI専用smoke scriptは撤去済みであり、代替の後段検査を追加していない。manual dispatchはbuildだけでpublishしない。[workflow](../../.github/workflows/publish-server.yml)・[既存構成試験](../../deploy/server/release.test.mjs)

host updaterは別経路の `verify-image.sh` → `verify-access.mjs` を保持する。使い捨てHOME/repos/work volumeと非root userでbinary/source版一致、Git履歴、正規Basic認証のlibgit2読取を試みる。これは公開CIの保証ではない。既知の未解決事項として、`HEAD^` がmerge commitの場合、direct diff要求にbase/parent指定がなく「Merge commit requires explicit 1-based parent」で失敗し得る。今回この問題は修復・再現試験の対象外とし、host検証成功とは報告しない。[updater呼出](../../deploy/developer/updater.py#L213)・[保持script](../../deploy/developer/verify-image.sh)・[読取要求](../../deploy/developer/verify-access.mjs#L88-L120)・[parent契約](../../crates/git/src/repository_read.rs#L556-L577)

今回のコード検証SHAは `b92bb02795fd16dc64972357f94bd1678975a6be`。固定base `3b57177b` へsource `51f6f9ba` を競合・修復なしで統合し、sourceとtreeが一致する。管理検証で既存release3件、対象3file整形、tree一致と差分空白が成功した。初回yaml依存不足はlocked install後に同一コマンド成功で解消した。包括build/check/lint/backendとDocker/E2Eは差分と既存SHA付き証拠に基づくagent判断のskipであり、新しいpassではない。完成image起動・認証・保持volume更新は未検証で、公開CIからその保証が減る。製品認証変更はない。Wikiはprepareで原本journal保存成功後の別commitであり、main反映は担当Run終了後のhost結果で確認する。GitHub push・tag・配布公開・デプロイ・稼働変更は実施しない。

同梱sourceは稼働binaryから独立した参照checkoutであり、編集しても稼働版は変わらない。UI/MCPの一般ヘルプは既存providerの認証・料金・REVIEW会話を使い、無指示の開発・build・再起動に進まない。履歴countとGit pack/checkoutサイズは [SOURCE.md](../../deploy/server/SOURCE.md) の実行user向けcommandで測定し、image layer/downloadサイズとは分けて記録する。

### Basic設定後のsmoke認証

`verify-access.mjs` は初回Basic設定まで有効なsetup Cookie/CSRFを使う。設定保存でsessionが失効するため、それ以後のrepository/read POSTではCookieを送らず、request共通のBasic Authorizationと `X-LVK-CSRF: basic` を使う。失効setup CSRFの再利用やCSRFの不足・誤りは403、無認証は401、正規Basic/CSRFだけ200となる契約を既存Access fixtureで検査する。[client](../../deploy/developer/verify-access.mjs#L14-L31)・[read](../../deploy/developer/verify-access.mjs#L88-L120)・[境界回帰](../../crates/server-access/src/tests.rs#L175-L247)

read失敗のassert診断はmethod/path、operation kind、HTTP statusを示し、response body全体やpassword/Cookie/CSRF/Authorization値を出さない。認証・CSRF製品コードを緩和する修正ではない。新規候補 `4aee2d1277a39dafc753d806566b68648e04b732` 上の認証23件と配布3件、整形/Clippyは成功したが、完成imageと新版実HTTPは未検証で次CI/ホスト隔離環境へ残す。[検証と省略の詳細](development.md#basic-smokeと整形の追加統合)
