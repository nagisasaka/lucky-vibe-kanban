---
type: operation
title: サーバー配置と復元
description: HTTPS 配置、永続化、バックアップと復元。
tags: [server, docker, backup, recovery]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-25T07:33:39.418Z
sources:
  - id: openwiki-source-a13fe4db1eee073d0a7e2c4d
    resource: repo://crates/server/src/main.rs
  - id: openwiki-source-f771a8041ae618a5a49437b9
    resource: repo://deploy/developer/STAGING.md
  - id: openwiki-source-df6b7819295f53fc2381f23d
    resource: repo://deploy/server/compose.yaml
  - id: openwiki-source-b62c808fd8e21e00434e716d
    resource: repo://deploy/server/ffmpeg-smoke.sh
  - id: openwiki-source-62c18d078e22dc3ceaf8bd8e
    resource: repo://deploy/server/FFMPEG.md
  - id: openwiki-source-bb1ebe868e35e9e500714501
    resource: repo://Dockerfile
  - id: openwiki-source-ef37b53f9e3030caf4e3bc84
    resource: repo://docs/changes/2026-11-25-LOCAL-501-0fd246f8.md
generated: { by: "codex", at: "2026-11-25T07:33:39.418Z" }
---

# サーバーコンテナー運用

標準 server image は local LVK、agent と nginx をサーバーで動かし、browser に HTTPS を提供する。認証は利用者ごとの OS 隔離ではなく、agent と server は同じ実行 user を使う。read-only secret mount は上書きを防ぐが同じ user による読取を防がない。[操作の段階](../quickstart.md#操作の段階)を区別する。[Dockerfile](../../Dockerfile)・[Compose](../../deploy/server/compose.yaml)

## 公開配置と永続化

versioned image、`LVK_HOST`、証明書と鍵を用意し、配置 directory の Compose と secrets を使う。Compose は host 443 を nginx 8443 に公開し、起動前に証明書と設定を検査する。初回はロックされた状態から [server access の初期設定](../concepts/server-access.md) を行う。health が正常でも認証設定や agent 実行成功の証明ではない。[Compose](../../deploy/server/compose.yaml)・[起動](../../deploy/server/server.mjs)

配置 directory で `LVK_IMAGE` に版固定 image、`LVK_HOST` と `LVK_PUBLIC_URL` に公開先を設定し、`secrets` に対応する証明書と鍵を用意する。Compose project と既存 volume を確認し、`docker compose up -d` で起動する。`docker compose ps` と logs、HTTPS を確認してから初回認証へ進む。設定や証明書に問題があれば修正して再確認し、空 volume で代用しない。

三つの named volume は home `/home/appuser`（DB、設定、認証）、repos `/repos`（元 Repo）、work `/var/tmp`（worktree と run 関連）を保持する。Compose project 名変更は別 volume の選択になる。custom mount は別途保護し、再起動が実行中 process を復元すると仮定しない。[Compose](../../deploy/server/compose.yaml)

## 配布イメージの SQLite と補助ツール

セルフホスト用 `server` image は `server-tools` → `server-browser` → `server` の継承で `nano` を含み、最終実行 user は `appuser` である。nano は server-tools の apt 一覧に追加され、ビルド用 stage や別の最終 `runtime` target への追加ではない。廃止済み `scripts/local-runner` の不存在ファイルをコピーする行は削除されている。[Dockerfile](../../Dockerfile)

配布候補を隔離環境で確認するときは、image digest と実行 SHA に加え、通常の `appuser` で `nano --version` を確認する。apt で解決する nano の版を Dockerfile が固定しているわけではない。稼働コンテナーへ手動導入して候補の検査に代えない。

サーバー起動ログの `SQLite engine initialized` は、実際の DB pool から取得した `sqlite_version`、`fts5`、`preupdate_hook`、`unlock_notify` を記録する。LOCAL-495 の期待 engine は SQLite 3.54.1 である。host の `sqlite3 --version` や Python の SQLite 版はこの確認の代わりにならない。[起動処理](../../crates/server/src/main.rs)・[依存と互換試験](development.md#sqlite-と-sqlx-の更新互換性検証)

注意: 起動ログは engine と compile options の確認であり、静的リンクの実バイナリ検査、DB データ保持やバックアップ復元の成功を単独では証明しない。旧版との小規模 fixture が成功しても、新しい schema や SQLite 機能を使った後の downgrade は保証されない。以下のバックアップと復元検証を省略しない。

## FFmpeg による媒体変換と候補の検証

`server` は Debian の公式 `ffmpeg` パッケージを `server-tools` の既存 apt 経路で導入する。利用者が録画の切り出しや形式変換に使う汎用 CLI としての同梱で、LVK 本体の動画 API/UI の追加ではない。別 target の `runtime` と dev バンドルはこの変更の対象外である。[Dockerfile](../../Dockerfile)・[LOCAL-501 の採用時仕様](../../docs/changes/2026-11-25-LOCAL-501-0fd246f8.md)

候補を検証する場合は、Docker を使える隔離環境と候補 image ID を用意する。LOCAL-495 の比較 image と既存 builder/cache を保持し、dev の重いビルドと重ねない。通常の稼働環境や永続 volume を検査に使わない。

1. `server/linux-amd64` の before/after を、同じ context、build args、FROM digest、apt snapshot、npm 解決結果で用意する。before は ffmpeg 追加だけを除く。未公開 SHA を公開 Git から取得する `bundled-source` に渡せない場合の前提と停止条件は[検証手順](../../deploy/server/FFMPEG.md)を参照する。
2. image ID、platform、既定 user、パッケージ版一覧を保存する。追加だけでなく更新・削除も比較し、無関係な差があれば同条件比較として扱わない。非圧縮 image の `.Size` の差を bytes と MiB で記録する。共有 layer を除いたディスク使用量や転送量とは異なる。
3. `AFTER` に候補の不変 image ID を設定し、repository root から次を実行する。既定 user を上書きせず、ネットワーク、host mount、永続 volume、公開 port を使わない。

```bash
docker run --rm -i --network none --read-only \
  --tmpfs /tmp:rw,nosuid,nodev,size=64m --cap-drop ALL \
  --security-opt no-new-privileges --entrypoint /bin/sh "$AFTER" \
  -s < deploy/server/ffmpeg-smoke.sh
```

4. [smoke](../../deploy/server/ffmpeg-smoke.sh) は appuser/UID 10001、ffmpeg/ffprobe の実行、人工 2 秒 FFV1 素材から 1 秒 MPEG-4/MP4 への変換、寸法・長さ・サイズ、全出力の再デコードと SHA-256 読取を確認する。全段階 exit 0 と image ID・ログを対応付ける。失敗時はログを保持して稼働環境へ適用せず、終了した検査の一時素材だけを片付ける。

2026-11-25 の統合依頼では、対象 SHA `3f4d8297127bff5516d2a63ac886c55821c901df` の VPS 隔離実行で appuser の変換・probe・再デコードが成功し、同条件 A/B 増分は **301.13 MiB** と報告された。これは依頼者提供の実測情報であり、この Wiki 更新時に再実行した結果ではない。候補の Dockerfile・smoke・手順が対象 SHA と同一であることを確認して Docker 再実行を省略した。[実装時の記録](../../deploy/server/FFMPEG.md)にある「未実施」は元の coding Workspace 時点の履歴である。

外部配布前には、候補に入る実版の全依存、著作権・ライセンス表示、対応ソースの保存と提供経路・維持責任を別ゲートで確認する。[配布確認項目](../../deploy/server/FFMPEG.md)は未解決事項を含み、CLI の実行成功やローカル main への統合は配布条件の充足を証明しない。

注意:

- apt 解決版は Dockerfile で固定されていないため、後日の build が同じ package set や容量になるとは限らない。
- この smoke は全 codec、GPU、音声、NewsRoom ワークフローを検証しない。
- source 作業時の `server:check` は 34 passed / 1 failed / 8 skipped と記録されている。今回の統合では再実行せず、差分とシェル構文のみを確認した。過去の失敗・未実施を pass に読み替えない。

## Provider の実行アカウントを準備する

provider CLI の login は LVK 認証と別である。コンテナー内の実行 account と同じ永続 home／設定 directory で login と status を行い、その後選択 profile の実行を確認する。別 machine の login を server の認証として扱わない。[provider](../integrations/agent-providers.md)

## 更新前のバックアップと復元検証

1. Agent を完了または停止し、Goal とコンテナー外の実 process を確認してから LVK を停止する。停止の要求だけを終了の証拠にしない。
2. 同じ停止期間に home、repos、work の全 volume と custom mount を保全する。SQLite 本体と存在する WAL／SHM、Git repository／worktree metadata、native session と journal を含める。配置 `.env`、Compose、TLS 等の secrets、ACME volume、access key／marker、provider／Git credentials も対応する組として保存する。秘密を含むため private な暗号化先を使う。[volume](../../deploy/server/compose.yaml)
3. 各 archive の成功、読取・展開可能性と checksum を検証し、Compose project／volume 名、内部 path、image digest、source identity と DB 版を記録する。失敗した archive を復元可能な backup としない。
4. 停止して backup した後は、同じ配置を起動し health、HTTPS、認証、既存データを確認してから通常操作を再開する。更新する場合は選択した image を起動後、ログと小さい agent 実行も確認する。
5. 復元／rollback は対応する旧 image と同時点の全 volume・設定を、同じ内部 path を持つ隔離 recovery deployment へ戻す。元 volume を同時に別 writer に mount しない。DB 整合性、認証、Repo／worktree、会話と小さな agent 実行を確認してから切り替える。migration 済み DB に互換性未確認の旧 binary だけを向けず、確認まで元環境と backup を保持する。[updater の保存と復元](../../deploy/developer/updater.py)

自動 host updater の maintenance gate と rollback は別の管理 transaction である。中断や不明状態はその記録と実環境を確認し、再起動だけで所有権や Goal が解決したと扱わない。[updater](../../deploy/developer/updater.py)
