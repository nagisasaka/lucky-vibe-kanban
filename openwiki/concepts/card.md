---
type: concept
title: Card
description: Kanban board 上の Issue と表示状態。
tags: [card, issue, kanban]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-4b75804f8a742afa505de60d
    resource: repo://crates/server/src/routes/local_remote/board_api/issues.rs
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# Card

Card は [Kanban board](kanban-board.md) に現れる [Issue](issue.md) の項目である。画面上の移動と仕事の状態を理解するときに読む。[操作の段階](../quickstart.md#操作の段階)も参照する。

Card の列は Project の status を表す。status と Agent Run の状態は別であり、Agent の成功だけで Done にはならない。自動移動と手動変更の優先関係は [Project と Issue](project-and-issue.md#board-の状態と実行) を参照する。

Board API は Project に属する Issue を取得し、archive 属性で絞り込める。[Issue API](../../crates/server/src/routes/local_remote/board_api/issues.rs)
