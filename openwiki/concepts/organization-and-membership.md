---
type: concept
title: Organization・Member・Invitation
description: local Board の固定 Organization と user の互換表現、および認証・認可との境界。
tags: [organization, membership, authorization, invitations, cloud]
sources:
  - id: openwiki-source-651d1fb6c9e49916a916ab51
    resource: repo://Cargo.toml
  - id: openwiki-source-0efe022da63bfbe33cd95498
    resource: repo://crates/server/src/remote_mcp/auth.rs
  - id: openwiki-source-1ffb01d1ba3953937e0771bf
    resource: repo://crates/server/src/routes/local_remote.rs
generated: { by: "codex", at: "2026-10-16T12:21:28.631Z" }
---

# Organization・Member・Invitation

CloudサービスのOrganization/Invitationは撤去されている。local Boardは固定Organization `Uuid::from_u128(2)` を引き続き使うが、公開配置の本人識別は新しいserver-accessの認証に基づく。[Board API](../../crates/server/src/routes/local_remote.rs)、[認証](../../crates/server-access/src/lib.rs)。

Localモードのuserは `Uuid::from_u128(1)`。Basic/Auth0では認証されたactorのIDを使用し、UIもaccess gateが取得した本人IDを用いる。過去のWorkspaceの固定所有者IDを現在の利用者へ書き換えず、共有Workspaceとして表示・操作する。[local identity](../../packages/web-core/src/shared/lib/localIdentity.ts)、[Workspace UI](../../packages/ui/src/components/IssueWorkspaceCard.tsx)。

Auth0の本人はissuerとsubjectで識別し、外部ログインしただけの利用者は承認待ちとなる。管理者が「認証・メンバー」で承認し、最後の有効な管理者は無効化・降格できない。全有効メンバーが全プロジェクトを使う。旧Cloudの招待・組織単位の隔離・同一メールによるアカウント統合は復活しない。[設定とメンバー管理](../../docs/self-hosting/authentication.mdx)、[handlers](../../crates/server-access/src/handlers.rs)。

メンバー無効化・session失効は新規API要求と継続接続の認証を失わせる。WebSocketは共通LocalWsUpgradeで認証leaseを保持し、失効後に終了する。既に受理したagent作業を自動取消する契約ではない。同じOSユーザーとしてのファイル・エージェント権限は引き続き共有される。[WebSocket](../../crates/server/src/middleware/websocket.rs)、[運用境界](../operations/server-container.md)。

実Auth0・ChatGPTアカウントでの受入確認はコンポーネント検証と区別する。[検証範囲](../../docs/development/server-access.md)。
