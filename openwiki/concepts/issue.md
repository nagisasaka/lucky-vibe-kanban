---
type: concept
title: Issue
description: Project の仕事、本文版と実施の関係。
tags: [issue, project, kanban]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-9bf50baebc6a9d4f59193172
    resource: repo://crates/db/migrations/20261111220000_issue_versions.sql
  - id: openwiki-source-6fe5fbd6df406f973641b771
    resource: repo://crates/db/src/models/issue_number.rs
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# Issue

Issue は [Project](project.md) の仕事であり、本文に仕様と人間の意図を記録する。[Kanban board](kanban-board.md) には [Card](card.md) として現れ、一つの Issue に複数の [Workspace](workspace.md) を関連付けられる。[操作の段階](../quickstart.md#操作の段階)

Issue は UUID を持ち、表示番号は Project key と高水位番号から作る。commit 済み番号は Issue を削除しても戻らない。本文には名前付きの版があり、現在版は将来の起動や統合受付に用いる。版の切替は既に送信した入力を変えない。詳しくは [Project と Issue](project-and-issue.md) を参照する。

親子・関連・blocking、コメントと添付は [Issue の共同作業](issue-collaboration.md) にある。archive と削除は別操作である。[番号](../../crates/db/src/models/issue_number.rs)・[本文版](../../crates/db/migrations/20261111220000_issue_versions.sql)
