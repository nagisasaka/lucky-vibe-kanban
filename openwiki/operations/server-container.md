---
type: operation
title: サーバーコンテナー運用
description: HTTPS 配置、永続化、バックアップと復元。
tags: [server, docker, backup, recovery]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-22T18:59:30.190Z
sources:
  - id: openwiki-source-df6b7819295f53fc2381f23d
    resource: repo://deploy/server/compose.yaml
generated: { by: "codex", at: "2026-11-22T18:59:30.190Z" }
---

# サーバーコンテナー運用

標準 server image は local LVK、agent と nginx をサーバーで動かし、browser に HTTPS を提供する。認証は利用者ごとの OS 隔離ではなく、agent と server は同じ実行 user を使う。read-only secret mount は上書きを防ぐが同じ user による読取を防がない。[Dockerfile](../../Dockerfile)・[Compose](../../deploy/server/compose.yaml)

## 公開配置と永続化

versioned image、`LVK_HOST`、証明書と鍵を用意し、配置 directory の Compose と secrets を使う。Compose は host 443 を nginx 8443 に公開し、起動前に証明書と設定を検査する。初回はロックされた状態から [server access の初期設定](../concepts/server-access.md) を行う。health が正常でも認証設定や agent 実行成功の証明ではない。[Compose](../../deploy/server/compose.yaml)・[起動](../../deploy/server/server.mjs)・[healthcheck](../../deploy/server/healthcheck.mjs)

三つの named volume は home `/home/appuser`（DB、設定、認証）、repos `/repos`（元 Repo）、work `/var/tmp`（worktree と run 関連）を保持する。Compose project 名変更は別 volume の選択になる。custom mount は別途保護し、再起動が実行中 process を復元すると仮定しない。[Compose](../../deploy/server/compose.yaml)

## Provider の実行アカウントを準備する

provider CLI の login は LVK 認証と別である。コンテナー内の実行 account と同じ永続 home／設定 directory で login と status を行い、その後選択 profile の実行を確認する。別 machine の login を server の認証として扱わない。[provider](../integrations/agent-providers.md)

## 更新前のバックアップと復元検証

1. Agent を完了または停止し、Goal とコンテナー外の実 process を確認してから LVK を停止する。停止の要求だけを終了の証拠にしない。
2. 同じ停止期間に home、repos、work の全 volume と custom mount を保全する。SQLite 本体と存在する WAL／SHM、Git repository／worktree metadata、native session と journal を含める。配置 `.env`、Compose、TLS 等の secrets、ACME volume、access key／marker、provider／Git credentials も対応する組として保存する。秘密を含むため private な暗号化先を使う。[volume](../../deploy/server/compose.yaml)
3. 各 archive の成功、読取・展開可能性と checksum を検証し、Compose project／volume 名、内部 path、image digest、source identity と DB 版を記録する。失敗した archive を復元可能な backup としない。
4. 停止して backup した後は、同じ配置を起動し health、HTTPS、認証、既存データを確認してから通常操作を再開する。更新する場合は選択した image を起動後、ログと小さい agent 実行も確認する。
5. 復元／rollback は対応する旧 image と同時点の全 volume・設定を、同じ内部 path を持つ隔離 recovery deployment へ戻す。元 volume を同時に別 writer に mount しない。DB 整合性、認証、Repo／worktree、会話と小さな agent 実行を確認してから切り替える。migration 済み DB に互換性未確認の旧 binary だけを向けず、確認まで元環境と backup を保持する。[updater の保存と復元](../../deploy/developer/updater.py)

自動 host updater の maintenance gate と rollback は別の管理 transaction である。中断や不明状態はその記録と実環境を確認し、再起動だけで所有権や Goal が解決したと扱わない。[updater](../../deploy/developer/updater.py)
