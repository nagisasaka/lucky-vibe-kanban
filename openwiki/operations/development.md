---
type: operation
title: 開発と検証
description: source 開発、検証、cache と復旧の境界。
tags: [development, testing]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:45:55.554Z
sources:
  - id: openwiki-source-f771a8041ae618a5a49437b9
    resource: repo://deploy/developer/STAGING.md
  - id: openwiki-source-8ea6c46d63953a7104f5dd97
    resource: repo://scripts/dev-cargo.mjs
generated: { by: "codex", at: "2026-11-23T00:45:55.554Z" }
---

# 開発と検証

[操作の段階](../quickstart.md#操作の段階)を区別し、[README](../../README.md) を開発の入口とする。実行対象は [package.json](../../package.json)、[Cargo workspace](../../Cargo.toml)、各試験の README で確認する。Node は 20 以上、pnpm は 10.13.1 が指定され、Rust は [toolchain](../../rust-toolchain.toml) に固定される。`pnpm run dev` はローカル Web と backend を起動し、backend watch は server と agent-process-host を扱う。

## 検証の選択

`pnpm run check` は frontend 型検査と backend check、`pnpm run lint` は lint と Clippy、`pnpm run backend:test` は Rust workspace tests を実行する。`pnpm run workflow:e2e` は専用 fixture の browser tests、`pnpm run server:check` は配布 kit の Node tests である。特定変更には対象の focused tests を加え、fixture の成功を実 provider、実認証、配備済み環境の受入と混同しない。[scripts](../../package.json)・[システム境界](../architecture/system.md)

POSIX の LVK source 開発は `node scripts/dev-cargo.mjs check --workspace` などの入口を使う。checkout の target と利用可能な共有 build cache を分け、競合する profile／wrapper 設定は拒否する。共有 cache の削除は他の checkout に影響し得るため、利用状況を確認する。[入口](../../scripts/dev-cargo.mjs)・[試験](../../scripts/dev-cargo.test.mjs)

Agent の状態や監査の変更は [runtime](../architecture/agent-runtime.md)、graph は [Workflow](../concepts/workflow-attempt.md)、実環境の更新と復元は [server 運用](server-container.md) を参照する。既存の試験記録は対象 SHA と実施範囲を確認し、今回の実行結果として再利用しない。


## 保存先と runtime の境界

開発入口は最終 `target/` を checkout に置き、利用可能な `.lvk/shared/cache` の公式 build-dir を中間生成物に使う。共有領域がなければ checkout 内に戻す。`cargo clean` は共有中間物も削除するため、この入口では禁止する。[入口](../../scripts/dev-cargo.mjs)

コンパイル時は checksum freshness を使うが、build script の file input は mtime の影響を受ける。共有 build-dir は外部 build script や compile 時の source path の安全性を保証しない。稼働中の debug frontend が読む staging source を保持する。[隔離開発と staging](../../deploy/developer/STAGING.md)

`LVK_INSTANCE_DIR` は data、runtime、ports を分離するが OS sandbox ではない。実行中の instance、DB、Git worktree は独立した所有と寿命を持つ。実機切替は [専用手順](../../deploy/developer/switch/README.md) を参照する。

## モバイル向け built preview

built preview は実行中の backend に接続する frontend のビルド結果を確認する。通常の HMR 開発とは別の経路である。[スクリプト](../../scripts/start-web-preview.sh)

1. 既存 backend を起動する。既定の接続先は `127.0.0.1:4021` で、変更する場合は `BACKEND_PORT` を指定する。
2. `pnpm run local-web:preview` を実行する。frontend は既定で port 4030、`HOST=0.0.0.0` で待ち受ける。必要に応じて `FRONTEND_PORT` と `HOST` を指定する。
3. 表示された URL に接続する。出力は `dist-preview` に作られ、Cargo に埋め込む assets とは分かれる。
4. source を変更したらコマンドを停止して再実行し、再ビルドする。HMR はない。

注意: port は 1〜65535 の整数で、frontend と backend に同じ値は使えない。指定した frontend port が使用中なら strict port により起動に失敗する。

## Project key の互換 migration

通常の server 起動は互換 migration を選択する。明示的な保守実行が必要な場合は、認可済みの既存 DB とその保全を確認し、[バックアップと復元](server-container.md) の手順に従う。互換選択の仕組みは [システム境界](../architecture/system.md) を参照する。

1. 対象 DB の正しいパスと操作権限を確認する。ツールに既定の DB 接続先はない。
2. リポジトリ root で `SQLX_OFFLINE=true DATABASE_URL=sqlite:<認可済みの既存DBパス> cargo run -p db --bin migrate_compat` を実行する。成功時は migration が適用され、接続 pool が閉じられる。[実行入口](../../crates/db/src/bin/migrate_compat.rs)
3. エラーの場合は対象と checksum を確認し、履歴を手修正せず原因を調査する。[互換選択](../../crates/db/src/migration_compat.rs)

注意: 互換版の適用後、raw `cargo sqlx migrate run` は元の SQL しか知らず checksum 不一致となる。旧 server も互換 checksum を拒否するため、単純な旧版への切り戻しはサポートされない。`_sqlx_migrations` の手修正で回避しない。[保守資料](../../crates/db/migration_compat/README.md)

## 開発用 Cargo の用途選択

開発入口の `--purpose` は subcommand より前に置く。既定の `preview` は最適化なしの開発ビルド、`dev` は最適化した開発 server、`stable` は配布向け profile の確認に使う。例えば `node scripts/dev-cargo.mjs --purpose dev build` とする。`dev` と `stable` は compile 操作に release profile を使う。[入口](../../scripts/dev-cargo.mjs)

注意: `--release`、`-r`、`--profile` を直接指定できない。`CARGO_INCREMENTAL`、`CARGO_BUILD_INCREMENTAL`、設定済み `build.incremental` も値にかかわらず拒否される。競合設定を解除して入口を使うか、別の設定を維持する場合は意図的に raw Cargo を使う。

試験の記録や fixture は対象の動作を検証するもので、実機・実 provider・配備環境の成功を保証しない。
