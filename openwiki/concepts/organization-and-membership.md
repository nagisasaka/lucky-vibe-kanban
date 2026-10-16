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
verified:
  - by: openwiki/0.5.1
    at: 2026-10-16T12:21:28.631Z
---

# Organization・Member・Invitation

現行の Organization / Member は、local Board が既存の Project・担当者 UI に渡す固定の互換表現である。複数人のチームや招待・権限移譲を管理する Cloud サービスは現行構成から削除されている。旧 personal organization の作成や Invitation の7日期限・メール受理・最後の Admin 保護を現在の契約として適用しない。[local API](../../crates/server/src/routes/local_remote.rs)、[crate 構成](../../Cargo.toml)。

## local の識別子と関係

local user は `Uuid::from_u128(1)`、organization は `Uuid::from_u128(2)`。組織一覧と member 一覧は固定 Local / Admin 表現を返す。Project metadata は local organization に結び付けられ、一覧もその ID で絞る。fallback の organization member / user metadata は対象 organization が一致しなければ空になる。[ID](../../crates/server/src/routes/local_remote.rs#L266-L272)、[Project metadata](../../crates/server/src/routes/local_remote.rs#L327-L406)、[一覧](../../crates/server/src/routes/local_remote.rs#L1228-L1293)。

この Admin は外部利用者のログイン証明でも、Repo や OS の権限付与でもない。仕事の分類は [Project・Issue](project-and-issue.md)、担当と Follower の保存は [Issue の関連](issue-collaboration.md#担当購読コメント)に説明する。固定 user の存在から多人数の認可を推定しない。

## 認証を変更する際の境界

通常の self-host UI は信頼された単一利用者を前提にし、公開時は運用上の HTTPS とアクセス制御が必要である。[製品の公開条件](../../README.md)。外部 MCP は独立した bearer introspection・owner subject・resource audience・project allowlist を検証する入口であり、この固定 Admin をその認証に代用しない。[外部アクセス](../integrations/remote-access.md)。

旧 Cloud の membership や Invitation と同名の型が残っていても、実行可能な router と保存モデルを確認する。新しい多人数認可や Auth0 連携はこの checkout から現行仕様として確定できず、その設計意図を実装済みの保証にしない。
