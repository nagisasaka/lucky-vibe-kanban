---
type: guide
title: 開発・検証・運用文書の使い分け
description: 起動形態、検証範囲、型とSQLx、DB回復、指示配信変更と切替bundleの検証境界を整理する。
tags: [development, testing, database, releases]
openwiki_generated: true
sources:
  - id: openwiki-source-c8500b43a8153df40d055c87
    resource: repo://.lvk/preview/vite.config.mjs
  - id: openwiki-source-651d1fb6c9e49916a916ab51
    resource: repo://Cargo.toml
  - id: openwiki-source-b41a0c4c19395b75cde58592
    resource: repo://CONTRIBUTORS.md
  - id: openwiki-source-0afb1e125b616abe2a64c15f
    resource: repo://crates/db/migration_compat/project_keys_multi.sql
  - id: openwiki-source-32853d735a8ae91f8c088286
    resource: repo://crates/db/migration_compat/README.md
  - id: openwiki-source-4169d2cd1d27a6243e90c06d
    resource: repo://crates/db/src/bin/migrate_compat.rs
  - id: openwiki-source-5c2565a649f8be059d306209
    resource: repo://crates/db/src/lib.rs
  - id: openwiki-source-7b783ec1a704bf9dadb9c003
    resource: repo://crates/db/src/migration_compat.rs
  - id: openwiki-source-706d3e4ab2b931252207b0e2
    resource: repo://crates/db/src/models/agent_runtime_reservation_tests.rs
  - id: openwiki-source-d20a82e2192a07c687b838cb
    resource: repo://crates/db/src/models/agent_runtime.rs
  - id: openwiki-source-8856c7b6451960c4ee31ae56
    resource: repo://crates/executors/src/account_usage_tests.rs
  - id: openwiki-source-22b82518475034ade38104e6
    resource: repo://crates/executors/src/executors/codex.rs
  - id: openwiki-source-c88a7c6fabdac15166554032
    resource: repo://crates/executors/src/executors/codex/client.rs
  - id: openwiki-source-f4e0f9b2ce513d3f8b8b070e
    resource: repo://crates/executors/src/executors/provider_adapter.rs
  - id: openwiki-source-788fa2f6f1a3d449a1a9efe4
    resource: repo://crates/local-deployment/src/agent_run_port.rs
  - id: openwiki-source-5d78398344c565f028fc5984
    resource: repo://crates/local-deployment/src/process_host/journal.rs
  - id: openwiki-source-c6b92f5dd86f9cc046935fa4
    resource: repo://crates/mcp/src/task_server/tools/sessions.rs
  - id: openwiki-source-d4130322a85c68ce2ddc220a
    resource: repo://crates/server/build_metadata.rs
  - id: openwiki-source-32657d5732db65a52d550bc6
    resource: repo://crates/server/src/bin/generate_types.rs
  - id: openwiki-source-448d823f6734081b48eb5350
    resource: repo://crates/server/src/remote_mcp/attachment_tests.rs
  - id: openwiki-source-3651b7ce4535b584634e8bc8
    resource: repo://crates/server/src/remote_mcp/runtime_read.rs
  - id: openwiki-source-6c4d6d22a9d50e0def61c50d
    resource: repo://crates/server/src/routes/workspaces/integration.rs
  - id: openwiki-source-66c6304230dff031e0d0b855
    resource: repo://crates/services/src/services/config/editor/mod.rs
  - id: openwiki-source-ae8c631b5b4385c934e47d60
    resource: repo://crates/services/src/services/config/versions/v9.rs
  - id: openwiki-source-7d788a7c8b1c5bf5122bab53
    resource: repo://crates/utils/src/assets.rs
  - id: openwiki-source-06fe728b6644f76e4e63a8f4
    resource: repo://crates/utils/src/instance.rs
  - id: openwiki-source-7272325fd3387a1cecfd5cbc
    resource: repo://crates/utils/src/process/owned_group.rs
  - id: openwiki-source-a854af1812d166ff4c31b521
    resource: repo://deploy/developer/instance.mjs
  - id: openwiki-source-f771a8041ae618a5a49437b9
    resource: repo://deploy/developer/STAGING.md
  - id: openwiki-source-bf1c49e27a5579a9fdbab2e9
    resource: repo://deploy/developer/switch/bundle.py
  - id: openwiki-source-a764bf933c082e67ed4b6192
    resource: repo://deploy/developer/switch/launcher.py
  - id: openwiki-source-668dcd1cbb12520ab064b910
    resource: repo://deploy/developer/switch/process_drain.py
  - id: openwiki-source-417b2b5ecb26c89c1b06df88
    resource: repo://deploy/developer/switch/README.md
  - id: openwiki-source-9e0cfce0846bb98d2dc47de0
    resource: repo://deploy/developer/switch/start_binary.py
  - id: openwiki-source-67b7631c2182dcc02247e199
    resource: repo://deploy/developer/switch/start_support.py
  - id: openwiki-source-55186e23cd9f537c0d522a81
    resource: repo://deploy/developer/switch/test_real_server.py
  - id: openwiki-source-353b77936aed6c3d486f5cb9
    resource: repo://deploy/developer/updater.py
  - id: openwiki-source-85c8f44da40d30a5a7a1f9c7
    resource: repo://deploy/server/acme.test.py
  - id: openwiki-source-8b0f6773cd6f7a2a989ad880
    resource: repo://deploy/server/nginx.test.mjs
  - id: openwiki-source-c5de8230bc8ac4c0c0000287
    resource: repo://deploy/server/release.test.mjs
  - id: openwiki-source-229e22cb29e176ae1fe594e1
    resource: repo://deploy/server/server.test.mjs
  - id: openwiki-source-bb1ebe868e35e9e500714501
    resource: repo://Dockerfile
  - id: openwiki-source-5b54a58d1b51cd490b0e7162
    resource: repo://package.json
  - id: openwiki-source-96cd4c94fb57fb4764608e3d
    resource: repo://packages/web-core/package.json
  - id: openwiki-source-a6112f33b0e8d6ebc86d6702
    resource: repo://plugins/lvk/skills/lvk-work/SKILL.md
  - id: openwiki-source-b7793decf9d7c9ba48e57e0f
    resource: repo://rust-toolchain.toml
  - id: openwiki-source-6b8120d95640edd862b2bff6
    resource: repo://scripts/check-i18n.mjs
  - id: openwiki-source-4460e7b365a8c7784f4bcfd6
    resource: repo://scripts/check-unused-i18n-keys.mjs
  - id: openwiki-source-8ea6c46d63953a7104f5dd97
    resource: repo://scripts/dev-cargo.mjs
  - id: openwiki-source-934f1cc67f10f765aeed70ed
    resource: repo://scripts/dev-cargo.test.mjs
  - id: openwiki-source-75a99f8fc3a29ed0c6adb3ab
    resource: repo://scripts/package-orchestration.py
  - id: openwiki-source-d90097af5777425ac57c25b3
    resource: repo://scripts/prepare-db.js
  - id: openwiki-source-fae34e38f06cb798a007a45b
    resource: repo://scripts/start-web-preview.sh
  - id: openwiki-source-bfd53bb2381141e59ae3273f
    resource: repo://scripts/ui4-preview.py
  - id: openwiki-source-b05e5a1a5a499bbc87e50965
    resource: repo://skills/lvk-orchestration/references/auto-merge.md
  - id: openwiki-source-8dce97867754548fb32565eb
    resource: repo://skills/lvk-orchestration/references/execution.md
  - id: openwiki-source-90505d6ad74573a5fb6c6b3b
    resource: repo://skills/lvk-orchestration/references/investigation.md
  - id: openwiki-source-6d88df283621a548f8021464
    resource: repo://skills/lvk-orchestration/SKILL.md
  - id: openwiki-source-29176c627ccc31a24d3d6ab2
    resource: repo://tests/build-info/component.test.mjs
  - id: openwiki-source-3b8e596f6d78967bbf29a877
    resource: repo://tests/build-info/isolated.mjs
  - id: openwiki-source-492bc994297fdb3fa913fe0c
    resource: repo://tests/chat/external-editors.test.mjs
  - id: openwiki-source-0f4dd03aa48c30d7b96247f8
    resource: repo://tests/chat/mcp-approval.mjs
  - id: openwiki-source-6bda7be6a5c83fa82bbb1e39
    resource: repo://tests/chat/paste-markdown.test.mjs
  - id: openwiki-source-7cc1d248ce00f0c5ef61c76e
    resource: repo://tests/chat/prompt-context.test.mjs
  - id: openwiki-source-e023073a2ead0d2a5b8304d5
    resource: repo://tests/project-keys/migration_compat.py
  - id: openwiki-source-27c3f21856d770e96494e465
    resource: repo://tests/server/run-managed-dev-server.mjs
  - id: openwiki-source-a1c202ba8b2becaa93218bc0
    resource: repo://tests/server/run-workspace-tokens.mjs
  - id: openwiki-source-f7bef772fe7a00d12c00a997
    resource: repo://tests/server/specs/workspace-tokens.spec.ts
  - id: openwiki-source-40491b61cd2a69361d0984ad
    resource: repo://tests/settings-redesign/fixture/screen.tsx
  - id: openwiki-source-990930bbf26fab345536198a
    resource: repo://tests/settings-redesign/general-i18n.spec.ts
  - id: openwiki-source-3ab332950b513e74b8a08d6c
    resource: repo://tests/settings-redesign/model-selector-i18n.spec.ts
  - id: openwiki-source-c20751bcb54a268469b435af
    resource: repo://tests/settings-redesign/schema-i18n.spec.ts
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
generated: { by: "codex", at: "2026-11-09T20:29:18.834Z" }
verified:
  - by: openwiki/0.7.2
    at: 2026-11-09T20:29:18.834Z
---


# 開発・検証・運用文書の使い分け

変更箇所から [システムの境界](../architecture/system.md) を選び、その境界の検証を行う。README は開発の入口だが、コマンドの実際の対象は package.json と各 Cargo manifest で確認する。

<a id="ローカル開発の入口"></a>

## 開発環境と起動形態

ルート package.json は Node >=20、pnpm >=8 を要求し、packageManager を pnpm@10.13.1 に固定する。Rust toolchain は nightly-2025-12-04。[root manifest](../../package.json)・[Rust toolchain](../../rust-toolchain.toml)

| 作業 | 入口 | 重要な境界 |
| --- | --- | --- |
| ローカル Web と backend | pnpm run dev | frontend/backend/preview の port を解決し並行起動する |
| Backend の再起動開発 | pnpm run backend:dev | agent-process-host を build してから server を実行する |

[各 script](../../package.json)

Tauri shell、native frontend bridge、desktop packaging/updater scripts と `tauri:*` コマンドは撤去済みである。ブラウザーから Docker server または上記の source 開発環境を使う。既存 desktop data の削除・移行は自動実行しない。[移行契約](server-container.md#旧-npx-ランチャーの撤去と移行)。`server::startup` API は残るが、現行 desktop アプリの起動入口ではない。standalone と library startup の回復・watcher の差は [起動順序](../architecture/system.md#起動時は内側の所有者から回復する)で確認する。

通常 backend watch と instance runner は旧 cleanup 無効化変数を設定しない。自動期限 cleanup は撤去され、作業木の解放は [手動 archive](../concepts/workspace.md#archive削除期限-cleanup)に従う。Agent runtime の変更では server だけでなく独立 process host の binary も変更対象になる。[watch 定義](../../package.json)

旧 Cloud / relay の起動scriptと別Cargo workspaceは撤去された。local Board API と外部MCP / Runnerの検証は現行serverを対象にする。

## LVK 開発用 Cargo と共有キャッシュ

LVK 自身の POSIX source 開発は checkout 内から `node scripts/dev-cargo.mjs check --workspace` のように実行する。backend の check/lint/test/format、型生成と watch の pnpm script もこの入口を共有する。入口は `cargo locate-project --workspace` の実パスで自分の checkout を確認し、crate サブディレクトリからも利用できる。build/check/test/clippy/run/fmt/metadata/watch が対象で、config・manifest・target・toolchain の上書きは raw Cargo で別の方針として実行する。[呼出元](../../package.json)・[対象と scope 検査](../../scripts/dev-cargo.mjs#L30-L41)

### 用途と profile の選択

`--purpose` は subcommand の前に置く。既定は `preview`。`dev` と `stable` は fmt / metadata 以外に `--release` を追加し、呼出側の `--release` / `-r` / `--profile` は拒否する。通常の Cargo.toml は release の debug=1 を保持するが、この入口は dev / test / release の各 profile を用途に応じて引数で上書きする。[選択と設定](../../scripts/dev-cargo.mjs#L10-L20)・[profile 制御](../../scripts/dev-cargo.mjs#L40-L84)・[通常 Cargo](../../Cargo.toml)

| 用途 | build の入口と出力 | 最適化・debug・strip |
| --- | --- | --- |
| preview | `node scripts/dev-cargo.mjs build --locked --bin server --bin agent-process-host` → checkout の `target/debug/` | opt-level=0、line-tables-only、strip=none |
| 常用 dev | `node scripts/dev-cargo.mjs --purpose dev build --locked -p server -p local-deployment -p mcp --bin server --bin agent-process-host --bin vibe-kanban-mcp --features server/developer-runtime` → checkout の `target/release/` | opt-level=3、line-tables-only、strip=none |
| stable | 配布は root Dockerfile の `server` target。ローカルの `--purpose stable build` は同じ profile の確認入口 | opt-level=3、debug=0、strip=symbols |

dev/test profile は incremental=true、release は incremental=false、全用途で split-debuginfo=off。preview / dev は行情報を実行ファイル内に残し、コピーした bundle に debug sidecar を追加しないという判断が記録されている。stable は chef cook と server build の両 command に同じ release 環境値を限定し、image 全体の ENV や製品 runtime へ適用しない。[記録された理由](#lvk-開発用-cargo-と共有キャッシュ)・[Docker の適用箇所](../../Dockerfile#L73-L107)

instance / bundle 作成前に frontend を build する。常用 dev bundle は上表の developer-runtime 付き build と [成果物切替](#開発用成果物切替と復旧)を組み合わせる。`instance:build` と `staging:build` は frontend の後に既定 preview の入口で server / process-host を作り、source の `target/debug/` から候補へコピーする。staging は環境名であり常用 dev の用途名ではない。[instance の結線](../../deploy/developer/instance.mjs#L130-L183)・[bundle 作成手順](../../deploy/developer/switch/README.md#buildとbundle作成切替engineの外側)

### 保存先と runtime の境界

[Workspace の cache リンク](../concepts/workspace.md#共有ディレクトリの意味)は保存先を提供し、利用設定はこの入口が明示的に所有する。global / auto-loaded Cargo 設定や Project setup へこの方針を導入しないという判断が記録されている。[適用境界の記録](#lvk-開発用-cargo-と共有キャッシュ)

| 保存・設定 | 契約 |
| --- | --- |
| 最終 target | 当該 source checkout の `target/` |
| Cargo 中間物 | `.lvk/shared/cache` があれば canonical path 配下の単一 `lvk-dev-cargo-build`。Workspace ごとの hash を挟まない |
| 共有リンクなし | 中間物も checkout の `target/` に置き、警告する。instance の source snapshot も同じ規則 |
| compiler cache | 開発入口はsccacheの探索・起動・設定を行わずrustcを直接使う。明示wrapperがあれば設定を変更せず拒否する |

[保存先とwrapper拒否](../../scripts/dev-cargo.mjs)・[廃止の範囲](#lvk-開発用-cargo-と共有キャッシュ)

Cargo や `cargo run` の binary、そこから起動した子 Project へ、この入口が追加の compiler 環境変数を export することはない。ただし無関係な既存の継承環境を洗い流す仕組みでもない。watch は `cargo watch -w crates -s` から選択用途を付けて入口を二度呼び、まず server と agent-process-host を build し、成功後に server を run する。外部 cargo-watch が Cargo の global 引数を伝播することには依存しない。[watch の結線](../../scripts/dev-cargo.mjs#L19-L29)・[runtime と子 repository の fixture](../../scripts/dev-cargo.test.mjs#L87-L104)

### 競合・失敗・削除の扱い

`CARGO_INCREMENTAL` と `CARGO_BUILD_INCREMENTAL` は0を含め未設定が必要で、設定済み `build.incremental` も拒否する。dev/test/release の opt/debug/strip/split/incremental profile 競合値、target/build-dir 環境変数、別の rustc / workspace wrapper は明示エラーとなる。環境と Cargo build/target 設定の Rust flags も調べ、これらの codegen 方針を上書きする値を拒否する。設定検査自体に失敗しても停止する。競合を解除して opt in するか raw Cargo を使う。[拒否条件](../../scripts/dev-cargo.mjs#L37-L79)・[wrapper の検査](../../scripts/dev-cargo.mjs#L96-L111)

開発入口はsccacheを探索せず、stats取得も行わない。明示のcompiler wrapperはraw Cargoで意図的に使うか、利用者が競合設定を解除する。ホストtool・稼働daemon・共有cacheは削除も再設定もしない。コンパイル操作にはnightly Cargoのchecksum-freshnessを要求し、fmt/metadataは対象外である。[入口](../../scripts/dev-cargo.mjs)・[適用範囲](#lvk-開発用-cargo-と共有キャッシュ)。

中間物の競合は Cargo 自身の build lock が扱うため、最終 target が異なる Workspace 間でも待機し得る。独自 lock や deps symlink で回避しない。入口は `clean` を拒否する。raw clean は共有中間物も削除でき、他 checkout の最終 binary が残っていても cache の保持を意味しない。[拒否](../../scripts/dev-cargo.mjs#L30-L32)・[実 Cargo の lock / clean fixture](../../scripts/dev-cargo.test.mjs#L106-L119)

SQLx prepare は raw Cargo の専用手順を維持する。一方、instance build は上述の preview 入口へ接続済みである。配布 image は raw Cargo の command-local stable 設定を使うため、この開発入口の存在だけで全 build の設定が変わるわけではない。[SQLx](../../scripts/prepare-db.js#L15-L41)・[instance](../../deploy/developer/instance.mjs#L153-L183)・[配布 build](../../Dockerfile#L73-L107)

### 検証で確かめる範囲

`node --test scripts/dev-cargo.test.mjs` はpinned Cargo/rustcと依存不要の一時repositoryで、三用途のprofile、ELF行情報、dev/test incremental=trueとrelease=false、共有中間物と分離target、fallback、wrapper競合、run/watch環境、Cargo lockとcleanを検査する。PATH上のpoisoned sccacheが呼ばれないことも確認し、実cache daemonは使わない。[試験](../../scripts/dev-cargo.test.mjs)。

[LOCAL-197の局所4件成功と実sccache smoke省略](#lvk-開発用-cargo-と共有キャッシュ)は過去の記録である。現在の開発入口の方針と混ぜず、fixtureの成功を実LVK性能測定や配備後のhealth受入に読み替えない。このWiki保守ではsourceと試験定義を照合し、これらのfixtureを追加実行していない。[現在の検証範囲](#lvk-開発用-cargo-と共有キャッシュ)。

## 検証の対象を取り違えない

repository の orchestration Skill は、検証済みSHA、候補差分、競合解消、新たな未検証範囲から必要な検証を選ぶ。担当Codexが候補作成・修復・検証と理由付き省略を記録し、hostが公開条件と証拠を検査する。設定されたcommand一覧は参考入力であり、hostの固定必須試験セットではない。実行と省略は判断主体・理由・残余リスク・対象SHAを分け、未実施をpassと呼ばない。[検証責務と証拠評価](../../skills/lvk-orchestration/references/auto-merge.md#risk-based-test-planning-and-auto-merge-validation)。

既存証拠が十分なら重複検証を省略できるが、公開前に必要な試験や実際の実行拒否は免除されない。buildによる成果物作成と試験の再実行も区別する。これはAIへの運用指針であり、製品の公開guardを変更する実装ではない。製品側の契約は [自動マージ](../concepts/formal-integration.md)を参照する。

### 同梱 Skill の責務と調査範囲

`skills/lvk-orchestration/SKILL.md` は対話・設計・調査・実行調整の指針である。LOCAL-224 の記録は dev plugin v1.0.33 への全文同期を示し、以前の「名称差分のみ」「指定5段落のみ」は当時の限定範囲として残す。`plugins/lvk/skills/lvk-work/SKILL.md` は接続済み LVK の作業操作を担う別 Skill で、orchestration の配布コピーではない。移行した過去の「同期範囲の記録」・[作業操作の責務](../../plugins/lvk/skills/lvk-work/SKILL.md#L1-L29)。この区別を保ち、一方の同期を理由に他方を全文置換しない。

orchestration の記録では、明示された調査には必要な独立 Workspace と、その中の再現コード・script・fixture・test が含まれる。一方、製品変更の採用、merge、公開、配備、共有環境・live data の変更はその調査許可に含まれない。最新の明示指示が過去の一時制約を更新したかを引継ぎ時に区別するが、実際の実行拒否は回避しない。独立 worktree は DB・資格情報・background process・外部 service の隔離を保証しない。[意図と許可の区別](../../skills/lvk-orchestration/references/execution.md)・[副作用の境界](../../skills/lvk-orchestration/references/investigation.md#investigation-boundaries-and-artifacts)。これらは AI の運用指針であり、新しい API 権限や実行 engine ではない。

| 検証 | 実際の対象・限界 |
| --- | --- |
| pnpm run check | local-web / web-core / ui の型、legacy path guard、現行 Rust workspace の cargo check |
| pnpm run lint | local-web / ui の lint、現行 Rust workspace の Clippy（qa-mode、all-targets）、未使用 i18n key 検査 |
| pnpm run backend:test | 現行 Rust workspace の tests |
| pnpm run workflow:e2e | 専用 Vite fixture に対する Chromium UI tests |
| web-core の test:wiki | Wiki feature の Vitest。全 frontend tests の包括実行ではない |
| pnpm run server:check | server 配布 kit の Node tests。native nginx test の実行には nginx が必要 |

[コマンド定義](../../package.json)・[Wiki test script](../../packages/web-core/package.json#L100-L104)

Tauri crate 自体が workspace からなくなったため、backend の check/lint/test は `--workspace` を使い、旧 `--exclude vibe-kanban-tauri` はない。[Cargo構成](../../Cargo.toml)、[script](../../package.json)。

旧Card ContextのWorkflow specは撤去した。専用Viteを使うWorkflow Playwrightは1workerの既存fixtureとして残り、保存済み本文の閲覧・編集・専用UI不在は `node --test tests/chat/prompt-context.test.mjs` のChromium隔離componentで確認する。[Workflow設定](../../tests/workflow/playwright.config.ts)・[本文試験](../../tests/chat/prompt-context.test.mjs)

実行制御の変更は [監査・投影の回復 tests](../architecture/agent-runtime.md)、Graph の変更は [Workflow の validation/planner tests](../concepts/workflow-attempt.md)、memory checkpoint は [Repository memory の冪等性 tests](../concepts/repository-memory.md) へ進む。画面上の成功と永続状態の成功を分けて検証する。

### ブラウザー貼り付けの回帰

`node --test tests/chat/paste-markdown.test.mjs` は esbuild と Playwright/Chromium を使い、共有 plugin を直接描画する。Markdown、HTML、HTML 付き raw paste の結果を検査し、browser を finally で閉じる。入力の優先順位・selection 復元・fallback は [貼り付け契約](../architecture/web-and-sync.md#markdownhtmlraw-の貼り付け)を正本とする。[試験定義](../../tests/chat/paste-markdown.test.mjs)

この fixture は OS clipboard を操作せず ClipboardEvent を生成するため、実 OS の clipboard、Firefox/WebKit、live Web Push 配送の保証にはならない。今回の Sync は凍結 source `749367be4937b0ed8338a72995aa84109b3ab0aa` を manifest、browser entry、共有 plugin、notification service、server startup と独立した試験定義から静的照合した。供給された開発時検証報告、より新しい Workspace Memory の compile 報告、自動マージ の明示的な検証省略は別の証拠であり、この Wiki 保守では製品テスト・Rust build・live 配送を再実行していない。変更に無関係な過去の内容全体を再検証したものではない。

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

移行した過去の「開発時記録」の成功報告は今回の Sync の再実行結果ではない。凍結された統合 source `066b2b4bd93d06d046865d66ae16bf1537e8b184` を adapter、schema/保存、API/MCP、UI、独立した試験定義から静的に照合した。入力にある自動マージ は明示的な検証省略であり、今回も製品テストや有料モデル run は実行していない。実アカウント精度・実 Auth0 経由・全 provider・子 thread 独立計測・counter reset 後の差分の保証はない。無関係な過去の説明全体を再検証したものでもない。

## モバイル向け built preview

`pnpm run local-web:preview` はlocal frontendをbuildしてVite previewをforegroundで実行する。既存backendが必要で、既定frontend4030・backend4021・HOST 0.0.0.0。HMRはなくsource変更後は再起動してbuildし直す。出力を `dist-preview` に分け、Cargoが埋め込むassetsと混ぜない。Node heapの既定は4GiBで、同一portや不正portは起動前に拒否する。[preview script](../../scripts/start-web-preview.sh)。

通常開発HMR、built preview、配布imageは別の検証経路である。`mobile-preview:test` と `chat:test` は対応するNode test群、Workflow PlaywrightはUI fixtureであり、実機・provider認証・公開の成功を代替しない。[定義](../../package.json)。

## 独立 instance と固定 staging 候補

LVK 自体の変更は card ごとに別 instance 名・port を使って検証できる。`LVK_INSTANCE_DIR` は Cargo の debug/release と独立した絶対ディレクトリで、data（DB・設定・添付）、runtime（実行状態）、ports（発見情報）を分ける。既定 stable data と重なる path や symlink alias は拒否する。未指定なら従来の保存先を使う。HOME は agent 認証の保存先として維持し、instance の代わりに変更しない。同じ OS user に対する security sandbox ではない。[path 検証](../../crates/utils/src/instance.rs)、[data](../../crates/utils/src/assets.rs)、[運用契約](../../deploy/developer/STAGING.md)

`instance:build NAME [REF]` は frontend と server / process-host を一組で build し、完了時に build identity と source revision を記録する。通常 card の未 commit 作業は dirty として記録できる。`staging:build` は main 等の固定 commit を detached snapshot にし、失敗 build を runnable candidate にしない。debug assets の参照元なので、その snapshot を実行中に削除しない。[build と記録](../../deploy/developer/instance.mjs#L115-L193)

`instance:run NAME BUILD_UUID PORT` は foreground supervisor と instance ごとの run lock を持ち、build lock は別なので次候補の build と現在の実行を両立できる。staging は loopback 4020、任意の nginx ingress は HTTPS 8443 を使う。稼働中の agent / Workflow が終わってから旧候補を停止し、新候補へ切り替える。schema が進んだ DB に旧 binary だけを戻さず、対応する idle data snapshot が必要となる。共有 staging は [共有資源調停](../concepts/shared-resource-coordination.md)の対象であり、host updater の agent 活動判定には instance/build process を含めない。image 置換で staging 等の非 agent process は停止し、更新後の staging は明示的に起動し直す。[run](../../deploy/developer/instance.mjs#L193-L217)、[更新・回復手順](../../deploy/developer/STAGING.md)

stagingの認証stateはstableと独立しており、同じ`LVK_INSTANCE_DIR`でsetupキーを発行する。stagingの公開URLは8443を含む。cookieはportで隔離されないため、同じhostname/IPのstableとstagingには別ブラウザープロファイルを使う。[配置とcookieの運用契約](../../deploy/developer/STAGING.md#staging)、[instance環境](../../deploy/developer/instance.mjs#L44-L64)。方式変更と失効は [共通認証](../concepts/server-access.md)に従う。

## 開発用成果物切替と復旧

`deploy/developer/switch` は、稼働中Linux開発instanceを外部で試験済みのbundleへ切り替えるoperator配置用kitである。安定版Docker imageの更新や停止済みserverの復旧とは別の入口で、repositoryへの統合はlive導入を意味しない。engineはGit操作・build・migration互換性審査を行わず、migrationは新serverの通常起動に任せる。MCPの認可・受付・再接続は [専用自己再起動](../concepts/runtime-access.md#開発専用-self_restart)、具体的な配置と移行操作は [kitの手順](../../deploy/developer/switch/README.md)を参照する。

### 起動設定とbundleの境界

`INSTANCE/launch.json` が唯一の永続起動設定で、portとenvironmentを持ちowner-onlyで保管する。必要なPATH・HOME・認証や暗号鍵の参照はoperatorが明示して保持し、環境や秘密をbundleへ丸ごと収集しない。`current.json` はinstanceの `releases/ID` とmanifestを選ぶ。固定 `switch/launcher.py` はinstanceをcwd、HOSTをloopback、agent hostを選択releaseの絶対path、PATHを同release優先にしてserverをexecする。実際に読んだ設定bytesのhashを渡すため、後の切替で設定差異を検出できる。[設定検証](../../deploy/developer/switch/start_binary.py#L27-L74)・[launcher](../../deploy/developer/switch/launcher.py#L10-L34)

bundleは `server`、`agent-process-host`、`vibe-kanban-mcp` と `bundle.json` のみ。manifestはformat 1、実行ファイルとassetのSHA256を持ち、symlink・欠落・余分なpayload・hashや権限不一致を拒否する。frontendをbuildし、3 binaryを `--features server/developer-runtime` 付きでbuildした後、`bundle.py`で既存成果物を梱包する。featureはdebugでもfrontend・sounds・scriptsを埋め込む。[作成手順](../../deploy/developer/switch/README.md#buildとbundle作成切替engineの外側)・[bundle検証](../../deploy/developer/switch/start_binary.py#L77-L137)・[埋込みprobe](../../crates/server/src/runtime_payload.rs#L9-L34)

実切替は稼働中にcandidateをreleaseへコピーし、コピー後の全binaryの `runtime-payload` と実asset bytesを検査してから停止する。manifestのembedded宣言だけは信用しない。それ以降はコピー先だけを参照し、元source・frontend・targetへのfallbackはない。明示 `VIBE_KANBAN_AGENT_PROCESS_HOST` は絶対の既存fileでなければ失敗し、別hostへfallbackしない。ただしOS libraries、Python、Git、provider CLIと認証は運用環境の依存として残る。[コピーと検査](../../deploy/developer/switch/start_binary.py#L101-L137)・[保存後の境界](../../deploy/developer/switch/start_binary.py#L217-L233)・[host解決](../../crates/local-deployment/src/agent_run_port.rs#L3638-L3650)・[外部依存](../../deploy/developer/switch/README.md#配置と正本)

### 受付から停止・起動まで

同期CLIと非同期MCPは同じengineを使う。CLIは配置されたswitchディレクトリの親をinstanceとし、`--bundle`、`--request-id`、`--no-new-work`で切替、同じIDの `--status` で結果を読む。`--dry-run` は停止・コピー・candidate実行をせず現在のhash・idle・容量見積りを確認するだけである。[CLI](../../deploy/developer/switch/start_binary.py#L552-L610)・[事前検査](../../deploy/developer/switch/start_binary.py#L182-L214)

外側で新規作業・schedule・論理queueを調整する。engineは過去Runの終端履歴ではなく、現在のserver子孫、instance process、data使用者を検査する。resource snapshotのholders/queueがあれば拒否するが、未使用の登録だけでは拒否しない。対象instance/DB自体が登録資源なら、この独立engineではなく [共有資源bundle](../concepts/shared-resource-coordination.md)の契約が必要である。no_new_workは外部調整の宣言で、scheduler停止を代行する強制flagではない。[物理guard](../../deploy/developer/switch/start_support.py#L139-L249)・[適用前提](../../deploy/developer/switch/README.md#保持と失敗復旧)

同じUUID・同じ入力は既存結果を返し、入力違いのID再利用・別要求の並行切替を拒否する。受付を原子的に保存し、独立session/I/Oのworkerへlock FDを渡す。workerは受付processの終了をpidfdで待ち、固定sleepや呼出元serverの生存に依存しない。lock FDは新serverへ渡さない。workerが失われlockが解放された未完了結果はinterruptedとなり自動再実行しない。[受付とworker](../../deploy/developer/switch/start_binary.py#L458-L600)・[結果回復](../../deploy/developer/switch/start_binary.py#L275-L309)

停止前にsnapshot・listener・設定hash・新旧release・idleを再照合し、pidfdで対象identityを固定してTERMを送る。60秒で終了未確認なら自動KILLせず停止する。停止確認後にdataとlaunch/current・固定kitをbackupし、SQLite backup APIでWAL確定分を単独DBへ取り込む。integrity/FK・manifest・fsync確認後の `backup-complete.json` が完成証拠となる。runtime/worktree・target・node_modules・共有cache・HOME全体はbackupしない。[停止](../../deploy/developer/switch/start_support.py#L222-L249)・[SQLite backup](../../deploy/developer/switch/start_support.py#L260-L298)・[切替順序](../../deploy/developer/switch/start_binary.py#L361-L437)

currentを原子的に切り替えて固定launcherを起動し、health、listener、exe/cwd、全release hash、配信index/JS/CSS hash、DB整合性を照合する。画像等は埋込みprobeの対象だが全URLのHTTP到達性を保証しない。成功時だけcurrent+previousの2世代と最新backup1セットへ整理し、小さなreceipt/logは保持する。途中はcandidateを含む3世代、backup2セット分の容量が必要になる。[health](../../deploy/developer/switch/start_binary.py#L312-L344)・[成功と保持](../../deploy/developer/switch/start_binary.py#L252-L265)・[完了](../../deploy/developer/switch/start_binary.py#L424-L447)

### 管理PTYを一度だけ終了する

この見出しは既存リンクのため保持する。現行の開発専用切替はPTY限定ではなく、対象serverの確認済み子孫を終了する。実切替では権限・snapshot・新旧成果物・resource検査後の停止処理で子孫を止める。dry-runはsignalを送らず、子があれば拒否する。[実行順序](../../deploy/developer/switch/start_binary.py#L418-L450)・[idle](../../deploy/developer/switch/start_support.py#L163-L175)

対象serverと子孫をpidfd・開始時刻・UID・親子edgeで固定する。環境を消した子や別session/groupの子も捕捉するが、明示的に別instanceを示す子や異UIDはsignal前に拒否する。名前・cwd・process group全体をkill対象選択に使わず、環境一致だけで非子孫を追加しない。[所有境界](../../deploy/developer/switch/process_drain.py#L30-L96)

signal前の捕捉情報をoperationの `child-stop.json` に保存する。serverと子孫を一時SIGSTOPしてfork元を凍結し、確認済み子孫へ個別SIGKILLを送り、共通10秒期限でpidfdのexitを待つ。捕捉後に親が変わっても同一processは追跡する。server本体は既存SIGTERM/60秒上限を維持しKILLしない。失敗時はこの処理で停止した生存processだけCONTし、backup/cutoverへ進まない。zombieは既存の終了済み扱いを維持し、全子孫wait回収は保証しない。[停止処理](../../deploy/developer/switch/process_drain.py#L49-L147)・[server終了待機](../../deploy/developer/switch/start_support.py#L236-L258)

`--no-new-work` はTerminal再接続停止を含む外部調整の宣言である。server再開後のgraceful shutdown中に新しい子を観測した場合も拒否する。/proc観測前に切り離された未知daemonの完全追跡は保証しない。対象外instance/data利用者の検査を維持し、子孫exit・listener消失・残存data FD不在を確認してから整合backupへ進む。[限界と前提](../../deploy/developer/switch/README.md#cliとmcp)・[停止後guard](../../deploy/developer/switch/start_support.py#L194-L233)

初回導入でも新binaryの先行起動は不要だが、別途承認されたoperator作業で同revisionの `launcher.py`、`start_binary.py`、`start_support.py`、`process_drain.py`、`start.sh` を一組で配置する。実行中workerのkitは置換しない。旧serverの認可再確認と新stdio委任の違いはREADMEの初回導入手順に従う。稼働binary SHAと固定kit script SHAは別々に記録し、過去失敗fenceとログを保全する。Rustの新protocolや安定版image更新を導入する変更ではない。[初回導入とstdio委任](../../deploy/developer/switch/README.md#初回導入とstdio委任)

### 失敗と旧契約からの移行

結果の正本は `INSTANCE/switch-operations/UUID/result.json`。failed/interrupted/未完了のoperationは後続切替を拒否する。新IDによる自動retry、自動DB rollback、旧binary復帰、失敗data削除はしない。新serverが診断用に残る場合もあるため、stage、process/listener、migration後DB、backup marker、保護ログを調べてoperatorが回復方針を決める。復旧後の失敗operationは保護された回復記録へ移してfenceを解除するのであって、成功へ書き換えない。[失敗保存と受付fence](../../deploy/developer/switch/start_binary.py#L448-L509)・[回復手順](../../deploy/developer/switch/README.md#保持と失敗復旧)

旧source/frontend/retain-worktree等のCLI・MCP入力、switch.json、run.pyのAST書換え、旧engine/config環境変数は新契約で使わない。初回legacy移行は外部terminalのoperator作業である。旧起動設定・backup・依存worktreeを保全し、承認済み停止と整合backup、固定kit/launch/currentへの移行、起動後の動作確認を行う。sourceを隠した隔離試験で独立性を確認してから旧保持例外を解消する。統合済みコードがあるだけで稼働版を移行済みとせず、[Workspace archiveの実参照検査](../concepts/workspace.md#archive削除期限-cleanup)も省略しない。[移行手順と廃止入力](../../deploy/developer/switch/README.md#旧契約からの移行)

### 検証の限界

Python `test_switch.py` はコピー後の元bundle不可視、manifest虚偽、停止前snapshot差異、backup/health失敗、worker中断、冪等性、保持世代を検査する。Rust `environment_switch_tests.rs` はproduction MCP routerとmock認証で呼出元停止・再接続後の同receipt取得を扱う。`test_real_server.py` は実binaryのruntime子にLandlockを適用し、source不可視でmigrationあり/なし、asset/API、host bootstrap、Workspace cwdからの同梱stdio MCP、async worker生存を検査する。engineはprocfs identity観測のため同じLandlock隔離に入れない。[Python回帰](../../deploy/developer/switch/test_switch.py#L210-L297)・[実runtime試験](../../deploy/developer/switch/test_real_server.py#L188-L250)

今回のSyncは凍結source `93913677602eaa8f975911641dcd2e81acb91f66` の実装・呼出元・設定・試験定義の静的照合で、製品テストやlive切替は実施していない。開発時の成功記録と、自動マージの明示的検証省略は別の証拠である。実Auth0/ChatGPTの一体E2E、実provider/model、browser操作、live移行は未確認。既存 `/mcp/*` 画像URLの一部404も残る。無関係な過去のWiki全体を再検証したものではない。移行した過去の「記録された試験結果・残る範囲」

LOCAL-243の統合コード `01d5e55ec0ce5bd25ea9270b24eb85c1a1cfc4ba` では、switch全treeがsource `ad674f76af0f18ae1961618f4d5964cf7e86c8dd` と完全一致し、固定baseとの差分がswitch配下10pathに限定されることと空白検査を実行確認した。競合・修復はない。公開memoryの既存検証はprocess_drain 10件、switch 48件、busy再確認5件成功であり、このWiki更新中の再実行結果ではない。重複suiteとRust/frontend rebuildは省略し、実binary前提stdio 7件も既存skipのままである。

`test_process_drain.py` は子孫・TERM無視・PID/UID/親子変化・別instance保全・捕捉後reparent・停止失敗を、`test_switch.py` は環境消去/別group・data reader停止・dry-run・shutdown新子拒否・resource/backup/cutover回帰を扱う。実bundle、browser再接続、実認証/モデル、live切替は今回未実行。source統合と固定kit配置・稼働binary更新を区別する。[停止fixture](../../deploy/developer/switch/test_process_drain.py)・[切替fixture](../../deploy/developer/switch/test_switch.py)

### ビルド情報の検証境界

instance runner は `build.json` と同じ revision / dirty を build 環境へ注入し、種別を `development` に固定する。staging という環境名や Cargo profile から release と推定しない。直接 Cargo build の再評価、明示注入の優先順位、API の optional 契約は [ビルド時ソース識別](server-container.md#ビルド時ソース識別と-api-の契約)にまとめる。[注入と記録](../../deploy/developer/instance.mjs#L131-L195)

検証は三つの境界を分ける。`crates/server/build_metadata.rs` の unit tests は明示 release、未知・不正入力、development の tag 除外、実 Git の clean → 未追跡 dirty → clean と空 injection の優先を扱う。`node --test tests/build-info/component.test.mjs` は SSR で native button、dialog trigger、日英 label、40/64桁 SHA と欠落時の非表示を検査する。配布引数の接続は [release test](../../deploy/server/release.test.mjs#L37-L58)が静的に検査するが、image 実行の代用にはならない。[metadata tests](../../crates/server/build_metadata.rs#L93-L191)・[component tests](../../tests/build-info/component.test.mjs#L29-L74)

`node tests/build-info/isolated.mjs` は root の `target/debug/server` と `agent-process-host`、build 済み frontend、Chromium を前提に、一時 instance / SQLite と loopback を使う。別 Git checkout から起動し、実行時に偽の `LVK_BUILD_*` を渡しても実 binary の情報が返ることを確認する。実 metadata は試験開始時の checkout HEAD / dirty と一致する必要があるので、build 後に source 状態を変えた古い binary で実行しない。[準備と起動](../../tests/build-info/isolated.mjs#L20-L84)・[API の照合](../../tests/build-info/isolated.mjs#L119-L157)

ブラウザー試験は Tab / Enter / Space、Escape / Close、focus 復帰、390px / 320px の touch 操作と画面内収容を検査する。release・unknown・legacy 欠落のケースは `/api/info` の metadata だけを置き換える fixture であり、公開済み release image の試験ではない。mobile の実値ケースは dirty 表示を固定で期待するため、clean checkout では前提が一致しない点にも注意する。touch は Pixel 7 の Chromium emulation であり実 Android や screen reader 音声の保証ではない。[metadata 置換と操作](../../tests/build-info/isolated.mjs#L183-L309)・[ケースの区別](../../tests/build-info/isolated.mjs#L332-L417)。表示の責任分割は [共有 shell のビルド情報](../architecture/web-and-sync.md#稼働バイナリのビルド情報を表示する)を参照する。

終了処理は browser を閉じ、server 終了を待ち、PID・port・一時 instance の除去を記録する。証跡先は既定 `test-results/build-info`、`LVK_BUILD_INFO_EVIDENCE` で変更できる。[cleanup](../../tests/build-info/isolated.mjs#L434-L473)。今回の Sync は凍結 source `393670802114ed09b421a52d6dd186a6da5dc2f5` を build resolver、instance、API、context、UI と独立した試験定義から静的に照合した。凍結入力の開発時成功報告と自動マージ の明示的な検証省略は別であり、この文書保守では製品テスト・Docker build・実機試験を再実行していない。無関係な過去の説明全体の再検証も意味しない。

## Docker 配布と同梱 binary

LVK の配布先は GHCR の Docker image のみ。root Dockerfile の `server` target を配布対象とする。[配布契約](server-container.md)。
`server` と `agent-process-host`、
`vibe-kanban-mcp` を同じ image に同梱する。既定 MCP 設定は同梱バイナリを直接起動し、
npm から別バージョンを取得しない。

`npx-cli/` と `local-build.sh` は撤去済みで、root package の npm 用 bin/files と
`build:npx` / `build:npx-cli` / `check:npx-cli` / `prepack` / `test:npm` も存在しない。
旧 version validator は残るが、現行の公開手順ではない。[現行 scripts](../../package.json)。
旧 server・MCP・desktop 起動方法からの移行は
[旧 npx ランチャーの撤去と移行](server-container.md#旧-npx-ランチャーの撤去と移行)を参照する。
配布 image と Compose の更新・旧名 volume の引継ぎは
[サーバーコンテナー](server-container.md)を参照する。

## LVK の名称

製品表記と現行の設定接頭辞は LVK を使う。名称変更の未記録の動機や、既存利用者のデータ有無を実装から推定しない。
オリジナル Vibe Kanban の `VK_*`、native binary、保存先、外部サービスの識別子は
変更対象ではない。上流への帰属表記と実際の過去の記録は保持する。

| 境界 | 現在の契約 | 根拠 |
| --- | --- | --- |
| 実行設定 | `LVK_*` を読み書きし、process host へそのまま渡す。旧接頭辞への変換はしない。 | [ExecutionEnv](../../crates/executors/src/env.rs)、[HostExecutionEnv](../../crates/local-deployment/src/process_host.rs) |
| 保存済み本文 | 専用UI・既定文生成・hidden分離を廃止し、独自文を通常descriptionで保持する。送信時の既知5hash限定除去は保存値を書き換えない。 | [本文保護](../concepts/card-context-and-llm-wiki.md) |
| 共有ファイル | `.lvk/shared/` から repository storage を参照する。追跡済み mount・別 repository 向きのリンク・ユーザーファイルを上書きしない。watcher、Git diff、Docker context から除外する。 | [共有リンク](../../crates/workspace-manager/src/shared_resources.rs)、[除外](../../crates/utils/src/path.rs) |
| Git publication | `LVK-Memory-Source` / `LVK-Memory-Integration` / `LVK-Wiki-Publication` trailer を記録し、復旧時も同じ marker を検索して重複 commit を防ぐ。 | [Memory 復旧](../../crates/services/src/services/repository_memory.rs)、[Wiki 公開](../../crates/services/src/services/openwiki.rs) |
| publication lock | repository 共通の `lvk-publication.lock` 一つを保持する。この lock は外部の任意 Git 操作までは制御しない。 | [GitPublicationGuard](../../crates/git/src/publication.rs) |
| サーバー | Compose project/service は `lvk-server` / `lvk`。接続先は IP・DNS 共通で `LVK_HOST`、Certbot lineage は `lvk`。 | [Compose](../../deploy/server/compose.yaml)、[ACME](../../deploy/server/acme.py) |
| MCP・配布 | 既定 MCP は image に同梱した `vibe-kanban-mcp` を直接実行する。LVK の npm package を取得する preset は提供しない。 | [既定 MCP](../../crates/executors/default_mcp.json)、[接続手順](../integrations/agent-providers.md#外部-mcp-client--lvk) |

## サーバー配布 kit の検証

`pnpm run server:check` は `node --test deploy/server/*.test.mjs` を実行する。設定・証明書・process supervision 等の検査と、nginx を実際に起動する TLS/Basic 認証・WebSocket・SSE・preview 境界の検査を含む。nginx がなければ後者は skip するため、全 subtest が実行されたかを結果で確認する。`LVK_TEST_NGINX` で実行ファイルを指定でき、`LVK_REQUIRE_NGINX_TEST=1` は欠落を失敗にする。[script](../../package.json)、[native test の前提](../../deploy/server/nginx.test.mjs)

この確認だけで Docker image の build・実配置・live agent 認証が成功したことにはならない。配布済み binary と作業 checkout の分離、永続 volume、停止・更新・復旧の契約は [サーバーコンテナー](server-container.md)に置く。[記録された検証限界](server-container.md)

### 独立アプリの CSV ルートを変更するとき

[独立アプリ経路の契約](server-container.md#独立アプリの-csv-ルート)は nginx supervisor の JavaScript と ACME companion の Python に同じ入力規則を要求する。共有 `preview-routes.cases.json` を両側で読み、受理・拒否だけでなく host/port 配列の期待値を比較する。JavaScript 側は101経路の受理、同一 upstream の別名、Compose 両サービスへの同じ環境変数の受渡しを検査し、Python 側は25識別子境界と名前の追加・置換・削除、IP と DNS の混在注文を検査する。[JS の回帰](../../deploy/server/server.test.mjs#L457-L494)・[Python の回帰](../../deploy/server/acme.test.py#L32-L57)

native nginx fixture は自己署名証明書と loopback の模擬 upstream を使い、LVK の Basic 保護と独立経路への匿名到達の併存、Authorization/Cookie/Host/forwarded 情報、独自認証ヘッダー、Set-Cookie、WebSocket upgrade/subprotocol、各 route の証明書名不足を検査する。253文字の DNS 名も含む。parser が受理するだけでは nginx の設定読込成功を保証しないため、長い名前の回帰はこの層で扱う。[fixture と長い名前](../../deploy/server/nginx.test.mjs#L23-L66)・[到達と認証の検査](../../deploy/server/nginx.test.mjs#L314-L377)

移行した過去の「開発時の検証記録」には native nginx を含む39件成功・Compose CLI 1件省略と、期待値補強後の局所18件成功が記録されている。実 ACME/DNS/VPS/アプリ・配布 image は未確認であり、凍結された自動マージ の入力も検証の明示省略を記録する。今回の Wiki 保守は source と試験定義の静的照合で、これらの成功を再実行したものではない。

## 版番号の検証と選別移植の追跡

旧 npm 用の version parser は過去の `easy` 入力も互換用に受け付ける。
そのテストは現在の Docker image の build・公開成功を証明しない。
現在の配布検証は [release test](../../deploy/server/release.test.mjs) とサーバー配布文書を参照する。

上流変更の採否・LVK 向け適応・故障注入・実 Codex を使った MCP 受入は 移行した過去の「選別移植の実装記録」にある。各試験の source revision と実行対象を識別し、過去の成功を別 revision の成功と取り違えない。特に server と `agent-process-host` を同じソースから更新し、旧 Host の互換接続と新 Host の journal 回復を分けて試す。[Host の回復契約](../architecture/agent-runtime.md#接続断と確認済み終了を分ける)

Workflow fixture には draft / Undo、設定安全性、route loading、Runtime input、diff tree の harness がある。これは製品 component のブラウザー試験であり、認証済み provider・publication を含む実機試験とは別である。[fixture の入口](../../tests/workflow/fixture/src/main.tsx#L1-L29)

## 型・schema・SQLx を変更する場合

local API の生成元は Rust の generate_types binary であり、shared/types.ts と shared/schemas を生成・比較する。同じgeneratorがlocal Board互換用 `shared/remote-types.ts` も生成する。名称は旧Remoteサービスの存在を意味しない。API 変更は生成先だけを編集せず、Rust 側を更新して各 generator と --check を使う。[local generator](../../crates/server/src/bin/generate_types.rs)

prepare-db は一時 SQLite を作り、migration を適用してから cargo sqlx prepare を実行し、finally で一時 DB を削除する。--check でも一時 DB の作成と migration は行うため、単なる読み取り専用の型検査ではない。[SQLite 準備](../../scripts/prepare-db.js)。

### Project key migration の互換運用

version `20261025120000` の元 SQL は、全既存 Project に `LOCAL` を付けてから unique index を作るため、複数 Project の旧 DB では失敗する。後続 migration だけでは先行失敗を越えられない。元 SQL を保持し、この一版だけ二つの固定 SQL と各 LF/CRLF の既知 checksum を選ぶ `MigrationSource` を使う。[元 SQL](../../crates/db/migrations/20261025120000_project_keys.sql)・[対応方針と理由](../../crates/db/migration_compat/README.md#project-key-migration-compatibility)。Project と過去 Issue の識別子の関係は [仕事の識別と分類](../concepts/project-and-issue.md#仕事の識別と分類)を参照する。

| 対象版の保存履歴 | 選択と失敗の契約 |
| --- | --- |
| 未適用 | checkout の互換 SQL を実行する |
| 元 SQL / 互換 SQL の既知 LF・CRLF checksum | 保存 checksum に一致する実 SQL bytes を選び、通常の SQLx 検証を行う |
| 未知 checksum、または改変された元・互換 artifact | エラーにし、履歴を書き換えない |

adapter は SQLx の公開 `MigrationSource`・`Migrator::new`・`Migration::new` を使う。まず両 artifact が既知であることを検証し、その後で固定 LF/CRLF 候補を選ぶため、任意 SQL を改行正規化して受け入れる仕組みではない。他版の検証を緩和する一般的な履歴変換でもない。[選択と構築](../../crates/db/src/migration_compat.rs#L40-L118)。互換層は適用済み receipt を更新せず、実行時は SQLx が実 SQL の checksum を記録する。[履歴保持の設計](../../crates/db/migration_compat/README.md#project-key-migration-compatibility)。

互換 SQL は Project UUID 順で `LOCAL`、`LOCAL2`〜`LOCAL99999` を割り当てる。既存 Issue 行は更新せず、列 default は `LOCAL` のまま。100000件目は10文字 CHECK を超えて migration 全体が rollback し、切詰めや再利用はしない。[割当と制約](../../crates/db/migration_compat/project_keys_multi.sql#L1-L21)。

起動と明示 tool の共通入口は `db::run_migrations`。明示実行では認可済みの既存 DB を `DATABASE_URL` に指定し、`SQLX_OFFLINE=true cargo run -p db --bin migrate_compat` を使う。tool は DB の既定接続先を持たず、終了時に pool を閉じる。prepare-db も同 binary を `SQLX_OFFLINE=true` で build/実行してから、一時 DB に対する SQLx prepare を行う。[tool](../../crates/db/src/bin/migrate_compat.rs#L1-L10)・[prepare の順序](../../scripts/prepare-db.js#L15-L50)。

互換版適用後に raw `cargo sqlx migrate run` を使うと、directory 内の元 SQL しか知らず checksum 不一致になる。旧 server binary も互換 checksum を扱えず、この変更は downgrade を提供しない。`_sqlx_migrations` の手修正で回避しない。[対応する入口と制限](../../crates/db/migration_compat/README.md)。

対象版の未知 checksum は Windows release の既存 fallback より前に拒否する。他版については debug / 非 Windows で不一致を失敗させ、Windows release では保存 checksum を更新して一度ずつ再試行する従来分岐が残る。comment は platform 改行差等を理由に挙げるが、任意の schema 不一致を修復する保証ではない。[版別 guard と fallback](../../crates/db/src/lib.rs#L18-L80)。実行成功後の schema guard は [起動境界](../architecture/system.md#永続化は一つではない)を参照する。

変更時は `cargo test -p db --lib migration_compat::tests` の実 SQLx fixture を確認する。fresh、旧0/1件 receipt、旧複数件の失敗から再開、互換 CRLF receipt の再起動、checkout 改行と履歴の組合せ、未知 checksum・改変 artifact の拒否を検査する。[Rust 試験](../../crates/db/src/migration_compat.rs#L135-L408)。`python3 -B tests/project-keys/migration_compat.py` は Issue 値保持、99999件境界、100000件の反復 rollback 等の軽量 SQL/checksum model であり、Rust selector や全履歴を実行しない。raw `sqlx::migrate!` を使う空 DB 試験も互換 selector の証拠とは別である。[軽量試験](../../tests/project-keys/migration_compat.py#L1-L153)・[試験の区別](../../crates/db/migration_compat/README.md)。

今回の Sync は凍結 source `cc39e6f25eb0f2ae1fc15df8712befb16fed4509` を起動 caller、共通入口、selector、SQL、allocator、tool、prepare と独立した試験定義から静的照合した。凍結入力には局所 Rust / Clippy / prepare と自動マージ の check / lint / backend:test 成功報告があるが、この文書保守では再実行していない。native Windows、全 CRLF checkout 全履歴、実 server 起動や稼働 DB の検証を追加で立証したものではなく、無関係な過去の説明も再検証していない。

## 外部エディター URI 連携の検証

[外部エディターの契約](workspace-inspection.md#外部エディターの-uri-連携)を変更するときは三層を分ける。services の editor unit test は URI の path/query encoding・不正構文・file context・旧 schema 読込を検査し、v9 config test は v6〜v9 の全体読込と他設定・空一覧の保持を検査する。[URI tests](../../crates/services/src/services/config/editor/mod.rs#L181-L330)・[Config migration test](../../crates/services/src/services/config/versions/v9.rs#L180-L254)

server の `routes::workspaces::integration::editor_tests` は一時ディレクトリで Workspace / Repo / file の解決と欠落時の非復元、traversal・symlink 逸脱を確認する。実 DB や外部 IDE 起動を試すものではない。[path tests](../../crates/server/src/routes/workspaces/integration.rs#L210-L326)

`node --test tests/chat/external-editors.test.mjs` は実設定 UI・Config controller・選択 dialog を Chromium に載せ、API を fixture に置換する。初期値保存、並べ替え、全削除保存、空一覧保持、対象別 request、古い候補の非表示、リンクの明示クリックと opener 分離を検査する。[fixture 境界](../../tests/chat/external-editors.test.mjs#L15-L65)・[操作検査](../../tests/chat/external-editors.test.mjs#L119-L301)。小さな helper の回帰は [初期値と diff path の Vitest](../../packages/web-core/src/shared/lib/externalEditors.test.ts)にある。

この Sync は凍結 source `887b8049b71093beca6a80ec7e522da2a1c2fab1` の外部エディター変更を、設定・API・パス検査・UI 呼出元・試験定義から静的照合した。製品テストは再実行していない。移行した過去の「開発時の検証記録」と、凍結入力に記録された自動マージ の明示的検証省略は別である。実 Windows IDE、WSL / SSH 認証、Docker Attach と Dev Containers のネスト URI は未検証であり、fixture の成功で補えない。無関係な既存節の再検証も主張しない。

## 文書の適用範囲

- [CONTRIBUTORS](../../CONTRIBUTORS.md) は maintainer review、変更管理、生成ファイルを直接編集しないという方針を記録する。実装の挙動の証拠とは分けて読む。[変更管理](../../CONTRIBUTORS.md)・[生成物の方針](../../CONTRIBUTORS.md)
- 旧 npm 配布手順は廃止され、LVK の npm package を公開しない。現行の Docker 公開・検証は [サーバー配布手順](server-container.md)を参照する。
- 旧Mobile設計のremote-web / relay前提は現在使えない。built previewと[直接アクセス](../integrations/remote-access.md#ui-の直接アクセス)を確認する。
- [OpenWiki maintenance](openwiki-maintenance.md) は、通常開発の tests や release と別の生成・レビュー・公開 checkpoint を説明する。

この Wiki は実装理解のための派生知識であり、試験成績そのものではない。初期生成時の静的調査と、その後の選別移植で行った自動検証・MCP受入を区別する。再検証するときは上記の実装記録と現行コマンドを参照し、未実行や既存 skip を合格に数えない。

認証の回帰検証は `crates/server-access/src/tests.rs`、公開MCPのJWT-to-tool検証、nginx ingress fixture、`tests/server-access/`のブラウザー経路を分けて読む。実Auth0/ChatGPTの受入はこれらと別である。[検証境界の記録](../concepts/server-access.md#検証と残る確認)、[配布の認証境界test](../../deploy/server/access.test.mjs)。

## 未起動予約と host receipt の回帰検証

[未起動予約の終端契約](../architecture/agent-runtime.md#未起動予約の終端と-not_started)を変更する場合、DB の `agent_runtime_reservation_tests.rs` で旧 terminal 行・active host identity の保持、未起動だけの解放、失敗時の event / projection rollback、再送と制約を確認する。初回 host batch が Failed / Cancelled / Crashed / AuditFailed の各場合も、cursor を先に保存して未起動へ誤解放しないこと、foreign Run / Session・保存失敗・conflicting replay で cursor が進まないことを検査する。[予約・batch 試験](../../crates/db/src/models/agent_runtime_reservation_tests.rs#L1-L345)

`reservation_terminal_migration_supports_fresh_and_previous_main_upgrade` は共通 migration 入口で fresh と直前 schema からの upgrade、再実行、trigger / 3 index と foreign key を確認する。port の `prelaunch_released_failure_diagnostic_survives_database_reopen` は Failed 保存後の診断エラーを作り、DB を閉じて再 open しても `not_started` の診断を一度だけ完了できることを確認する。[移行 fixture](../../crates/db/src/migration_compat.rs#L126-L166)・[中断回復 fixture](../../crates/local-deployment/src/agent_run_port.rs#L4654-L4710)

busy_timeout=0 の port fixture では、独立した監視 SELECT の SQLITE_BUSY（code 5）だけを既存2秒 timeout 内で再観測する。writer の失敗・予算・identity の assertion を弱めたり、製品に retry を追加したりする変更ではない。[監視と writer の検査](../../crates/local-deployment/src/agent_run_port.rs#L4815-L4903)

今回の Sync は source `31b6fcf2927a1c7de502753dc7f248da8643b06e` と凍結 manifest / 全4 chunk を照合した。供給 Change Manifest は局所 DB / port / completion 試験と compiler / SQLx check、および最終統合候補の install・frontend build・check・lint・backend:test 成功を記録する。局所 port 試験の初回監視 read 競合と、fixture 修正後の成功は別の結果である。本保守では試験を再実行せず、稼働 DB の履歴予約修復、deployment / restart、実ユーザー Workspace の archive を検証したものでもない。過去の別機能の検証記録はその当時の範囲として保持する。

## 起動回復・受動参照・MCP 操作を変更するとき

[Starting 回復](../architecture/agent-runtime.md#starting-保存失敗と未起動の回復)では、`agent_run_port::tests` の SQLite busy、再起動越し予算、部分終端保存、Stop/重複要求、setup除外、ACK不確実性を分けて確認する。DBのmigration fixtureは旧行の既定値・保持・部分indexと、Starting/host予約フェンスを検査する。画面のPreparingと実開始時計はcanonical conversation / taskAttemptモデル試験が対象であり、実providerが一度だけ起動したことのE2E保証ではない。[port試験](../../crates/local-deployment/src/agent_run_port.rs#L4468-L4903)・[migration試験](../../crates/db/src/models/agent_runtime.rs#L2560-L2659)・[表示試験](../../packages/web-core/src/features/agent-runtime/model/canonicalAgentConversation.test.ts#L1505-L1550)

[Codex resume指示](../integrations/agent-providers.md#codex-resume-の固定指示と現在-identity)は短い現在契約を明示injectする。履歴JSONLの保持解析・版条件・専用fixtureは撤去した。`resumed_host_context_is_visible_before_any_new_turn` は注入順序と失敗時停止、`goal_skill_tests` は初回/継続のSkill・profile・guide参照を検証する。実provider/modelの到達とは区別する。[client試験](../../crates/executors/src/executors/codex/client.rs)・[provider設定試験](../../crates/executors/src/executors/codex.rs)

[受動quota/usage](../integrations/agent-providers.md#受動的な-account-quotausage-参照)のfixtureは非モデルRPCだけを許し、成功・失敗・timeout・cancelと子process回収、admission制限、未知CLI版、単位/null/秘密field除外、所属/provider/profile不一致を検査する。ignored live試験は通常suiteと別であり、実行しなければlive成功とは扱わない。[executor fixture](../../crates/executors/src/account_usage_tests.rs)・[server認可](../../crates/server/src/remote_mcp/provider_usage.rs#L42-L123)

[DevServer](workspace-inspection.md#preview-と開発サーバー)はregistryの観測/settlement、UI manager、stdio scope、外部HTTP receipt/再送/並行startを横断して確認する。`tests/server/run-managed-dev-server.mjs` は隔離instanceと対応するMCP/host binaryを使い、終了確認後だけ成功instanceを削除する。実agent modeは明示的な追加実行であり、通常fixtureから推論しない。一回限り承認のprotocol fixtureとoffline `tests/chat/mcp-approval.mjs` は別の境界である。後者はnetwork/API/providerへ接続せず、ボタンはmock stateのみを変更する。[runner](../../tests/server/run-managed-dev-server.mjs)・[remote試験](../../crates/server/src/remote_mcp/dev_server_tests.rs)・[protocol試験](../../crates/executors/src/executors/codex/client/mcp_approval/tests.rs)・[offline UI](../../tests/chat/mcp-approval.mjs)

[カンバンactivity順序](../concepts/project-and-issue.md#activity-による手動順序の更新)は全5終端・両canonical保存経路・明示sort停止、本文/コメントtransaction、遅延昇格とdrop両順序を確認する。rank helperとChromium部品fixtureはghost/drop/cancel/focusと表示ON/OFFを扱うが、実ネットワーク競合や実provider状態の証拠ではない。[DB回帰](../../crates/db/src/models/agent_runtime_kanban_tests.rs#L750-L865)・[rank試験](../../tests/kanban/order.test.mjs)・[部品fixture](../../tests/kanban/readability.mjs)

コメント本人編集はowner/revision/receiptとWAL競合、raw配信はUTF-8 filename・header injection・全binary bytes・認証とpath containmentを確認する。[コメント試験](../../crates/server/src/remote_mcp/issue_comments.rs#L370-L558)・[file試験](../../crates/server/src/routes/workspaces/files.rs)。詳細契約は [コメント編集](../concepts/issue-collaboration.md#自分の既存コメント本文を編集する)と [Workspace file viewer](workspace-inspection.md#repo-内ファイルを読む)に置く。

凍結入力の候補合成記録は局所Rust・Clippy・型・モデル/部品試験の成功を報告するが、最終統合 `b9fd2194154bfff52fe362eb9253037521f91963` は明示的に正式検証を省略した。初期失敗と修復後の成功、実agentが承認待ちへ到達して取消された証拠、人の承認後のtool成功を区別する。実Auth0/ChatGPT、全本番DB移行、Windows、配布artifactの保証は残る未確認範囲である。移行した過去の「候補合成と限界」。今回の文書保守はソース・試験定義・差分・リンクとOpenWiki lifecycleの検証で、製品試験は再実行していない。

## REVIEW・添付搬送・Wiki再開を変更するとき

REVIEWは既存CODE経路を使うため、mode保存・復元、permission parity、通常会話の終了とsource finalisation抑制を一組で検証する。`tests/review-mode/selector.test.mjs` はChromiumの1440/390幅でREVIEW選択・再表示・送信とCODE/PLAN/GOALを確認する。`run_isolated.py` はUI/API/SQLite/hostを使いproviderだけをmockする。実モデルの指示遵守や外部承認改善は別の未検証事項である。[試験の範囲](../../tests/review-mode/README.md)

添付は `isolated_attachment_remote_stdio_e2e` を新しい `attachment-fixture-*` instanceとその内側のXDG_CACHE_HOMEで実行する。実HTTP MCPとstdio codecから初回・続行まで通し、bytes・サイズ/hash・所属・再送・改変/部分失敗を確認する。native実行はmockで、実ChatGPT/Library fileParams・署名URL期限・host/proxy適合は公開後ゲートに残す。260の保存設定読取routeは267の隔離local routerにも必要であり、製品routeの追加時にfixtureのHTTP面も追従させる。[隔離fixture](../../crates/server/src/remote_mcp/attachment_tests.rs)・[公開後の確認](../integrations/remote-access.md#外部接続と素材搬送を受け入れる)

Wiki継続proofのprotocol testsと `isolated_conversation_repair_publication_drift_and_idempotence` は、所有begin→中断→同じ会話の明示再開→finish→exact publication→archiveを確認する。実外部OpenWikiや実Codexを起動する試験ではない。[proof](../../crates/services/src/services/openwiki/completion.rs)・[server境界](../../crates/server/src/routes/openwiki/completion.rs)・[実Git/SQLite fixture](../../crates/server/src/routes/integrations/conversation_tests.rs)

共有Cargo中間物にはコンパイル時のcheckout絶対パスが残り得る。utilsのプロセス回収テストはPython fixtureの場所を実行時 `CARGO_MANIFEST_DIR` から解決し、削除済みの過去worktreeを参照しない。これは製品のprocess cleanupや共有cacheの全入力に対する一般的な鮮度保証ではない。[fixture参照](../../crates/utils/src/process/owned_group.rs#L355-L435)

今回の統合検証はコード `7a740a94` でfrontend build/check/lint/backend:test/types/SQLxの同一batchを成功させ、以後のtest-only差分を `b803a1ed` の隔離E2E・Chromium・Vitest15件・fmt・server all-targets Clippyで検証した。旧metadata15件削除の整合は正式prepare-db:checkが確認した。初期失敗は履歴に保持し、古いpassを最終SHAの全面再実行と呼ばない。Wikiのみの後続commitにコード試験を付け替えない。

## i18nの常設検査と移行検証

`pnpm run i18n:test`は検査器selftest、`pnpm run i18n:check`は英日キー・空値・補間・正当plural差・JSON重複、literal／有限dynamic参照の存在と生成schema翻訳の同期を調べる。hardcode検出は直接JSX本文に限定する。same-en-ja一致の強制やlocal const／属性／toast等の複雑な独自解析は行わず、英日文面の自然さや全構文の解決を保証しない。[検査器](../../scripts/check-i18n.mjs)・[selftest](../../scripts/check-i18n.test.mjs)

unused検査は文字列・dynamic prefixを使う保守的なinventoryで、コメントや広いprefixが未使用keyを残すことがある。削除判断では呼出元とschemaを確認する。辞書同士の一致やunusedだけをソース参照存在の代用にしない。生成型・schemaの正規確認は `pnpm run generate-types:check` を使う。[unused](../../scripts/check-unused-i18n-keys.mjs)・[生成元](../../crates/server/src/bin/generate_types.rs)

settings-redesignの恒久i18n browserは、言語保存とBrowser通知hint、JA→ENでのcanonical model/override保持、生成schema表示・validationという4ケースを3specに残す。既存製品40ケースと合わせて44件であり、移行用に増やした一般承認・対象payload・再送・各画面表示の網羅は常設化しない。削減した範囲は同等の自動検知が残るとは言えず、checkerは画面配線や一般製品状態を保証しない。[suite設定](../../tests/settings-redesign/playwright.config.ts)・[言語保存](../../tests/settings-redesign/general-i18n.spec.ts)・[model値保持](../../tests/settings-redesign/model-selector-i18n.spec.ts)・[schema](../../tests/settings-redesign/schema-i18n.spec.ts)

診断の確認は既存Wiki/File・認証errorの少数unitで、翻訳した説明と原文をReactテキストとして併記する契約を確認する。fixture成功と実Auth0/ChatGPT/Push/OS配送の受入は別である。検証記録は実行したコードSHAに結び付け、後続Wiki-only commitへ付け替えない。

## 共通orchestration Skillの正本と配布

共通正本は `skills/lvk-orchestration/`。薄いルートが思想・共通原則と読み込み条件を持ち、実行、調査、自動マージと移管・片付け、記録、プレビュー、LVK開発、利用指針の7資料を必要な場面だけ読む。既存の操作用 `lvk-work` と担当実行用auto-merge Skillは別責務として保つ。[ルート](../../skills/lvk-orchestration/SKILL.md)・[配置と変更理由](#共通orchestration-skillの正本と配布)。

ビルドは適切なLVK管理Workspaceの担当coding agentへ委譲し、上位AIは直接実行しない。これはルートSkillの常時適用原則である。[ビルドの担当境界](../../skills/lvk-orchestration/SKILL.md#L10-L17)。追加候補 `a373c5b61732d673eac4eba22c7367bd9625308b` はこの1bulletとOpenAI両manifest・Claude manifestの版 `1.0.43` のみを変更し、プラグイン登録成功を意味しない。[OpenAI版](../../plugins/lvk-orchestration/openai/plugin.json)・[Codex版](../../plugins/lvk-orchestration/openai/.codex-plugin/plugin.json)・[Claude版](../../plugins/lvk-orchestration/claude/.claude-plugin/plugin.json)。

`scripts/package-orchestration.py` は指定Git commitからOpenAI/Claudeの共通Skillと全参照資料を生成し、配布元SHA・一覧・ZIPハッシュをreceiptへ保存する。作業木の未commit内容を配布元にしない。Claudeの `--claude-without-connection` はmanifest・共通Skill・配布元記録のallowlistだけを含め、既存コネクターを上書きしない。接続付き方式は明示HTTPS URLで `.mcp.json` を追加する。[生成入口](../../scripts/package-orchestration.py)。

移管後にAI判断で旧IssueをCancelledへ変えた場合は、Issueと全Workspaceのarchive・物理削除を個別に確認する。同対象・削除範囲の既承認は再確認せず、未承認やbusy拒否は迂回しない。部分移管や人間の単なるRun取消から自動archiveを導かず、Cancelledだけを片付け完了としない。[片付けの完了条件](../../skills/lvk-orchestration/references/auto-merge.md#cleanup-completion-gate)。

LOCAL-242統合ではコード候補 `a1f87c7c93e8818aa84d4213788c1429b723ecd7` の関連内容が検証済みsource `c14c22f` と同一であることを確認した。候補から両配布経路を生成し、参照・ZIP/CRC・正本byte一致・接続なしの正確な集合を管理commandで再確認した。参考アプリ検証は既存証拠と差分からagent判断で省略し、今回の成功へ付け替えていない。Wikiはその後の別commitである。プラグイン登録、Claudeアカウント導入/認証、サーバー再起動・デプロイは実施していない。配布生成成功、登録保存、実読、mainへのGit公開は独立した結果として扱う。

## チャット表示と入力の再検証

依存をfrozen lockfileで準備し、`pnpm run chat:test`、agent-runtimeのcanonical conversation/timelineとsessionDraftのVitest、影響packageの型検査・lintを選ぶ。built-app経路では先に `pnpm --dir packages/local-web run build` と `node scripts/dev-cargo.mjs build --locked --bin server --bin agent-process-host` を行い、`python3 tests/chat/run-isolated.py` を実行する。runnerは固有のinstance・loopback portを使い、finallyで停止・port閉鎖を確認する。実Codex steeringを含むため認証済み環境の明示的受入として扱い、共有devへ向けない。[runner](../../tests/chat/run-isolated.py)

`isolated.spec.ts` の合成AgentRun履歴、実Scratchと制御follow-up、実Codex steeringは別の証拠である。prepend座標・最新追従・遅延Workspace応答、末尾改行・IME・Enter設定、再接続後の一度だけのuser inputを確認し、component成功を実provider成功へ拡張しない。[試験定義](../../tests/chat/isolated.spec.ts)


### 管理ファイル集約後の開発入口

開発用preview設定は `.lvk/preview/vite.config.mjs` に置き、`scripts/ui4-preview.py` もこの場所を参照する。一般利用者Repoへpreview機能を追加する変更とは区別する。共有設定を隠さないため `.lvk/` 全体をwatcher・検索・snapshotから除外せず、`shared/`・`attachments/` の実行時子だけを除外する。旧配置の自動移行や既存データ整理は行わない。[preview設定](../../.lvk/preview/vite.config.mjs)・[起動参照](../../scripts/ui4-preview.py)・[所有境界](../concepts/workspace.md#lvkの所有境界)

LOCAL-346単独のコード候補 `3b88263a13d2e15606ea445c1a6753919dc08303` は固定base `5dc268936` へsource `35e96363` を無競合統合した。候補上の管理実行でCargo/release fixture 6件とPython Runner 10件、差分空白・選択履歴を確認した。release初回はyaml不足で失敗し、offline固定依存導入後に同一command全体が成功した。Rust/Frontend全suiteと実provider/全UIは既存sourceおよび3本結合の証拠・無競合差分を根拠に担当判断で再実行を省略した。旧候補の成功を本候補のpassに置換しない。348・344・独立339は追加採用していない。環境切替・再起動・既存実データ移行/削除は行っていない。


## LOCAL-361の統合検証とbundle

コード候補 `da8f775878924b833971888e7c8ee28586cdf84b` は固定base `e093e788…` へG `86a2708f…` を競合なくmergeし、Gとの全tree一致・両祖先・空白・cleanを管理commandで確認した。Gの型生成・影響crate/consumer検査と独立レビュー、Fの本文/guide焦点検証は既存SHAの証拠であり、今回候補の再実行passへ付け替えない。重複する全check/lint/backend suiteと実CLI行列は理由と残存riskをagent判断のskipとして記録した。

今回候補ではlocked install、実frontend build、developer-runtime付きdev最適化のserver/agent-process-host/MCP buildを実行した。現行switch kitの `bundle.py` と `test_real_server.py` でsourceアクセス拒否下のasset/API、pending/no-pending migration、async receipt、host bootstrap、bundled stdio MCPを確認した。起動のbuild_infoは当該SHA・development・dirty=falseだった。pip不在とwheel tag誤選択の準備失敗2件は保持し、task専用のpysqlite3 wheelを正しく展開した修正版で成功した。一時依存・隔離processは回収し、要求されたbundleのみ共有persistentに保持した。[実bundle試験の定義](../../deploy/developer/switch/test_real_server.py)・[正式buildと切替手順](../../deploy/developer/switch/README.md)

これはlive DB互換性の全履歴試験、実Auth0/browser/model、dev切替や再起動の成功ではない。Wiki commitはコード候補と別であり、main publicationは担当Run終了後のhost確認に従う。ガイド移設だけをtoken削減の実測とは扱わない。

## LOCAL-366/368の統合検証とbundle

固定base `6ad758d6ee1d74785a6e70e0f17ef93a0ab43d8e` へ366 `68c3faab…` と368 `7a6ad766…` を競合なくmergeした。コード検証SHAは `9647311264878cc047f465fd05869597c1d524da`。追加修復は `account_usage_tests.rs` の2つのwildcardを削除し、qa-mode時に存在するQaMockだけをcfg付きで明示したもの。単純削除の旧候補はQA構成の非網羅matchで失敗し、その後の通常全target Clippyとexecutors/serverのqa-mode全target Clippyが成功した。警告抑制や型の緩和は使っていない。

最終コードSHAではexecutors libが350成功・既存7ignored、Markdown保存往復6件、両実rendererを使う明暗・狭幅/広幅のbrowser fixture、UI/web-core/local-web型検査が成功した。sourceのlint・Claude隔離E2E・frontend79件・Kanban幅行列は固定SHA付きManifestの既存証拠を参照し、包括check/lint/backend suiteとともに担当判断で反復を省略した。これは今回のpassではない。ignored live診断、実OS clipboard/mobile、認証後アプリ・実provider/model/quota、外部preview/HMRは未検証である。[Markdown試験](../../tests/markdown/README.md)・[usage回帰](../../crates/executors/src/account_usage_tests.rs)

管理Workspaceで実frontendと `--purpose dev` の最適化server/agent-process-host/vibe-kanban-mcp（developer-runtime付き）をbuildし、bundle.pyで埋込みpayloadを照合した。隔離受入はPython SQLite 3.40.1に必要な拡張がなく開始前失敗し、続くcommandもshのpipefail非対応で開始前失敗した。hash確認したpysqlite3 wheelをtask-localへ展開し、Bashを明示して、完成済みbundleの試験だけを成功させた。失敗の履歴と完全build/受入ログは保持し、重複buildの反復は理由付き省略として記録した。

[test_real_server.py](../../deploy/developer/switch/test_real_server.py) は専用DBを過去schemaから構築し、pending/no-pending migrationの2ケース、DB integrity、sourceアクセス拒否下の埋込みasset hash/API、async workerとreceipt再読、host bootstrap、Workspace cwdからのbundled stdio MCPを検査した。稼働identityは上記コードSHA・development・dirty=falseだった。live DBのコピーや認証変更はしておらず、live DB全履歴の互換性・実Auth0/UI/model成功まで証明しない。

bundleとSHA256SUMS・code-commit.txt・試験ログはrepositoryの `.lvk/shared/persistent/auto-merge-366-368-9647311264878cc047f465fd05869597c1d524da/` へ保持した。Workspace archive後も読む成果物であり、通常の一時cleanup対象ではない。導入は[切替手順](../../deploy/developer/switch/README.md)の後続操作で、今回のbuild成功はdev切替や再起動の成功ではない。368 Workspaceは後続改善・preview2用に保持する。コード検証後のprepare_wikiで原本journal保存を確認してからWikiを開始し、Wikiだけを別commitとする。正式main publicationは担当Run終了後のhost結果で別に証明する。

LOCAL-368はGit正本のorchestration Skillと既存Codex REVIEW指示へ、人間の判断事項を結論直後へ集約する指針を追加した。DECISIONは未決の選択だけに限定し、確定した人間の意図は通常のラベル付き引用で扱う。空欄の固定sectionや通常Runへの新規共通注入は増やしていない。[Skill](../../skills/lvk-orchestration/SKILL.md)・[REVIEW指示](../../crates/executors/src/executors/codex.rs#L1-L10)
