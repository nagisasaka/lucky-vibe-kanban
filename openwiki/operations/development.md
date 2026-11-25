---
type: operation
title: 開発と検証
description: source 開発、検証、cache と復旧の境界。
tags: [development, testing]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-25T03:59:01.553Z
sources:
  - id: openwiki-source-21cc2c4c9c881a800c33a448
    resource: repo://.github/actions/cargo-checks-common-setup/action.yml
  - id: openwiki-source-16e6e7418602542c69565bde
    resource: repo://Cargo.lock
  - id: openwiki-source-5c2565a649f8be059d306209
    resource: repo://crates/db/src/lib.rs
  - id: openwiki-source-7b783ec1a704bf9dadb9c003
    resource: repo://crates/db/src/migration_compat.rs
  - id: openwiki-source-b32329ab5b5881417948073f
    resource: repo://crates/db/tests/sqlite_runtime.rs
  - id: openwiki-source-f771a8041ae618a5a49437b9
    resource: repo://deploy/developer/STAGING.md
  - id: openwiki-source-be93997623f6f900b78dd099
    resource: repo://deploy/developer/switch/build.py
  - id: openwiki-source-6c34da0ea16f48ea0096fb57
    resource: repo://deploy/developer/switch/EXTERNAL_FRONTEND.md
  - id: openwiki-source-7b357f516273c69d1ac79fb3
    resource: repo://deploy/developer/switch/frontend_switch.py
  - id: openwiki-source-a764bf933c082e67ed4b6192
    resource: repo://deploy/developer/switch/launcher.py
  - id: openwiki-source-9e0cfce0846bb98d2dc47de0
    resource: repo://deploy/developer/switch/start_binary.py
  - id: openwiki-source-6dc3f97222d590fbb8686593
    resource: repo://deploy/developer/switch/test_split_build.py
  - id: openwiki-source-b7793decf9d7c9ba48e57e0f
    resource: repo://rust-toolchain.toml
  - id: openwiki-source-8ea6c46d63953a7104f5dd97
    resource: repo://scripts/dev-cargo.mjs
  - id: openwiki-source-e433d5ca645048f1043ca4a3
    resource: repo://tests/sqlite-upgrade/run.py
generated: { by: "codex", at: "2026-11-25T03:59:01.553Z" }
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

## 共有 dev の分離ビルドと切替

共有 dev は変更していない側の再ビルド・再起動を避けるため、Vite dist と Rust backend を別々に更新できる。[Issue 原文](../../docs/changes/2026-11-24-LOCAL-477-2f87d09e.md)はこの意図を記録し、現在の配信契約は[Web と同期](../architecture/web-and-sync.md#共有-dev-の配信世代と稼働-sha)にある。安定版の埋め込み配布と preview2 の HMR は別経路である。

前提は、レビュー済みのクリーンな固定 checkout、locked dependencies、指定 toolchain、隔離環境での互換性検証と、次節の初回移行である。成果物は [Workspace](../concepts/workspace.md) ルートの `.lvk/local/` に置く。共有環境への切替は別途認可された運用であり、ビルド成功だけでは切替承認にならない。

| mode | ビルドと選択 |
| --- | --- |
| `frontend` | Vite のみを実行し、server・agent host・stdio MCP を再起動しない。DB・migration・Runを操作しない |
| `backend` | Rust の3バイナリを対象とし、現在の frontend 選択を保持する。再コンパイル要否は同じ checkout の Cargo 依存グラフ・鮮度判定に委ねる |
| `both`（既定） | 同一 SHA の backend と dist を一組にする。API・共有型・共有依存の変更や互換性判断が曖昧な場合に使う |

1. `python3 deploy/developer/switch/build.py --source /absolute/pinned-checkout --output /absolute/Workspace/.lvk/local/candidate --mode frontend --reason '旧SHAと対象SHA、変更入力、互換性判断'` で候補を作る。mode は表に従って選ぶ。成功時だけ完成候補名へ rename する。失敗した `.pending-*` は診断用であり、選択しない。[ビルド実装](../../deploy/developer/switch/build.py)
2. frontend は固定 kit の `python3 /absolute/INSTANCE/switch/frontend_switch.py --bundle /absolute/Workspace/.lvk/local/candidate/frontend --reason '同じ判断根拠' --dry-run` で検査する。実行権限と対象を確認した後、`--dry-run` を外して切り替える。
3. backend/both は `/absolute/INSTANCE/switch/start.sh --bundle /absolute/Workspace/.lvk/local/candidate/backend --dry-run` を経て、既存の[停止・backup・切替手順](../../deploy/developer/switch/README.md)の `--request-id UUID --no-new-work` 付き実行へ進む。安全停止、整合 backup、起動後の health と DB 確認を省略しない。
4. frontend の結果は `frontend-operations/UUID/`、backend は `switch-operations/UUID/` で確認する。両 SHA と配信 JS/CSS hash、UI・API・WebSocket・MCP を確認する。frontend 単独なら backend の PID と SHA が不変であることも確認する。

frontend bundle は HTML、Vite manifest の静的・動的依存、全ファイル hash と path を確認してから配置する。同じ SHA の別内容は上書きできない。`start.lock` 内で `current.json` 全体を原子的に選択し、HTTP health に失敗すると直前の選択へ戻す。ロック競合や未解決 backend operation は切替前に拒否する。[bundle](../../deploy/developer/switch/frontend_bundle.py)・[切替](../../deploy/developer/switch/frontend_switch.py)

`build.json` は入力 SHA、判断、コマンド、ビルド時間、実バイナリ hash と補助バイナリの由来を残す。Cargo が再利用した補助バイナリの元コンパイル commit を推測せず、`original_build_revision` は不明として記録する。frontend operation は切替時間も記録する。

### 初回移行と復旧の境界

初回は埋め込み dev の起動設定を変更する外部 operator の保守作業であり、通常の frontend 単独切替ではない。[初回移行の原手順](../../deploy/developer/switch/EXTERNAL_FRONTEND.md#初回移行)と[失敗時の復旧](../../deploy/developer/switch/README.md)を併用する。

1. 同じ固定 revision の kit、`developer-runtime` バイナリ、外部 dist を `--mode both` で用意し隔離検証する。旧 release、kit、launch/current と必要な稼働アセットを保持する。
2. 認可された保守時間に新規作業を停止し、実 PID の確認、安全停止、整合 backup を行う。DB 互換性は切替前に確認する。
3. 従来5ファイルに `frontend_bundle.py` と `frontend_switch.py` を加えた同一 revision の固定 kit を配置する。実行中 worker の kit は置換せず、source symlink で代用しない。
4. 検証済み backend release と、`frontend_bundle.install` の検査・copy・fsync 手順に従った `frontend-releases/SHA` を配置する。`current.json.frontend` に対応 manifest を加え、backend と組にする。`launch.json.environment` に `LVK_DEV_EXTERNAL_FRONTEND=1` を加え、認証・暗号鍵参照・instance・port 設定を保全する。
5. 固定 launcher で起動し、両 SHA、UI、API、WebSocket、Agent host、MCP を確認する。移行前の埋め込みタブは世代URLを持たないためリロードする。

注意: 稼働中に launch.json だけを変えると identity 検証に拒否される。この拒否を迂回して通常の start.sh や既存 self_restart に split-built backend だけを渡す初回移行は行わない。初回適用には対象環境に合わせた正式な外部 operator 手順が必要であり、今回のコード統合は実機切替を含まない。

frontend rollback は `frontend_switch.py --bundle INSTANCE/frontend-releases/OLD_SHA --reason '復旧理由'` で保持済み dist を再選択する。旧タブの lazy chunk と復旧用 dist は自動削除しない。保持容量を監視し、旧タブも復旧選択も不要と確認できた世代だけを operator が整理する。

backend/both の失敗は停止段階、backup、実プロセス、DBを確認して復旧する。DB migration の自動 rollback はない。旧 backend と互換な DB を確認した上で、直前の frontend と組に戻す。`frontend-operations` の `previous.json` と backend operation の保存済み current を根拠とし、backend bundle の古い frontend 宣言を直前の稼働状態と取り違えない。

`test_frontend_bundle.py`、`test_frontend_switch.py`、`test_split_build.py` は hash・依存グラフ・原子的選択・失敗復元・build dispatch を扱う。Rust `routes::dev_frontend` は配信と認証境界、`test_external_real_server.py` は隔離実serverでの切替と、指定時の実Vite dist・Chromiumを扱う。fixture 成功は実 Auth0、共有環境への初回導入、実機受入を証明しない。

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

## SQLite と SQLx の更新・互換性検証

LOCAL-495 は SQLite 自体を更新するため、SQLx 0.9.1 と libsqlite3-sys 0.40.0 を [Cargo.lock](../../Cargo.lock) に固定した。サーバーが使用する同梱 SQLite の期待値は 3.54.1 で、FTS5・preupdate hook・unlock notify は [runtime 試験](../../crates/db/tests/sqlite_runtime.rs)で検査する。検索機能や WAL 化はこの変更に含めない。[Issue 原文](../../docs/changes/2026-11-25-LOCAL-495-b65b381b.md)

Rust は `nightly-2026-03-06`、CI の sqlx-cli は 0.9.1 を使う。依存を更新する際は [toolchain](../../rust-toolchain.toml)、[Dockerfile](../../Dockerfile)、[CI setup](../../.github/actions/cargo-checks-common-setup/action.yml)を一組として確認する。SQL query metadata は `pnpm run prepare-db` で生成し、`pnpm run prepare-db:check` で照合する。既存の metadata を手で調整して型差分を隠さない。

SQLx 0.9 の動的 SQL は、識別子の allowlist・引用と値の bind を維持して `AssertSqlSafe` を適用する。この型は入力を自動的に無害化する仕組みではない。[動的 query](../../crates/db/src/models/design_note_read.rs)を変更するときも、SQL の生成元を確認する。migration の API 追従は埋め込み SQL と LF/CRLF の既知 checksum を選択する既存境界を維持する。[互換 migration](../../crates/db/src/migration_compat.rs)

[DBService](../../crates/db/src/lib.rs)の journal mode は DELETE、busy timeout は5秒のままである。SQLite 更新を理由とした schema migration、WAL 切替、VACUUM は追加していない。既存の通知 hook は維持し、[通知試験](../../crates/services/src/services/events/hook_tests.rs)と runtime 試験が削除通知、接続設定、NULL と transaction recovery の回帰を扱う。

旧版と新版の実 DBService を比較する場合は、稼働 DB ではなく次の隔離 fixture を使う。[fixture 手順](../../tests/sqlite-upgrade/README.md)

1. 旧版・新版の固定 checkout に `tests/sqlite-upgrade/sqlite_upgrade_fixture.rs` を一時的に `crates/db/tests/` へコピーし、それぞれの toolchain で `node scripts/dev-cargo.mjs test -p db --test sqlite_upgrade_fixture --no-run` を実行する。
2. 生成した二つの実行ファイルを指定し、`python3 -B tests/sqlite-upgrade/run.py --old /absolute/old-test --new /absolute/new-test --repo /absolute/new-checkout` を実行する。driver の期待版は旧3.46.0・新3.54.1であり、任意の旧版を受け付ける汎用ツールではない。
3. driver の結果、実行ファイル hash と対象 SHA を保存する。旧→新→旧の読み書き、全テーブルと migration receipt、integrity/FK、双方向 Project export/import、既存 Python `backup_data` による backup と復元後の再接続を確認する。Project import は ID を再割当てし、`extension_metadata` を除外する既存契約に従う。
4. 一時コピーした Rust fixture を削除する。driver は一時 instance と package を終了時に掃除する。失敗時は出力を保持し、実 DB の migration 履歴を手修正して試験を通さない。

注意: fixture の存在は実行成功の証明ではない。小規模な代表データの互換性は、全 production データ、添付実体、将来の schema・SQLite 機能や拡張、Docker 配布、実機切替を保証しない。Python の SQLite とサーバーにリンクされた SQLite は別である。配備と rollback は [サーバー配置と復元](server-container.md)の整合 backup と版対応の確認に従う。

## 開発用 Cargo の用途選択

開発入口の `--purpose` は subcommand より前に置く。既定の `preview` は最適化なしの開発ビルド、`dev` は最適化した開発 server、`stable` は配布向け profile の確認に使う。例えば `node scripts/dev-cargo.mjs --purpose dev build` とする。`dev` と `stable` は compile 操作に release profile を使う。[入口](../../scripts/dev-cargo.mjs)

注意: `--release`、`-r`、`--profile` を直接指定できない。`CARGO_INCREMENTAL`、`CARGO_BUILD_INCREMENTAL`、設定済み `build.incremental` も値にかかわらず拒否される。競合設定を解除して入口を使うか、別の設定を維持する場合は意図的に raw Cargo を使う。

試験の記録や fixture は対象の動作を検証するもので、実機・実 provider・配備環境の成功を保証しない。
