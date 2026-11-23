---
type: concept
title: Project
description: LVK の開発作業、状態と Repository を整理する単位。
tags: [project, issue, repository]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-bdb4dae5722fe76ec5e44001
    resource: repo://crates/server/src/routes/workspaces/create.rs
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# Project

Project は LVK で開発作業を整理する大きな単位である。[Issue](issue.md)、[Kanban board](kanban-board.md)、[TODO list](project-todos.md) と [Notes](design-notes.md) を持ち、複数の Git Repository を関連付けられる。[操作の段階](../quickstart.md#操作の段階)

Project の Repository 群は新規 [Workspace](workspace.md) の作成プリセットである。既存 Workspace の Repository や branch を後から暗黙に変更しない。作業と状態の詳細は [Project と Issue](project-and-issue.md) を参照する。[作成](../../crates/server/src/routes/workspaces/create.rs)
