---
type: concept
title: Column
description: Project の Kanban board を状態ごとに分ける列。
tags: [kanban, status, project]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-3f1917605488d5ec8c09afc2
    resource: repo://crates/server/src/routes/local_remote/board_api/project_statuses.rs
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# Column

Column は [Kanban board](kanban-board.md) を仕事の状態ごとに分ける列である。Project ごとに状態を管理する。ボード上の状態と実際の処理を区別するときに読む。[操作の段階](../quickstart.md#操作の段階)

列の status は process の状態そのものではない。実開始や成功に伴う自動移動、手動変更との優先関係は [Project と Issue](project-and-issue.md#board-の状態と実行) を参照する。隠れた列や独自の列がある場合も、画面の状態だけから実行の成否を推定しない。

Board API は Project ID を指定して status を取得する。[status API](../../crates/server/src/routes/local_remote/board_api/project_statuses.rs)
