---
type: overview
title: クイックスタートと調査案内
description: LVK の目的、主要概念と調査先。
tags: [overview, quickstart]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-22T18:59:30.190Z
sources:
  - id: openwiki-source-23775c3de52f3ab95a13cb8b
    resource: repo://README.md
generated: { by: "codex", at: "2026-11-22T18:59:30.190Z" }
---

# クイックスタートと調査案内

Lucky Vibe Kanban（LVK）は、カンバンで仕事を計画し、coding agent を実行し、その変更をレビューする self-hosted 開発環境である。複数 Session、Git worktree、Workflow、local Integration と OpenWiki を備える。[README](../README.md)・[システム構成](architecture/system.md)

まず [サーバー配置と復元](operations/server-container.md) と [初回認証](concepts/server-access.md) を確認し、同じ実行環境で [provider を準備](integrations/agent-providers.md)する。source 開発は [開発と検証](operations/development.md) を参照する。

[Project と Issue](concepts/project-and-issue.md) は要求と整理、[Workspace](concepts/workspace.md) は作業場所、[Session と Agent Run](concepts/session-and-agent-run.md) は会話と実行の単位である。通常は単一 Agent を開始し、複数工程の引渡しには [Workflow preset](concepts/workflow-attempt.md) を選ぶ。具体的な順序は [Issue から統合まで](workflows/task-to-integration.md) にある。

## 操作の段階

| 段階 | 確認すること |
| --- | --- |
| 受付 | 対象、権限、固定 source と request ID。受付は実行完了ではない |
| 実行 | Run、質問・承認、停止と実 process の状態 |
| 成果と検証 | ファイル、固定 commit、検証結果と未確認事項 |
| 統合と公開 | [正式 Integration](concepts/formal-integration.md) や [PR](integrations/github-review.md) の実結果。要求受付や Agent 成功とは別 |
| 知識と保全 | [Repository Memory](concepts/repository-memory.md) と [Wiki 保守](operations/openwiki-maintenance.md) の結果、archive と backup の状態 |

## 調査先

- 概念の関係は [用語集](concepts/glossary.md)、実行と回復は [Agent Runtime](architecture/agent-runtime.md) を読む。
- ファイル・差分・実行状態は [Workspace の確認](operations/workspace-inspection.md)、外部接続と receipt は [MCP](integrations/remote-access.md) を参照する。
- 実機や DB の並行操作では、worktree の分離だけで排他が成立すると考えず、必要に応じて環境や作業順序を分ける。
- 会話の質問は [対話ヘルプ](concepts/interactive-help.md)、設計意図は [設計メモ](concepts/design-notes.md)、小さなメモは [Project Todo](concepts/project-todos.md) を参照する。
