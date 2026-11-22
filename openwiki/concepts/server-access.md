---
type: guide
title: サーバー認証・設定・回復
description: Local/Public 配置、初回認証、session、閉鎖状態と限定された operator 回復を説明する。
tags: [authentication, authorization, sessions, security, recovery]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-22T05:31:45.518Z
sources:
  - id: openwiki-source-20b72970921fabf5b536668c
    resource: repo://crates/server-access/src/handlers.rs
  - id: openwiki-source-7d94e7454f63f16057833046
    resource: repo://crates/server-access/src/lib.rs
  - id: openwiki-source-9d15c737f5e087237387893e
    resource: repo://crates/server-access/src/store.rs
  - id: openwiki-source-a13fe4db1eee073d0a7e2c4d
    resource: repo://crates/server/src/main.rs
generated: { by: "codex", at: "2026-11-22T05:31:45.518Z" }
---

# サーバー認証・設定・回復

`server-access` は信頼されたチームが LVK に入るための境界である。LVK ログインは coding provider や独立アプリのログイン、Workspace の実行予約、OS 隔離とは別である。承認済み member は同じ実行環境を共有し、Project ごとの tenant 隔離はない。[access](../../crates/server-access/src/lib.rs)・[Member](organization-and-membership.md)・[操作の段階](../quickstart.md#操作の段階)

## 配置と認証

| 配置 | 条件 |
| --- | --- |
| Local | 未設定時に Local mode を使い、HTTP Host は loopback、Origin があれば一致を要求する |
| Public | canonical HTTPS origin の `LVK_PUBLIC_URL` が必要。初期状態は locked で、Basic または Auth0 を設定する。Local mode は許可しない |

`LVK_ACCESS_DEPLOYMENT` 未指定で非 loopback に bind する設定は拒否する。`LVK_AUTH_MODE` は許す方式を制限するが資格情報を作らない。公開 URL は path・query・userinfo 等を持たず、Host と Origin を照合する。[配置検査](../../crates/server-access/src/lib.rs)

Web cookie は HttpOnly、SameSite=Lax、公開配置では Secure で、session は絶対12時間・idle30分である。変更要求は CSRF を要する。外部 MCP は Auth0 bearer JWT と scope を使い、Web cookie や Basic で代用しない。内部 stdio は owner-only token と loopback Host・Origin なしを必要とし、認証設定管理者にはならない。[access](../../crates/server-access/src/lib.rs)・[provider](../../crates/server-access/src/provider.rs)・[外部 MCP](../integrations/remote-access.md)

失効や member 無効化は新規要求と継続接続の lease に影響するが、既に受理された agent 作業の自動取消を意味しない。Auth0 refresh の取消と LVK member の無効化も別であり、provider 障害時に Basic / Local へ自動 fallback しない。[lease](../../crates/server-access/src/lib.rs)・[Member](organization-and-membership.md)

## 初回設定

1. 対象 deployment と同じ data directory、公開 HTTPS、operator 権限を確認する。独立 instance では CLI と server に同じ `LVK_INSTANCE_DIR` を指定する。
2. 標準 Compose では `docker compose exec lvk server setup-token` を発行し、HTTPS の設定画面で15分以内に一度だけ交換する。キーを URL、チャットや通常ログへ残さない。CLI は listener や agent を起動しない。
3. Basic はユーザー名と12〜1024 byte の password を設定し、切替後に再ログインする。Auth0 は discovery の後、実ログイン試験と本人確認を経て activate する。MCP client と Web client は分け、resource と audience を一致させる。
4. 新しい方式でログインし、権限と拒否の両方を確認する。設定切替は期待 revision と管理権限を再確認し、旧 session・setup key・transaction を無効化する。[CLI](../../crates/server/src/main.rs)・[発行と切替](../../crates/server-access/src/store.rs)・[handler](../../crates/server-access/src/handlers.rs)

Auth0 作成支援は Preview と Apply を別にし、確認後に外部 plan が変わると再 Preview を要する。部分失敗は Auth0 側を自動 rollback せず、現在の plan を再確認する。作成支援の成功自体は LVK 認証の activate や実 MCP 接続成功ではない。[provisioning](../../crates/server-access/src/provisioning.rs)

## 閉鎖と回復

設定と OIDC 中間情報は AES-256-GCM で暗号化され、32 byte の `access.key` は DB と分離する。Unix の鍵は owner-only、通常 file である必要がある。`access.configured` は設定履歴の marker である。**DB、鍵、marker を同時点の整合した組として保全する。** 鍵欠損や破損設定、marker があるのに設定 row がない状態を未設定 Local へ戻さず、閉鎖する。[鍵と load](../../crates/server-access/src/store.rs)

回復 session は認証設定の修正専用であり、Kanban、file、member 一覧や MCP を無条件に開かない。元の鍵がなければ旧暗号化設定や同じ鍵で暗号化された secret は復号できず、再登録が必要である。[回復 tests](../../crates/server-access/src/tests.rs)・[ChatGPT secret](../../crates/server-access/src/chatgpt.rs)

### Operator の回復手順

1. 対象の配置 directory、Compose project・ファイル、image digest / binary、volume と設定を記録する。新規 agent・Runner 作業を始めず、実 process と共有資源の収束を確認する。
2. 同じ deployment を停止し、DB と存在する journal、`access.key`、`access.configured`、設定・secret・証明書および home/repos/work 等の全 volume を同じ時点の組として保全する。[停止 backup](../operations/server-container.md#更新前のバックアップと復元検証)の archive 読取と hash を検証し、失敗したら先へ進まない。
3. 同じ image/binary、project、volume と設定で既存 deployment を `docker compose start` で戻し、`docker compose ps` と logs を確認する。新しい image や空 volume を作らない。既存 service や対応関係が不明なら停止して調査する。
4. `docker compose exec lvk node /opt/lvk-server/healthcheck.mjs` の成功と、既存 HTTPS URL の `/?access=recovery` で証明書検証・復旧画面を確認する。health は認証修復の成功ではなく、業務 API の閉鎖を解除する根拠でもない。
5. 同じ対象で `docker compose exec lvk server recovery-token` を発行し、確認済み HTTPS 画面で15分以内に交換する。許可された Basic を再設定するか、Auth0 の実ログイン試験・本人確認・activate を行う。新方式で再ログインし、権限と拒否を確認してから既存 Goal・定期実行・資源所有を照合し、必要な作業だけを再開する。[CLI](../../crates/server/src/main.rs)・[token](../../crates/server-access/src/store.rs)・[healthcheck](../../deploy/server/healthcheck.mjs)

正しい元鍵があるなら、現在の組を退避し対象停止後に対応 DB と元鍵を復元することを検討する。鍵が不正で CLI が拒否する場合は、元ファイルと DB を保全し、不正鍵を退避した後に同じ data directory で recovery token を発行する。CLI は存在する不正鍵を上書きせず、鍵がない場合だけ新規生成する。新鍵は旧設定を復号しない。業務 DB や marker を削除して未設定扱いへ戻さない。[鍵生成](../../crates/server-access/src/store.rs)

認証 fixture は閉鎖、失効と限定回復を検査する。実 Auth0 tenant、外部 client、公開 TLS や稼働環境の復元は別に検証する。[tests](../../crates/server-access/src/tests.rs)
