---
type: concept
title: 上位AI
description: MCP から仕様相談、委任と進捗確認を行う AI の役割。
tags: [orchestration, mcp, agent]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-71f837a8aa36443ad9688945
    resource: repo://crates/server/src/remote_mcp/mod.rs
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# 上位AI

上位AI は人と仕様を相談し、MCP を通して Issue の作成、[coding agent](coding-agent.md) への委任や進捗確認を行う。Workspace 内で実際に作業する agent とは会話と責任が異なる。[操作の段階](../quickstart.md#操作の段階)

MCP の受付は実行や公開の成功ではない。対象の状態と receipt を確認し、結果不明の要求を別 ID で重複実行しない。接続、権限、receipt の契約は [外部 MCP](../integrations/remote-access.md) を参照する。[MCP server](../../crates/server/src/remote_mcp/mod.rs)
