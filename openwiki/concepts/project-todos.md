---
type: concept
title: Project Todo の共有メモ
description: Project に保存する Todo と Issue・実行の違い、Web と二つの MCP の共有データ、競合・再送・表示の契約。
tags: [todos, project, persistence, mcp, concurrency]
verified:
  - by: openwiki/0.5.1
    at: 2026-10-21T03:28:49.332Z
sources:
  - id: openwiki-source-9c3e7b4b0d07a43f862a9bd7
    resource: repo://crates/db/migrations/20261018000000_project_todos.sql
  - id: openwiki-source-77fa2216e4e605d16556c5db
    resource: repo://crates/db/src/models/todo.rs
  - id: openwiki-source-2d2a5b79056fd785f8bb7684
    resource: repo://crates/mcp/src/task_server/tools/todos.rs
  - id: openwiki-source-4c077906852b0e677ae7c746
    resource: repo://crates/server/src/remote_mcp/todos.rs
  - id: openwiki-source-ac6b6772c135d9aa4040d538
    resource: repo://crates/server/src/routes/todos.rs
  - id: openwiki-source-3ab59f96f5b3a57177b098b6
    resource: repo://packages/web-core/src/features/todos/model/todoApi.ts
  - id: openwiki-source-cc2b48361dc9ee47e4a6cc4c
    resource: repo://packages/web-core/src/features/todos/model/todoViewStore.ts
  - id: openwiki-source-748c9fc87da8ec6efd060367
    resource: repo://packages/web-core/src/features/todos/ui/TodoPanel.tsx
  - id: openwiki-source-8cbec0ee8d7f6154508744c9
    resource: repo://packages/web-core/src/features/todos/ui/WorkspaceTodoPanel.tsx
generated: { by: "codex", at: "2026-10-21T03:28:49.332Z" }
---

# Project Todo の共有メモ

Todo は Project 内で共有する本文・任意カテゴリー・明示的な完了状態を持つメモである。[Issue / Card](project-and-issue.md)や Board の [Not Started 列（旧 Todo）](project-and-issue.md#kanban-と実行の同期契約)、旧 Task、agent の実行計画とは別の `project_todos` レコードであり、作成や完了で Card や実行を自動生成しない。本文は参照データで、実行指示として扱わない。[モデル](../../crates/db/src/models/todo.rs#L1-L45)、[外部 MCP の操作契約](../../crates/server/src/remote_mcp/todos.rs#L60-L83)

## 所属と寿命

全読み書きは Project と Todo の identity を組み合わせる。同一 Project の Card を任意に参照できるが、Card への変換ではない。別 Project の Card はモデルと DB trigger の双方が拒否する。Project 削除はメモを削除し、関連 Card 削除は参照だけを null にする。[所属と制約](../../crates/db/src/models/todo.rs#L84-L113)、[migration](../../crates/db/migrations/20261018000000_project_todos.sql)

本文は空白のみを拒否し最大16,000文字、カテゴリーは前後空白を除去し最大100文字、空文字は未分類とする。本文の改行や文字列は保持する。完了は削除と別で、未完了へ戻せる。[保存時検査](../../crates/db/src/models/todo.rs#L75-L81)、[作成と更新](../../crates/db/src/models/todo.rs#L133-L192)

## 更新競合と再送

REST / stdio の作成は client が固定 UUID を渡し、同じ本文・カテゴリー・Card 参照なら既存項目を返す。同じ ID に違う内容を重ねると競合する。更新・削除には取得時の `expected_revision` が必要で、更新は SQL の条件付き書込で revision を増やす。古い revision を最新値で勝手に置換して再送しない。省略 field は保持し、Card 参照を外すには `clear_issue=true` を使う。これと新しい `issue_id` の同時指定は拒否する。[原子的更新](../../crates/db/src/models/todo.rs#L133-L209)

REST は不在を404、競合を409、不正入力を400、DB故障を内部エラーとして区別する。削除成功後の再送を、同じ結果が保証された成功とは扱わない。[エラー変換](../../crates/server/src/error.rs#L202-L210)

| 入口 | 共有する状態と応答 |
| --- | --- |
| Web REST `/api/projects/{project_id}/todos` | DB モデルの結果を ApiResponse で返す |
| 内部 stdio MCP | REST を呼び、作成は安定した `input.id`、更新・削除は revision を渡す |
| 外部 HTTP MCP | read に `lvk:read`、変更にさらに `lvk:write`。変更は `request_id` に束縛した durable operation receipt を返す |

[REST](../../crates/server/src/routes/todos.rs#L14-L71)、[stdio](../../crates/mcp/src/task_server/tools/todos.rs#L41-L135)、[外部 MCP](../../crates/server/src/remote_mcp/todos.rs#L113-L235)。外部 MCP は DB エラーを uncertain、モデル拒否を failed として保存する。応答不明時は同じ request ID の receipt と Todo を読む。接続断・再起動時の一般的な意味は [外部操作 receipt](../integrations/remote-access.md#参照結果と操作-receipt)に従う。

## 通常操作と画面状態

Project の Board / Todos 切替と Workspace の Todo panel は同じ Project のメモを使う。Workspace は現在の Card link の Project を優先し、未関連なら利用者が Project を選ぶ。Project 判定の取得失敗を「未関連」の成功へ変換しない。[Workspace の解決](../../crates/server/src/routes/todos.rs#L74-L91)、[選択 UI](../../packages/web-core/src/features/todos/ui/WorkspaceTodoPanel.tsx)

一覧は作成時刻降順・ID順の offset pagination。REST は既定100・最大200件、MCP は100件ずつ、`next_offset` が null になるまで読む。これは固定 snapshot の cursor ではないので、並行追加中の全ページが同時点の集合である保証はない。Web は完了済みも含め200件ずつ読み、画面側でカテゴリーと完了表示を分け、5秒ごとに再取得する。[一覧](../../crates/db/src/models/todo.rs#L114-L132)、[Web 取得](../../packages/web-core/src/features/todos/model/todoApi.ts#L13-L25)、[表示](../../packages/web-core/src/features/todos/ui/TodoPanel.tsx#L33-L82)

追加 draft と表示先は Zustand のメモリ上に保持し、pane 切替では残るが、DB に保存した Todo と同じ永続性は持たない。編集は開始時 revision を保持し、競合で再取得しても入力を自動上書きせずエラーを示す。追加失敗でも draft の ID を保持する。[画面状態](../../packages/web-core/src/features/todos/model/todoViewStore.ts)、[追加と編集](../../packages/web-core/src/features/todos/ui/TodoPanel.tsx#L41-L68)、[編集 revision](../../packages/web-core/src/features/todos/ui/TodoPanel.tsx#L182-L235)

## 変更時の検証入口

[モデルテスト](../../crates/db/src/models/todo_tests.rs)は並行 revision、所属、文字列、完了・再開、pagination を確認する。[MCP テスト](../../crates/server/src/remote_mcp/tests.rs#L1191-L1300)は scope・receipt再送・revision を確認する。[隔離 E2E](../../tests/todos/isolated.mjs)は実 backend、Web、stdio を組み合わせる検証入口である。これらの存在は今回の実行結果や実 ChatGPT 接続成功を意味しない。
