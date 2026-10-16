---
type: guide
title: 開発・検証・運用文書の使い分け
description: 起動形態、検証の適用範囲、型と SQLx の更新、DB 回復の制約、リリース文書の位置づけを整理する。
tags: [development, testing, database, releases]
sources:
  - id: openwiki-source-651d1fb6c9e49916a916ab51
    resource: repo://Cargo.toml
  - id: openwiki-source-b41a0c4c19395b75cde58592
    resource: repo://CONTRIBUTORS.md
  - id: openwiki-source-5c2565a649f8be059d306209
    resource: repo://crates/db/src/lib.rs
  - id: openwiki-source-32657d5732db65a52d550bc6
    resource: repo://crates/server/src/bin/generate_types.rs
  - id: openwiki-source-88fc73cd214d9f8c3b051c4f
    resource: repo://crates/tauri-app/src/main.rs
  - id: openwiki-source-57eebdf400ca14f6e9abea48
    resource: repo://crates/tauri-app/tauri.conf.json
  - id: openwiki-source-7d788a7c8b1c5bf5122bab53
    resource: repo://crates/utils/src/assets.rs
  - id: openwiki-source-06fe728b6644f76e4e63a8f4
    resource: repo://crates/utils/src/instance.rs
  - id: openwiki-source-a854af1812d166ff4c31b521
    resource: repo://deploy/developer/instance.mjs
  - id: openwiki-source-f771a8041ae618a5a49437b9
    resource: repo://deploy/developer/STAGING.md
  - id: openwiki-source-8b0f6773cd6f7a2a989ad880
    resource: repo://deploy/server/nginx.test.mjs
  - id: openwiki-source-5b54a58d1b51cd490b0e7162
    resource: repo://package.json
  - id: openwiki-source-96cd4c94fb57fb4764608e3d
    resource: repo://packages/web-core/package.json
  - id: openwiki-source-b7793decf9d7c9ba48e57e0f
    resource: repo://rust-toolchain.toml
  - id: openwiki-source-d90097af5777425ac57c25b3
    resource: repo://scripts/prepare-db.js
  - id: openwiki-source-fae34e38f06cb798a007a45b
    resource: repo://scripts/start-web-preview.sh
  - id: openwiki-source-966297deb875824dc95ee8d5
    resource: repo://tests/workflow/playwright.config.ts
  - id: openwiki-source-1d08facbf90704cd4c4c54d8
    resource: repo://tests/workflow/specs/card-context.spec.ts
generated: { by: "codex", at: "2026-10-16T12:23:35.935Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-16T12:23:35.935Z
---

# 開発・検証・運用文書の使い分け

変更箇所から [システムの境界](../architecture/system.md) を選び、その境界の検証を行う。README は開発の入口だが、コマンドの実際の対象は package.json と各 Cargo manifest で確認する。

## 開発環境と起動形態

ルート package.json は Node >=20、pnpm >=8 を要求し、packageManager を pnpm@10.13.1 に固定する。Rust toolchain は nightly-2025-12-04。[root manifest](../../package.json)・[Rust toolchain](../../rust-toolchain.toml)

| 作業 | 入口 | 重要な境界 |
| --- | --- | --- |
| ローカル Web と backend | pnpm run dev | frontend/backend/preview の port を解決し並行起動する |
| Backend の再起動開発 | pnpm run backend:dev | agent-process-host を build してから server を実行する |
| Tauri desktop の開発 | pnpm run tauri:dev | 外部 Vite / backend に接続する debug 起動。production の embedded backend 起動とは異なる |

[各 script](../../package.json)

Tauri の `beforeDevCommand` は frontend と backend watch を並行起動する。debug build の window はその外部 frontend に接続し、production build だけが `server::startup::start()` で backend を内蔵起動してから window を開く。したがって desktop の開発試験を、production の embedded startup・completion watcher 配線を検証した証拠にしない。[開発前処理](../../crates/tauri-app/tauri.conf.json#L6-L12)、[debug / production 分岐](../../crates/tauri-app/src/main.rs#L176-L239)

backend の watch 起動は DISABLE_WORKTREE_CLEANUP=1 を設定する。[Workspace の期限切れ cleanup](../concepts/workspace.md) の挙動を検証するときは、この開発時の差を考慮する。Agent runtime の変更では server だけでなく独立 process host の binary も変更対象になる。[watch 定義](../../package.json)

旧 Cloud / relay の起動scriptと別Cargo workspaceは撤去された。local Board API と外部MCP / Runnerの検証は現行serverを対象にする。

## 検証の対象を取り違えない

| 検証 | 実際の対象・限界 |
| --- | --- |
| pnpm run check | local-web / web-core / ui の型、legacy path guard、Tauriを除く cargo check |
| pnpm run lint | local-web / ui の lint、Tauriを除く Clippy（qa-mode、all-targets）、未使用 i18n key 検査 |
| pnpm run backend:test | Tauriを除くRust tests。全workspaceを明示したCargo実行はTauriも含む |
| pnpm run workflow:e2e | 専用 Vite fixture に対する Chromium UI tests |
| web-core の test:wiki | Pipeline model と Wiki feature の Vitest。全 frontend tests の包括実行ではない |
| pnpm run server:check | server 配布 kit の Node tests。native nginx test の実行には nginx が必要 |

[コマンド定義](../../package.json)・[Wiki test script](../../packages/web-core/package.json#L100-L104)

通常のcheck/lint/testはTauriを明示除外する。desktopは `tauri:check` / `tauri:lint` / `tauri:test` という別入口で検証する。[Cargo構成](../../Cargo.toml)、[script](../../package.json)。

Workflow の Playwright は1 workerで専用 fixture を立ち上げる。fixture は製品の Workflow component/model を import するが、実 provider と DB を含む本番実行全体の test ではない。たとえば Card context test は折り畳み、read-only preview、保存 description との独立性を確認する。[test 設定](../../tests/workflow/playwright.config.ts)・[fixture](../../tests/workflow/fixture/src/main.tsx#L1-L24)・[Card context test](../../tests/workflow/specs/card-context.spec.ts)

実行制御の変更は [監査・投影の回復 tests](../architecture/agent-runtime.md)、Graph の変更は [Workflow の validation/planner tests](../concepts/workflow-attempt.md)、memory checkpoint は [Repository memory の冪等性 tests](../concepts/repository-memory.md) へ進む。画面上の成功と永続状態の成功を分けて検証する。

## モバイル向け built preview

`pnpm run local-web:preview` はlocal frontendをbuildしてVite previewをforegroundで実行する。既存backendが必要で、既定frontend4030・backend4021・HOST 0.0.0.0。HMRはなくsource変更後は再起動してbuildし直す。出力を `dist-preview` に分け、Cargoが埋め込むassetsと混ぜない。Node heapの既定は4GiBで、同一portや不正portは起動前に拒否する。[preview script](../../scripts/start-web-preview.sh)。

通常開発HMR、built preview、配布imageは別の検証経路である。`mobile-preview:test` と `chat:test` は対応するNode test群、Workflow PlaywrightはUI fixtureであり、実機・provider認証・公開の成功を代替しない。[定義](../../package.json)。

## 独立 instance と固定 staging 候補

LVK 自体の変更は card ごとに別 instance 名・port を使って検証できる。`LVK_INSTANCE_DIR` は Cargo の debug/release と独立した絶対ディレクトリで、data（DB・設定・添付）、runtime（実行状態）、ports（発見情報）を分ける。既定 stable data と重なる path や symlink alias は拒否する。未指定なら従来の保存先を使う。HOME は agent 認証の保存先として維持し、instance の代わりに変更しない。同じ OS user に対する security sandbox ではない。[path 検証](../../crates/utils/src/instance.rs)、[data](../../crates/utils/src/assets.rs)、[運用契約](../../deploy/developer/STAGING.md)

`instance:build NAME [REF]` は frontend と server / process-host を一組で build し、完了時に build identity と source revision を記録する。通常 card の未 commit 作業は dirty として記録できる。`staging:build` は main 等の固定 commit を detached snapshot にし、失敗 build を runnable candidate にしない。debug assets の参照元なので、その snapshot を実行中に削除しない。[build と記録](../../deploy/developer/instance.mjs#L115-L193)

`instance:run NAME BUILD_UUID PORT` は foreground supervisor と instance ごとの run lock を持ち、build lock は別なので次候補の build と現在の実行を両立できる。staging は loopback 4020、任意の nginx ingress は HTTPS 8443 を使う。稼働中の agent / Workflow が終わってから旧候補を停止し、新候補へ切り替える。schema が進んだ DB に旧 binary だけを戻さず、対応する idle data snapshot が必要となる。共有 staging は [共有資源調停](../concepts/shared-resource-coordination.md)の対象であり、stable 更新時は instance/build process も busy と扱う。[run](../../deploy/developer/instance.mjs#L193-L217)、[更新・回復手順](../../deploy/developer/STAGING.md)

## Docker 配布と同梱 binary

LVK の配布先は GHCR の Docker image のみ。root Dockerfile の `server` target を配布対象とする。[配布契約](../../docs/self-hosting/server-container.mdx)。
`server` と `agent-process-host`、
`vibe-kanban-mcp` を同じ image に同梱する。既定 MCP 設定は同梱バイナリを直接起動し、
npm から別バージョンを取得しない。

`npx-cli/` と旧 version validator は upstream 由来の保管コードであり、
現在の LVK の公開手順ではない。旧 NPX 文書も廃止案内へ置き換えた。
配布 image と Compose の更新・旧名 volume の引継ぎは
[サーバーコンテナー](server-container.md)を参照する。

## LVK の名称

製品表記と現行の設定接頭辞は LVK を使う。名称変更の未記録の動機や、既存利用者のデータ有無を実装から推定しない。
オリジナル Vibe Kanban の `VK_*`、native binary、保存先、外部サービスの識別子は
変更対象ではない。上流への帰属表記と実際の過去の記録は保持する。

| 境界 | 現在の契約 | 根拠 |
| --- | --- | --- |
| 実行設定 | `LVK_*` を読み書きし、process host へそのまま渡す。旧接頭辞への変換はしない。 | [ExecutionEnv](../../crates/executors/src/env.rs)、[HostExecutionEnv](../../crates/local-deployment/src/process_host.rs) |
| Card Context | `lvk:` marker を読み書きする。説明の編集では保存済み指示と明示的 opt-out を維持する。 | [Card Context](../../packages/web-core/src/features/pipeline/model/cardContext.ts) |
| 共有ファイル | `.lvk-shared` から repository storage を参照する。追跡済み mount・別 repository 向きのリンク・ユーザーファイルを上書きしない。watcher、Git diff、Docker context から除外する。 | [共有リンク](../../crates/workspace-manager/src/shared_resources.rs)、[除外](../../crates/utils/src/path.rs) |
| Git publication | `LVK-Memory-Source` / `LVK-Memory-Integration` / `LVK-Wiki-Publication` trailer を記録し、復旧時も同じ marker を検索して重複 commit を防ぐ。 | [Memory 復旧](../../crates/services/src/services/repository_memory.rs)、[Wiki 公開](../../crates/services/src/services/openwiki.rs) |
| publication lock | repository 共通の `lvk-publication.lock` 一つを保持する。この lock は外部の任意 Git 操作までは制御しない。 | [GitPublicationGuard](../../crates/git/src/publication.rs) |
| サーバー | Compose project/service は `lvk-server` / `lvk`。接続先は IP・DNS 共通で `LVK_HOST`、Certbot lineage は `lvk`。 | [Compose](../../deploy/server/compose.yaml)、[ACME](../../deploy/server/acme.py) |
| MCP・配布 | 既定 MCP は image に同梱した `vibe-kanban-mcp` を直接実行する。LVK の npm package を取得する preset は提供しない。 | [既定 MCP](../../crates/executors/default_mcp.json)、[接続手順](../../docs/integrations/vibe-kanban-mcp-server.mdx) |

## サーバー配布 kit の検証

`pnpm run server:check` は `node --test deploy/server/*.test.mjs` を実行する。設定・証明書・process supervision 等の検査と、nginx を実際に起動する TLS/Basic 認証・WebSocket・SSE・preview 境界の検査を含む。nginx がなければ後者は skip するため、全 subtest が実行されたかを結果で確認する。`LVK_TEST_NGINX` で実行ファイルを指定でき、`LVK_REQUIRE_NGINX_TEST=1` は欠落を失敗にする。[script](../../package.json)、[native test の前提](../../deploy/server/nginx.test.mjs)

この確認だけで Docker image の build・実配置・live agent 認証が成功したことにはならない。配布済み binary と作業 checkout の分離、永続 volume、停止・更新・復旧の契約は [サーバーコンテナー](server-container.md)に置く。[記録された検証限界](../../docs/self-hosting/server-container.mdx#validation-boundaries)

## 版番号の検証と選別移植の追跡

旧 npm 用の version parser は過去の `easy` 入力も互換用に受け付ける。
そのテストは現在の Docker image の build・公開成功を証明しない。
現在の配布検証は [release test](../../deploy/server/release.test.mjs) とサーバー配布文書を参照する。

上流変更の採否・LVK 向け適応・故障注入・実 Codex を使った MCP 受入は [選別移植の実装記録](../../docs/design/upstream-selective-backport-implementation.md)にある。各試験の source revision と実行対象を識別し、過去の成功を別 revision の成功と取り違えない。特に server と `agent-process-host` を同じソースから更新し、旧 Host の互換接続と新 Host の journal 回復を分けて試す。[Host の回復契約](../architecture/agent-runtime.md#接続断と確認済み終了を分ける)

Workflow fixture には draft / Undo、設定安全性、route loading、Runtime input、diff tree の harness がある。これは製品 component のブラウザー試験であり、認証済み provider・publication を含む実機試験とは別である。[fixture の入口](../../tests/workflow/fixture/src/main.tsx#L1-L29)

## 型・schema・SQLx を変更する場合

local API の生成元は Rust の generate_types binary であり、shared/types.ts と shared/schemas を生成・比較する。同じgeneratorがlocal Board互換用 `shared/remote-types.ts` も生成する。名称は旧Remoteサービスの存在を意味しない。API 変更は生成先だけを編集せず、Rust 側を更新して各 generator と --check を使う。[local generator](../../crates/server/src/bin/generate_types.rs)

prepare-db は一時 SQLite を作り、migration を適用してから cargo sqlx prepare を実行し、finally で一時 DB を削除する。--check でも一時 DB の作成と migration は行うため、単なる読み取り専用の型検査ではない。[SQLite 準備](../../scripts/prepare-db.js)。

本番 DB の migration checksum 不一致は、debug build と非 Windows ではエラーになる。Windows release は記録済み checksum を更新して再試行する分岐を持ち、その根拠として改行等の platform 差を code comment が挙げる。これは任意の schema 不一致を修復する保証ではない。起動時の schema guard は [システムの永続化境界](../architecture/system.md) を参照する。[platform 分岐と記録された理由](../../crates/db/src/lib.rs#L29-L73)

## 文書の適用範囲

- [CONTRIBUTORS](../../CONTRIBUTORS.md) は maintainer review、変更管理、生成ファイルを直接編集しないという方針を記録する。実装の挙動の証拠とは分けて読む。[変更管理](../../CONTRIBUTORS.md)・[生成物の方針](../../CONTRIBUTORS.md)
- [LVK release distribution](../../docs/easy-npx-npm-publish.md) は旧 npm 手順の廃止案内である。現行の Docker 公開・検証は [サーバー配布手順](../../docs/self-hosting/server-container.mdx)を参照する。
- 旧Mobile設計のremote-web / relay前提は現在使えない。built previewと[直接アクセス](../integrations/remote-access.md#ui-の直接アクセス)を確認する。
- [OpenWiki maintenance](openwiki-maintenance.md) は、通常開発の tests や release と別の生成・レビュー・公開 checkpoint を説明する。

この Wiki は実装理解のための派生知識であり、試験成績そのものではない。初期生成時の静的調査と、その後の選別移植で行った自動検証・MCP受入を区別する。再検証するときは上記の実装記録と現行コマンドを参照し、未実行や既存 skip を合格に数えない。
