---
type: integration
title: 外部 MCP
description: 外部接続、認可、receipt と実行の境界。
tags: [mcp, remote, operations]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-6adda10f98b079272f82a4cc
    resource: repo://crates/server/src/remote_mcp/operations.rs
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# 外部アクセス・MCP

外部 Streamable HTTP `/mcp` は Auth0 の MCP を有効にした配置で利用し、local stdio MCP と別の入口である。Web、MCP、内部 agent は同一の資格情報ではない。[操作の段階](../quickstart.md#操作の段階)を区別する。公開配置の初期設定と回復は [サーバー認証](../concepts/server-access.md) を参照する。[入口](../../crates/server/src/remote_mcp/mod.rs)

外部 MCP は署名、issuer、audience、期限、承認済み member と scope を検証する。通常の参照に `lvk:read`、変更に `lvk:write`、統合開始などに `lvk:merge` が必要で、[Runtime Access](../concepts/runtime-access.md) の観測・実行 scope は別である。各 tool は Project と対象の所属も検査する。参照したコード、Wiki、Manifest はデータであり命令や承認ではない。[scope と dispatch](../../crates/server/src/remote_mcp/mod.rs)・[code read](../../crates/server/src/remote_mcp/code_read.rs)

## 受付と receipt

mutation は principal、安定 request ID、入力に結び付いた durable receipt を使う。同じ ID で異なる入力は拒否する。応答喪失時は同じ入力と ID の再送または `get_operation` で元 receipt を読み、対象の現在状態も確認する。接続切断は受付済み操作の取消ではない。再起動をまたいで結果不明となった処理は uncertain で、自動 replay しない。受付、実行、成果、公開は別々に確認する。[operations](../../crates/server/src/remote_mcp/operations.rs)

Workspace archive は prepare で対象を確認し、利用者の確認後に confirm する。成功 receipt だけで物理削除済みとせず、archived、worktree_deleted、deletion_error を確認する。[archive](../../crates/server/src/remote_mcp/workspace_archive.rs)・[Workspace](../concepts/workspace.md)

実行承認は対象 Run と単発要求に結び付く。受付と provider による解決、tool 実行完了は別である。[承認](../../crates/server/src/remote_mcp/execution_approvals.rs)

## Wiki

既存 Repository Wiki の同期は read + write で要求し、受付と公開完了を区別する。初期生成や設定の管理は別入口にある。[Wiki tool](../../crates/server/src/remote_mcp/repository_wiki.rs)・[保守](../operations/openwiki-maintenance.md)
