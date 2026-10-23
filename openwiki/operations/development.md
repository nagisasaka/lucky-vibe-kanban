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
  - id: openwiki-source-d20a82e2192a07c687b838cb
    resource: repo://crates/db/src/models/agent_runtime.rs
  - id: openwiki-source-f4e0f9b2ce513d3f8b8b070e
    resource: repo://crates/executors/src/executors/provider_adapter.rs
  - id: openwiki-source-5d78398344c565f028fc5984
    resource: repo://crates/local-deployment/src/process_host/journal.rs
  - id: openwiki-source-d4130322a85c68ce2ddc220a
    resource: repo://crates/server/build_metadata.rs
  - id: openwiki-source-32657d5732db65a52d550bc6
    resource: repo://crates/server/src/bin/generate_types.rs
  - id: openwiki-source-3651b7ce4535b584634e8bc8
    resource: repo://crates/server/src/remote_mcp/runtime_read.rs
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
  - id: openwiki-source-353b77936aed6c3d486f5cb9
    resource: repo://deploy/developer/updater.py
  - id: openwiki-source-8b0f6773cd6f7a2a989ad880
    resource: repo://deploy/server/nginx.test.mjs
  - id: openwiki-source-c5de8230bc8ac4c0c0000287
    resource: repo://deploy/server/release.test.mjs
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
  - id: openwiki-source-29176c627ccc31a24d3d6ab2
    resource: repo://tests/build-info/component.test.mjs
  - id: openwiki-source-3b8e596f6d78967bbf29a877
    resource: repo://tests/build-info/isolated.mjs
  - id: openwiki-source-a1c202ba8b2becaa93218bc0
    resource: repo://tests/server/run-workspace-tokens.mjs
  - id: openwiki-source-f7bef772fe7a00d12c00a997
    resource: repo://tests/server/specs/workspace-tokens.spec.ts
  - id: openwiki-source-67c9f08f4593b3acbf1fe7b8
    resource: repo://tests/terminal-latency/codex-fixture.mjs
  - id: openwiki-source-a8dd8312a18cc58d1cb88211
    resource: repo://tests/terminal-latency/latency.spec.ts
  - id: openwiki-source-a53d4e495dbfa4b6be9ca682
    resource: repo://tests/terminal-latency/README.md
  - id: openwiki-source-00fd62855c4297a48ed4a170
    resource: repo://tests/terminal-latency/RESULTS.md
  - id: openwiki-source-596c6cb75168f4c904bcc50d
    resource: repo://tests/terminal-latency/run.py
  - id: openwiki-source-966297deb875824dc95ee8d5
    resource: repo://tests/workflow/playwright.config.ts
  - id: openwiki-source-1d08facbf90704cd4c4c54d8
    resource: repo://tests/workflow/specs/card-context.spec.ts
generated: { by: "codex", at: "2026-10-22T07:38:38.993Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-23T19:15:59.449Z
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

## Terminal から次操作までの回帰試験

終了通知から次の送信までを調べるときは `tests/terminal-latency/` を使う。外部 Codex app-server の JSON-RPC だけを fixture に置換し、実 adapter・process host・監査・SQLite・HTTP/WebSocket・React・Chromium を通す。実 Codex cloud/model や認証サービスの相互運用を検証する試験ではない。[試験の境界](../../tests/terminal-latency/README.md#reproduce)・[provider fixture](../../tests/terminal-latency/codex-fixture.mjs#L1-L65)。原因と cleanup / drain の所有権契約は [Runtime の終了時 stdout](../architecture/agent-runtime.md#終了時の-stdout-所有権と-drain)を参照する。

repository toolchain と依存関係を用意し、root で frontend を `pnpm --dir packages/local-web run build`、両 binary を `cargo build --bin server --bin agent-process-host` で build してから `python3 tests/terminal-latency/run.py` を実行する。runner は `target/debug` の両 binary を使い、新規 `LVK_INSTANCE_DIR` と loopback の動的 port を設定する。継承した `VIBE_KANBAN_AGENT_PROCESS_HOST` を自身の binary に固定し、`LVK_PUBLIC_URL` を空にする点が、別 build や公開 URL の混入を避けるために重要である。Chromium は `LVK_E2E_CHROMIUM` で指定できる。source と binary の hash を保存し、browser test は fixture の実親 executable も照合する。[起動・provenance](../../tests/terminal-latency/run.py#L21-L84)・[親の照合](../../tests/terminal-latency/latency.spec.ts#L193-L202)

runner は終了時に自身の instance に属する process の停止を確認してから一時 DB/runtime を削除する。残存 process があればエラーにして一時領域を回復用に残す。監査由来の時刻、Host journal、fixture 観測などの診断資料は `test-results/terminal-latency-instance/` に残る。[cleanup と記録](../../tests/terminal-latency/run.py#L85-L130)

5ケースは正常終了、失敗、取消、WebSocket 再接続、終了後のページ再表示である。各ケースで活動中 follow-up の400拒否を確認し、Send 有効化後は実ボタンで次 Run を一つだけ開始して成功を確認する。再接続は実 frame を転送したまま切断だけを注入する。`canonicalAt` は HTTP poll の観測時刻であり DB commit 時刻ではなく、reload の値は navigation / loading 時間も含む。[拒否と接続](../../tests/terminal-latency/latency.spec.ts#L6-L93)・[終端と次の実行](../../tests/terminal-latency/latency.spec.ts#L114-L213)

[2026-10-20 の測定記録](../../tests/terminal-latency/RESULTS.md#前後の時刻と差分)には正常終了の9056→4050ms、各版5ケース成功が記録される。ただし各ケース各版1回の比較であり、本番の分位点や負荷時性能の保証ではない。実 Codex、Windows、高負荷は未確認と明記されている。この記録は過去の試験成績であって、この Wiki Sync での再実行結果ではない。[条件](../../tests/terminal-latency/RESULTS.md#測定条件)・[結果と限界](../../tests/terminal-latency/RESULTS.md#再現操作と観測結果)・[未確認事項](../../tests/terminal-latency/RESULTS.md#未確認事項仕様判断)

## Workspace トークン集計の検証

[累積値の契約](../concepts/workspace.md#workspace-累積トークン)を変更するときは、通知の解釈、耐久保存、読取と表示を別の入口から照合する。`cargo test -p executors -p db -p server -p local-deployment -p mcp --lib workspace_tokens` は adapter の包含・矛盾通知拒否、DB の resume / 再送・複数 Session・失敗/取消後の保持・unknown と 0・valid → None、journal replay、外部 MCP の共通集計と scope を対象にする。MCP crate にはこの filter に一致する試験がないため、crate 名がコマンドにあるだけで stdio を検証済みと扱わない。[adapter](../../crates/executors/src/executors/provider_adapter.rs#L3461-L3504)・[DB](../../crates/db/src/models/agent_runtime.rs#L2941-L3170)・[journal](../../crates/local-deployment/src/process_host/journal.rs#L527-L589)・[外部 MCP](../../crates/server/src/remote_mcp/runtime_read.rs#L279-L317)

ブラウザーと実 stdio の境界は root から `node tests/server/run-workspace-tokens.mjs` を使う。先に同 checkout の `target/debug/server` と `target/debug/vibe-kanban-mcp`、実 frontend dist、Playwright/Chromium を用意する。runner は `.build/tokens-instance-*` の一時 instance と動的 loopback port を使い、初回試験後に同 DB で server を再起動して保持を検査する。終了時は自身の instance 環境を持つ process の停止を確認してから削除し、残存時は失敗して一時 state を残す。[起動と二段階実行](../../tests/server/run-workspace-tokens.mjs#L13-L167)・[cleanup](../../tests/server/run-workspace-tokens.mjs#L168-L214)

ブラウザー fixture は保存行を投入し、unknown → 120 → 200、partial、内訳の 1/2 thread、5秒再取得、日英表示、390px 幅、reload と server restart、API と実 stdio MCP の JSON 一致、削除済み attempt 数 field の不在を確認する。provider は起動しないため、通知からの保存は上記 Rust fixture と組み合わせて確認する。[試験本体](../../tests/server/specs/workspace-tokens.spec.ts#L7-L191)

[開発時記録](../../docs/development/workspace-token-usage.md#検証記録)の成功報告は今回の Sync の再実行結果ではない。凍結された統合 source `066b2b4bd93d06d046865d66ae16bf1537e8b184` を adapter、schema/保存、API/MCP、UI、独立した試験定義から静的に照合した。入力にある正式 Integration は明示的な検証省略であり、今回も製品テストや有料モデル run は実行していない。実アカウント精度・実 Auth0 経由・全 provider・子 thread 独立計測・counter reset 後の差分の保証はない。無関係な過去の説明全体を再検証したものでもない。

## モバイル向け built preview

`pnpm run local-web:preview` はlocal frontendをbuildしてVite previewをforegroundで実行する。既存backendが必要で、既定frontend4030・backend4021・HOST 0.0.0.0。HMRはなくsource変更後は再起動してbuildし直す。出力を `dist-preview` に分け、Cargoが埋め込むassetsと混ぜない。Node heapの既定は4GiBで、同一portや不正portは起動前に拒否する。[preview script](../../scripts/start-web-preview.sh)。

通常開発HMR、built preview、配布imageは別の検証経路である。`mobile-preview:test` と `chat:test` は対応するNode test群、Workflow PlaywrightはUI fixtureであり、実機・provider認証・公開の成功を代替しない。[定義](../../package.json)。

## 独立 instance と固定 staging 候補

LVK 自体の変更は card ごとに別 instance 名・port を使って検証できる。`LVK_INSTANCE_DIR` は Cargo の debug/release と独立した絶対ディレクトリで、data（DB・設定・添付）、runtime（実行状態）、ports（発見情報）を分ける。既定 stable data と重なる path や symlink alias は拒否する。未指定なら従来の保存先を使う。HOME は agent 認証の保存先として維持し、instance の代わりに変更しない。同じ OS user に対する security sandbox ではない。[path 検証](../../crates/utils/src/instance.rs)、[data](../../crates/utils/src/assets.rs)、[運用契約](../../deploy/developer/STAGING.md)

`instance:build NAME [REF]` は frontend と server / process-host を一組で build し、完了時に build identity と source revision を記録する。通常 card の未 commit 作業は dirty として記録できる。`staging:build` は main 等の固定 commit を detached snapshot にし、失敗 build を runnable candidate にしない。debug assets の参照元なので、その snapshot を実行中に削除しない。[build と記録](../../deploy/developer/instance.mjs#L115-L193)

`instance:run NAME BUILD_UUID PORT` は foreground supervisor と instance ごとの run lock を持ち、build lock は別なので次候補の build と現在の実行を両立できる。staging は loopback 4020、任意の nginx ingress は HTTPS 8443 を使う。稼働中の agent / Workflow が終わってから旧候補を停止し、新候補へ切り替える。schema が進んだ DB に旧 binary だけを戻さず、対応する idle data snapshot が必要となる。共有 staging は [共有資源調停](../concepts/shared-resource-coordination.md)の対象であり、host updater の agent 活動判定には instance/build process を含めない。image 置換で staging 等の非 agent process は停止し、更新後の staging は明示的に起動し直す。[run](../../deploy/developer/instance.mjs#L193-L217)、[更新・回復手順](../../deploy/developer/STAGING.md)

stagingの認証stateはstableと独立しており、同じ`LVK_INSTANCE_DIR`でsetupキーを発行する。stagingの公開URLは8443を含む。cookieはportで隔離されないため、同じhostname/IPのstableとstagingには別ブラウザープロファイルを使う。[配置とcookieの運用契約](../../deploy/developer/STAGING.md#staging)、[instance環境](../../deploy/developer/instance.mjs#L44-L64)。方式変更と失効は [共通認証](../concepts/server-access.md)に従う。

### ビルド情報の検証境界

instance runner は `build.json` と同じ revision / dirty を build 環境へ注入し、種別を `development` に固定する。staging という環境名や Cargo profile から release と推定しない。直接 Cargo build の再評価、明示注入の優先順位、API の optional 契約は [ビルド時ソース識別](server-container.md#ビルド時ソース識別と-api-の契約)にまとめる。[注入と記録](../../deploy/developer/instance.mjs#L131-L195)

検証は三つの境界を分ける。`crates/server/build_metadata.rs` の unit tests は明示 release、未知・不正入力、development の tag 除外、実 Git の clean → 未追跡 dirty → clean と空 injection の優先を扱う。`node --test tests/build-info/component.test.mjs` は SSR で native button、dialog trigger、日英 label、40/64桁 SHA と欠落時の非表示を検査する。配布引数の接続は [release test](../../deploy/server/release.test.mjs#L37-L58)が静的に検査するが、image 実行の代用にはならない。[metadata tests](../../crates/server/build_metadata.rs#L93-L191)・[component tests](../../tests/build-info/component.test.mjs#L29-L74)

`node tests/build-info/isolated.mjs` は root の `target/debug/server` と `agent-process-host`、build 済み frontend、Chromium を前提に、一時 instance / SQLite と loopback を使う。別 Git checkout から起動し、実行時に偽の `LVK_BUILD_*` を渡しても実 binary の情報が返ることを確認する。実 metadata は試験開始時の checkout HEAD / dirty と一致する必要があるので、build 後に source 状態を変えた古い binary で実行しない。[準備と起動](../../tests/build-info/isolated.mjs#L20-L84)・[API の照合](../../tests/build-info/isolated.mjs#L119-L157)

ブラウザー試験は Tab / Enter / Space、Escape / Close、focus 復帰、390px / 320px の touch 操作と画面内収容を検査する。release・unknown・legacy 欠落のケースは `/api/info` の metadata だけを置き換える fixture であり、公開済み release image の試験ではない。mobile の実値ケースは dirty 表示を固定で期待するため、clean checkout では前提が一致しない点にも注意する。touch は Pixel 7 の Chromium emulation であり実 Android や screen reader 音声の保証ではない。[metadata 置換と操作](../../tests/build-info/isolated.mjs#L183-L309)・[ケースの区別](../../tests/build-info/isolated.mjs#L332-L417)。表示の責任分割は [共有 shell のビルド情報](../architecture/web-and-sync.md#稼働バイナリのビルド情報を表示する)を参照する。

終了処理は browser を閉じ、server 終了を待ち、PID・port・一時 instance の除去を記録する。証跡先は既定 `test-results/build-info`、`LVK_BUILD_INFO_EVIDENCE` で変更できる。[cleanup](../../tests/build-info/isolated.mjs#L434-L473)。今回の Sync は凍結 source `393670802114ed09b421a52d6dd186a6da5dc2f5` を build resolver、instance、API、context、UI と独立した試験定義から静的に照合した。凍結入力の開発時成功報告と正式 Integration の明示的な検証省略は別であり、この文書保守では製品テスト・Docker build・実機試験を再実行していない。無関係な過去の説明全体の再検証も意味しない。

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

認証の回帰検証は `crates/server-access/src/tests.rs`、公開MCPのJWT-to-tool検証、nginx ingress fixture、`tests/server-access/`のブラウザー経路を分けて読む。実Auth0/ChatGPTの受入はこれらと別である。[検証境界の記録](../../docs/development/server-access.md)、[配布の認証境界test](../../deploy/server/access.test.mjs)。
