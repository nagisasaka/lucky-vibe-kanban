---
type: concept
title: Kanban board
description: Project の Issue を状態別に表示するボード。
tags: [kanban, board, project]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-699012344cbfd472d165f098
    resource: repo://crates/server/src/routes/local_remote/board_api/mod.rs
  - id: openwiki-source-92cb09ac7279f8310b0dd21e
    resource: repo://packages/local-web/src/routes/_app.projects.%24projectId.tsx
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# Kanban board

Kanban board は [Project](project.md) の [Issue](issue.md) を [Column](column.md) に並べる主画面である。仕事を選び、状態と順序を確認するときに使う。[操作の段階](../quickstart.md#操作の段階)

[Card](card.md) の移動は共有 Issue の状態や順位を変更する。表示 filter と実行状態は別であり、表示列だけから処理の完了や統合を判定しない。自動状態同期や archive の詳細は [Project と Issue](project-and-issue.md) にある。

Project の route は Kanban 画面を開き、Board API はその Project の Issue と status を取得する。[画面入口](../../packages/local-web/src/routes/_app.projects.$projectId.tsx)・[Board API](../../crates/server/src/routes/local_remote/board_api/mod.rs)
