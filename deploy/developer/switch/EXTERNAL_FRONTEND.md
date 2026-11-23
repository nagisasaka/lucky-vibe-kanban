# 共有 dev のフロント／バックエンド分離

この入口は `developer-runtime` を持つ Rust server と、明示設定
`LVK_DEV_EXTERNAL_FRONTEND=1` の組合せだけで有効です。安定版の埋め込み配信、
preview2 の作業ツリー＋Vite HMR、認証設定を変更しません。別の配信サービスは不要です。
外部 dist も既存 Rust router の認証・maintenance layer の内側にあります。

## 選択と保持

`current.json` の既存 `release` / `manifest` はバックエンドを指し、追加の `frontend` は
`{format:1, revision:FULL_SHA, files:{relative_path:SHA256}}` です。
実体は `INSTANCE/frontend-releases/FULL_SHA/` に配置します。
選択全体を既存の `start.lock` 内で原子的に置換します。
同じ SHA に異なる内容を上書きできません。内容変更は別のコミットとして用意してください。

Vite の base は `/__lvk_frontend/FULL_SHA/`。旧タブの dynamic import は旧 SHA の
配信物へ到達します。旧 dist は自動削除しません。保持量を監視し、旧タブが不要になり、
バックエンドの復旧選択・rollback 用にも不要と確認した世代だけを operator が整理します。
index と通常 URL は no-cache、世代付きアセットは immutable です。
不明な世代付き URL は SPA HTML に変換せず 404 を返します。
`/api/info` の `build_info.revision` は従来どおり実バイナリのビルド SHA、
`frontend_revision` は選択済み dist の SHA です。異なる SHA は正常です。
安定版では追加値を送らず、従来の単一 SHA 表示を維持します。

## ビルドと切替

前提は、統合・レビュー済みのクリーンな固定コミットの checkout、locked dependencies、
既存 dev Cargo toolchain、後述の初回移行です。成果物は Workspace ルートの `.lvk/local/` に
置きます。以下のパスは operator が実対象へ明示置換する例です。本作業の範囲では
live dev / stable / preview2 に対して実行しません。

```sh
python3 deploy/developer/switch/build.py \
  --source /absolute/pinned-checkout \
  --output /absolute/Workspace/.lvk/local/candidate \
  --mode frontend \
  --reason '比較した旧SHA→対象SHA、変更ファイル、API互換性の判断根拠'
python3 /absolute/INSTANCE/switch/frontend_switch.py \
  --bundle /absolute/Workspace/.lvk/local/candidate/frontend \
  --reason '同じ判断根拠' --dry-run
python3 /absolute/INSTANCE/switch/frontend_switch.py \
  --bundle /absolute/Workspace/.lvk/local/candidate/frontend \
  --reason '同じ判断根拠'
```

フロント単独ビルドは Cargo を呼びません。完成した dist の全ファイル、HTML、
Vite manifest の静的・動的依存を検証してからコピーし、同期・再検証後に選択します。
server/host/stdio MCP の再起動、DB ファイルの直接操作・migration・backup、Run の操作を行いません。
HTTP health のアセット hash 照合に失敗した場合は、同じロック内で直前の選択へ戻します。
結果と前後の選択は `frontend-operations/UUID/` に残ります。

バックエンド単独は `--mode backend`、API・共有型・生成物・共有依存・判断が曖昧な場合は
`--mode both`（既定）にします。巨大な変更分類は行いません。`--reason` に対象入力と判断を
必ず記録します。後者は同一 SHA の dist とバイナリを bundle に含めます。

```sh
/absolute/INSTANCE/switch/start.sh \
  --bundle /absolute/Workspace/.lvk/local/candidate/backend --dry-run
/absolute/INSTANCE/switch/start.sh \
  --bundle /absolute/Workspace/.lvk/local/candidate/backend \
  --request-id UUID --no-new-work
```

バックエンド経路は既存の停止・整合 backup・起動・health・DB検証を維持します。
backend-only は現在の frontend 選択を保持し、both は一つの current 選択で両側を変更します。
Cargo は同じ checkout の依存グラフとビルド入力の鮮度に基づいて3バイナリの再コンパイル要否を
判定します。別 checkout の補助バイナリを推測で流用しません。
`build.json` に入力 SHA、判断、コマンド、前後別のビルド時間、実バイナリ hash と
補助バイナリの由来を記録します。frontend operation は切替時間も記録します。

## 初回移行

初回だけは既存の埋め込み dev から起動設定を変えるため、別途承認された保守作業です。
通常のフロント単独切替ではありません。

1. この revision の kit・developer-runtime バイナリと、同一コミットの外部 dist を隔離検証する。
   `build.py --mode both` の候補と旧 release / launch / current を保持する。
2. 既存の [切替・失敗復旧手順](README.md) に従い、新規作業停止、対象 PID の確認、
   安全停止と整合 backup を行う。DB 互換性は切替前に確認する。
3. 固定 kit に従来5ファイルと `frontend_bundle.py`、`frontend_switch.py` を整合して配置する。
   実行中 worker の kit は置換しない。source symlink は使わない。
4. 検証済み backend release を既存の配置形式で置き、`frontend_bundle.install` と同じ
   検証・コピー・fsync 手順で `frontend-releases/SHA` を配置する。
   `current.json` の frontend に同梱の frontend manifest を追加し、backend release と合わせる。
   `launch.json.environment` に `LVK_DEV_EXTERNAL_FRONTEND=1` を追加する。
   その他の認証・暗号鍵参照・instance/port 設定は保全する。
5. 固定 launcher で起動し、両 SHA、UI、API、WebSocket、Agent host、MCP を確認する。
   初回より前に開いた埋め込み版タブはリロードする（旧 URL は世代付きではない）。

既存 server 稼働中に launch.json のみ編集すると既存 identity 検証が拒否します。
この拒否を迂回して通常 start.sh で初回設定変更を行わないでください。

## ロールバックと失敗

フロントは `frontend_switch.py --bundle INSTANCE/frontend-releases/OLD_SHA --reason ...`
で保持済みの旧 dist を再選択します。旧 index を持つタブの世代付き chunk は保持されます。
無効な候補、ロック競合、未解決の backend operation は切替前に拒否します。

バックエンド／両側の失敗後は従来どおり停止段階・backup・実プロセス・DB を確認します。
DB migration の自動 rollback はありません。旧 DB との互換性・必要な復元を確認した上で、
旧 backend とそれに対応する frontend を一組として既存の復旧手順で選択します。
`frontend-operations` の previous と backend operation の settings/current を復旧根拠に使い、
backend bundle 内の過去 frontend 宣言を「直前の稼働 frontend」と取り違えないでください。

## 検証範囲

`test_frontend_bundle.py` はグラフ・hash・path 検証、`test_frontend_switch.py` は隔離ファイルと
mock process/HTTP による原子的選択・復元・ロック・既存状態保全を扱います。
Rust `routes::dev_frontend` は実ファイルからの選択・旧 chunk・rollback・破損拒否を扱います。
これらだけでは実 server 全体、ブラウザ、実 Auth0、稼働環境での受入を証明しません。
`test_external_real_server.py` は、実 developer-runtime バイナリの新規隔離 instance で
PID/SHA 不変のフロント切替、旧 chunk、rollback、無効物拒否、health失敗復元、
backend-only の dist 保持、paired 選択、実 host bootstrap / stdio MCP を検証します。
`--frontend-bundle` で実 Vite dist、`--browser-output` で Chromium の個別 SHA 表示も検証できます。
fixture の frontend SHA は合成識別子であり、実コミットの放出実績とは区別します。

```sh
python3 -m unittest discover -s deploy/developer/switch -p 'test_frontend*.py' -v
python3 -m unittest discover -s deploy/developer/switch -p test_split_build.py -v
node scripts/dev-cargo.mjs test -p server --features developer-runtime --lib frontend
# TMPDIR は Workspace/.lvk/local 以下の専用空ディレクトリを指定する。
# Python SQLite < 3.42 の場合は test_real_server と同じ試験用 pysqlite3 が必要。
python3 deploy/developer/switch/test_external_real_server.py \
  --source /absolute/checkout --bundle /absolute/test-backend-bundle \
  --frontend-bundle /absolute/test-frontend-a \
  --diagnostics /absolute/Workspace/.lvk/local/diagnostics
```

実機未確認と実行した隔離試験・時間は作業報告で区別します。
