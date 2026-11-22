---
type: concept
title: Organization と Member
description: local Board の固定 Organization、現在 actor、承認制メンバーと過去 identity の境界。
tags: [organization, membership, authorization]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-22T05:31:45.518Z
sources:
  - id: openwiki-source-20b72970921fabf5b536668c
    resource: repo://crates/server-access/src/handlers.rs
  - id: openwiki-source-9d15c737f5e087237387893e
    resource: repo://crates/server-access/src/store.rs
  - id: openwiki-source-1ffb01d1ba3953937e0771bf
    resource: repo://crates/server/src/routes/local_remote.rs
generated: { by: "codex", at: "2026-11-22T05:31:45.518Z" }
---

# Organization と Member

LVK の local Board は固定 Organization を使い、公開配置の本人は [サーバー認証](server-access.md) の actor で識別する。所属、権限、過去の所有者表示を変更するときに読む。認証の受付と作業の完了は[別の段階](../quickstart.md#操作の段階)である。[Board API](../../crates/server/src/routes/local_remote.rs)・[access](../../crates/server-access/src/lib.rs)

Local mode の user は固定 local identity である。Basic と Auth0 は認証済み actor の ID を使い、UI も access gate が得た本人 ID を使う。過去の Workspace の固定所有者を現在の利用者へ書き換えない。[local identity](../../packages/web-core/src/shared/lib/localIdentity.ts)・[access](../../crates/server-access/src/lib.rs)

Auth0 の本人は issuer と subject で識別する。初回ログインだけでは active にならず、管理者の承認を待つ。最後の active admin は無効化・降格できない。固定 Organization は tenant 分離や全員が admin である証明ではない。[identity](../../crates/server-access/src/store.rs)・[member 管理](../../crates/server-access/src/handlers.rs)

Board の member 表示は現在 actor の issuer に属する active member を読む。過去の固定 ID は帰属不明の legacy user として残す。旧 Cloud の Invitation、組織単位の隔離や同じメールによるアカウント統合を現行機能と扱わない。[表示](../../crates/server/src/routes/local_remote.rs)

Member の無効化や session 失効は新規要求と継続接続の認証に影響する。すでに受理された agent 作業の自動取消を意味せず、同じ OS 実行ユーザーのファイル権限も別である。[WebSocket](../../crates/server/src/middleware/websocket.rs)・[運用境界](../operations/server-container.md)

実 Auth0・外部 client の受入は fixture の成功と別に確認する。初期設定と回復、provider ログインの違いは[サーバー認証](server-access.md)と[provider](../integrations/agent-providers.md)を参照する。[access tests](../../crates/server-access/src/tests.rs)
