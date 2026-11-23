---
type: overview
title: クイックスタートと調査案内
description: LVK の目的、主要概念、操作段階と調査先。
tags: [overview, quickstart]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-23775c3de52f3ab95a13cb8b
    resource: repo://README.md
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# クイックスタートと調査案内

Lucky Vibe Kanban（LVK）は Kanban で仕事を計画し、coding agent を実行し、その変更をレビューする self-hosted 開発環境である。複数 Session、Git worktree、Workflow、local Integration と OpenWiki を備える。[README](../README.md)・[システム構成](architecture/system.md)

まず [サーバー配置と復元](operations/server-container.md) と [初回認証](concepts/server-access.md) を確認し、同じ実行環境で [provider を準備](integrations/agent-providers.md)する。source 開発は [開発と検証](operations/development.md) を参照する。

[Project と Issue](concepts/project-and-issue.md) は要求と整理、[Workspace](concepts/workspace.md) は作業場所、[Session と Agent Run](concepts/session-and-agent-run.md) は会話と実行の単位である。通常は単一 Agent を開始し、複数工程には [Workflow preset](concepts/workflow-attempt.md) を選ぶ。順序は [Issue から統合まで](workflows/task-to-integration.md) にある。

## 操作の段階

| 段階 | 確認すること |
| --- | --- |
| 受付 | 対象、権限、固定 source と request ID。受付は実行完了ではない |
| 実行 | Run、質問・承認、停止と実 process の状態 |
| 完了 | 成果、固定 commit、検証結果と未確認事項。Agent の終了だけでは検証を証明しない |
| 公開 | [Auto-merge](concepts/formal-integration.md) や [PR](integrations/github-review.md) の実際の Git 結果。公開要求とは別 |
| 内容受入 | 成果や Wiki が目的と根拠を満たすかの判断。receipt や公開だけでは保証されない |

## 調査先

- 概念の関係は [用語集](concepts/glossary.md)、実行と回復は [Agent Runtime](architecture/agent-runtime.md) を読む。
- ファイル・差分・実行状態は [Workspace の確認](operations/workspace-inspection.md)、外部接続と receipt は [外部 MCP](integrations/remote-access.md) を参照する。
- 設計意図は [Notes](concepts/design-notes.md)、共有メモは [TODO list](concepts/project-todos.md)、一般質問は [対話ヘルプ](concepts/interactive-help.md) にある。
- 知識の鮮度と保全は [Repository Memory](concepts/repository-memory.md)、[Wiki 保守](operations/openwiki-maintenance.md)、[バックアップ](operations/server-container.md#更新前のバックアップと復元検証)を参照する。worktree の分離だけで実機や DB の排他が成立するわけではない。
