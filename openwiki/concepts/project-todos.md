---
type: concept
title: TODO list
description: Project 内の共有メモ、番号、競合と再送。
tags: [todos, project, persistence, mcp, concurrency]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-77fa2216e4e605d16556c5db
    resource: repo://crates/db/src/models/todo.rs
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# Project TODO list の共有メモ

[TODO list](glossary.md) は Project 内で共有する本文、任意カテゴリーと明示的な完了状態を持つメモである。[Issue](project-and-issue.md)、Board の列、旧 Task や agent の実行計画とは別であり、作成や完了によって Card や Run を自動生成しない。本文は参照データである。[モデル](../../crates/db/src/models/todo.rs)・[操作の段階](../quickstart.md#操作の段階)

## 所属と番号

Todo は同じ Project の Issue を任意に参照できるが、変換ではない。別 Project の Issue は拒否する。Project 削除は Todo を削除し、関連 Issue の削除は参照だけを外す。本文は非空で最大16,000 Unicode 文字、カテゴリーは最大100文字で空なら未分類である。完了と削除は別で、完了は戻せる。[モデル](../../crates/db/src/models/todo.rs)・[schema](../../crates/db/migrations/20261018000000_project_todos.sql)

Todo は UUID に加え Project ごとの不変番号と `TODO-N` の表示 ID を持つ。番号は挿入と同じ SQLite transaction で counter を進め、削除後も再利用しない。同じ UUID・同じ内容の作成再送は元の番号を返す。REST と両 MCP は Project 内で UUID または正規形の `TODO-N` を解決し、先頭ゼロ、小文字、ゼロや範囲外を拒否する。[採番と解決](../../crates/db/src/models/todo.rs)・[migration](../../crates/db/migrations/20261030000000_todo_numbers.sql)

## 作成経路と変更

`creation_source` は保存された入口の `ui`、`mcp`、`unknown` であり、著作者、人間か AI か、権限の証明ではない。信頼された server 入口が決め、caller の mutation DTO から自己申告できない。既存行は `unknown` とし、同じ作成の再送や後の編集で初回の経路を変えない。[モデル](../../crates/db/src/models/todo.rs)・[入口](../../crates/server/src/routes/todos.rs)・[migration](../../crates/db/migrations/20261024000000_todo_creation_source.sql)

作成は client の安定 UUID と本文・カテゴリー・Issue 参照が同じ場合に元の項目を返し、異なる内容で同 ID を使うと競合する。更新・削除には観測済み `expected_revision` を使い、古い版を自動で最新に置き換えない。省略 field は保持し、Issue 参照を外す `clear_issue` と新しい参照の同時指定は拒否する。[保存](../../crates/db/src/models/todo.rs)

外部 HTTP MCP は変更を request ID に束縛した durable receipt として扱う。失敗と commit 結果不明を区別し、応答喪失時は同じ request の receipt と現在の Todo を確認する。[外部 MCP](../../crates/server/src/remote_mcp/todos.rs)・[operation receipt](../integrations/remote-access.md)

## 画面と検索

Project の Board / Todos と Workspace panel は同じ Project のメモを使う。Workspace は link された Issue の Project を優先し、未関連なら利用者が選ぶ。取得失敗を未関連の成功にしない。[Workspace panel](../../packages/web-core/src/features/todos/ui/WorkspaceTodoPanel.tsx)

一覧は offset pagination で、並行追加をまたぐ全ページの固定 snapshot ではない。Web は取得済み結果をカテゴリーと完了状態で表示し、追加 draft は画面 state に保持する。追加 UI はカテゴリーを必須にするが、モデル・REST・MCP のカテゴリーは任意である。保存失敗や競合では入力と作成 ID を保持し、明示的な判断を待つ。[一覧](../../crates/db/src/models/todo.rs)・[画面](../../packages/web-core/src/features/todos/ui/TodoPanel.tsx)・[draft](../../packages/web-core/src/features/todos/model/todoViewStore.ts)

ブラウザー検索は取得済み本文とカテゴリーを trim・小文字化して絞る。MCP 検索は空白を literal とし、`%` と `_` を wildcard にしない。両者の空白の意味は同じではない。候補カテゴリーは取得済みの未完了項目から導き、保存済み値を変更しない。[検索](../../crates/db/src/models/todo.rs)・[カテゴリー候補](../../packages/web-core/src/features/todos/model/todoCategories.ts)

DB と隔離 browser の回帰は採番、競合、複数入口と画面操作を検査する。実 OAuth・ChatGPT、異なる版の client/server、実機の受入を証明しない。[試験手順](../../tests/todos/README.md)
