---
type: concept
title: Arena の履歴と互換性
description: 旧 Arena の候補、選択と Workspace の寿命、および Workflow の winner 適用を説明する。
tags: [arena, workspace, comparison, lifecycle]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-22T05:31:45.518Z
sources:
  - id: openwiki-source-2a51fb8409723c3914db4954
    resource: repo://crates/db/src/models/arena_group.rs
  - id: openwiki-source-1ffb01d1ba3953937e0771bf
    resource: repo://crates/server/src/routes/local_remote.rs
  - id: openwiki-source-13bc408495036974093d65eb
    resource: repo://crates/server/src/workflow_runtime/arena.rs
generated: { by: "codex", at: "2026-11-22T05:31:45.518Z" }
---

# Arena の履歴と互換性

Arena は一つの [Issue](project-and-issue.md) に対して複数の [Workspace](workspace.md) を候補として持つ旧比較機能である。既存の group や Run を調査するときに読む。新規 Arena 作成入口は guard で拒否され、現在の開始は単一 Agent または[固定 Workflow preset](workflow-attempt.md) を使う。旧 group や winner 適用 runtime が一括削除されたという意味ではない。[入口](../../crates/server/src/routes/local_remote.rs)・[モデル](../../crates/db/src/models/arena_group.rs)

## 候補と状態

Design は Workspace の会話で方案を比較する既定 mode であり、Open 中は LVK の既定 commit 方針を抑制する。Implementation は差分と promote を使う mode である。この抑制は provider の任意操作を禁止する sandbox ではない。[mode](../../crates/db/src/models/arena_group.rs)・[commit policy](../../crates/local-deployment/src/container.rs)

| 状態 | 意味 |
| --- | --- |
| Group lifecycle | `open`、`closed`、`adopted`、`implementation_started` |
| 候補の Arena status | `active`、`promoted`、`archived` |
| Workspace の通常 archive と Agent Run | 保存場所や実行の状態。Arena status とは独立 |

Arena の `archived` は落選や Retry で置き換えられた候補を示し、通常の Workspace archive や物理削除を意味しない。Closed group は promote されていなくても active group の検索対象から外れる。[状態定義](../../crates/db/src/models/arena_group.rs)

## 互換操作と部分失敗

既存 group の message は全候補、特定 Workspace、challenge、synthesis を扱う。Implementation 開始は既存候補を実装対象として記録する操作であり、新しい worktree 作成や merge と同義ではない。通常の Promote は winner と他候補の Arena status を更新し、Git merge や worktree の同期削除は行わない。選択後のレビューと統合は[通常の作業フロー](../workflows/task-to-integration.md)に従う。[操作](../../crates/server/src/routes/local_remote.rs)

Retry は候補を Arena 内で archive し、同じ Repo 群を持つ新候補を作る。これは同一 Agent Run の Attempt retry とは別である。Close は group を閉じ、Dissolve は未 promote group の候補の Arena status を更新して group を削除する。どちらも process 終了や通常 Workspace の削除を証明しない。[操作](../../crates/server/src/routes/local_remote.rs)・[Run の retry](session-and-agent-run.md)

候補作成は複数 Workspace の一括 transaction ではない。途中失敗時の cleanup 自体も失敗し得るため、残存 Workspace と実行状態を確認する。Issue archive は選択状態と独立するが、新しい候補の作成や Retry は archive 済み Issue を拒否する。[handler](../../crates/server/src/routes/local_remote.rs)

## Workflow の winner

旧 Workflow の互換 runtime では winner の base からの patch を主 Workspace へ適用する。通常 Arena の Promote と異なり、これはコードを反映する操作である。複数 Repo は順次適用され、途中失敗時に先行分を一括 rollback する transaction はない。固定 preset の新規 graph に Arena node は含まれない。[適用](../../crates/server/src/workflow_runtime/arena.rs)・[固定 graph](../../crates/workflow/src/presets.rs)

モデルの状態試験と handler の検証は既存 group の契約を調べる入口であり、新規作成 UI や実 provider の全操作を保証しない。[モデル試験](../../crates/db/src/models/arena_group.rs)
