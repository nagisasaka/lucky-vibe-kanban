---
type: concept
title: Organization・Member・Invitation
description: 固定Organizationと認証actor、承認制メンバー、過去の所有者IDを区別する。
tags: [organization, membership, authorization, invitations, cloud]
sources:
  - id: openwiki-source-20b72970921fabf5b536668c
    resource: repo://crates/server-access/src/handlers.rs
  - id: openwiki-source-9d15c737f5e087237387893e
    resource: repo://crates/server-access/src/store.rs
  - id: openwiki-source-a6cad3d930b299ebbbd793a3
    resource: repo://crates/server-access/src/tests.rs
  - id: openwiki-source-1ffb01d1ba3953937e0771bf
    resource: repo://crates/server/src/routes/local_remote.rs
  - id: openwiki-source-647c795785981f31f6ce0896
    resource: repo://packages/ui/src/components/IssueWorkspacesSection.tsx
  - id: openwiki-source-ab4a5800fcfc16d8494e494a
    resource: repo://packages/web-core/src/shared/lib/localIdentity.ts
generated: { by: "codex", at: "2026-10-16T14:34:12.007Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-24T14:58:13.588Z
---

# Organization・Member・Invitation

CloudサービスのOrganization/Invitationは撤去されている。local Boardは固定Organization `Uuid::from_u128(2)` を引き続き使うが、公開配置の本人識別は新しいserver-accessの認証に基づく。[Board API](../../crates/server/src/routes/local_remote.rs)、[認証](../../crates/server-access/src/lib.rs)。

Localモードのuserは `Uuid::from_u128(1)`。Basic/Auth0では認証されたactorのIDを使用し、UIもaccess gateが取得した本人IDを用いる。過去のWorkspaceの固定所有者IDを現在の利用者へ書き換えず、共有Workspaceとして表示・操作する。[local identity](../../packages/web-core/src/shared/lib/localIdentity.ts)、[Workspace UI](../../packages/ui/src/components/IssueWorkspaceCard.tsx)。

Auth0の本人はissuerとsubjectで識別し、外部ログインしただけの利用者は承認待ちとなる。管理者が「認証・メンバー」で承認し、最後の有効な管理者は無効化・降格できない。全有効メンバーが全プロジェクトを使う。旧Cloudの招待・組織単位の隔離・同一メールによるアカウント統合は復活しない。[設定とメンバー管理](../../docs/self-hosting/authentication.mdx)、[handlers](../../crates/server-access/src/handlers.rs)。

Boardのメンバー表示は現在actorのissuerに属するactiveメンバーを読み、Organizationのroleもactorから返す。users snapshotには過去の固定IDを「Legacy local user (unattributed)」として残す。固定Organizationはtenant境界や全員Adminの証明ではない。[表示の組立て](../../crates/server/src/routes/local_remote.rs#L1228-L1358)

メンバー無効化・session失効は新規API要求と継続接続の認証を失わせる。WebSocketは共通LocalWsUpgradeで認証leaseを保持し、失効後に終了する。既に受理したagent作業を自動取消する契約ではない。同じOSユーザーとしてのファイル・エージェント権限は引き続き共有される。[WebSocket](../../crates/server/src/middleware/websocket.rs)、[運用境界](../operations/server-container.md)。

実Auth0・ChatGPTアカウントでの受入確認はコンポーネント検証と区別する。[検証範囲](../../docs/development/server-access.md)。

設定の初回有効化、session寿命、暗号化・復旧と内部agentの権限は [サーバー認証](server-access.md)に集約する。メンバーの承認はagent providerへのログインを代行しない。
