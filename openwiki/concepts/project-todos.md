---
type: concept
title: Project Todo の共有メモ
description: Project に保存する Todo と Issue・実行の違い、Web と二つの MCP の共有データ、競合・再送・表示の契約。
tags: [todos, project, persistence, mcp, concurrency]
sources:
  - id: openwiki-source-9c3e7b4b0d07a43f862a9bd7
    resource: repo://crates/db/migrations/20261018000000_project_todos.sql
  - id: openwiki-source-830e28055c4ae6f5464df312
    resource: repo://crates/db/src/models/design_note.rs
  - id: openwiki-source-77fa2216e4e605d16556c5db
    resource: repo://crates/db/src/models/todo.rs
  - id: openwiki-source-2d2a5b79056fd785f8bb7684
    resource: repo://crates/mcp/src/task_server/tools/todos.rs
  - id: openwiki-source-4c077906852b0e677ae7c746
    resource: repo://crates/server/src/remote_mcp/todos.rs
  - id: openwiki-source-ac6b6772c135d9aa4040d538
    resource: repo://crates/server/src/routes/todos.rs
  - id: openwiki-source-e0c2de9e01617a8949b78360
    resource: repo://packages/web-core/src/features/kanban/ui/KanbanContainer.tsx
  - id: openwiki-source-3ab59f96f5b3a57177b098b6
    resource: repo://packages/web-core/src/features/todos/model/todoApi.ts
  - id: openwiki-source-cc2b48361dc9ee47e4a6cc4c
    resource: repo://packages/web-core/src/features/todos/model/todoViewStore.ts
  - id: openwiki-source-748c9fc87da8ec6efd060367
    resource: repo://packages/web-core/src/features/todos/ui/TodoPanel.tsx
  - id: openwiki-source-8cbec0ee8d7f6154508744c9
    resource: repo://packages/web-core/src/features/todos/ui/WorkspaceTodoPanel.tsx
  - id: openwiki-source-dea540ef1c25316603003cab
    resource: repo://tests/chat/input-operations.isolated.mjs
  - id: openwiki-source-377661330a398dbc51b24a19
    resource: repo://tests/todos/isolated.mjs
  - id: openwiki-source-535ab30a62460ea3bb688c51
    resource: repo://tests/todos/layout-validation.md
  - id: openwiki-source-e252f7831699ea3488a22e81
    resource: repo://tests/todos/layout.mjs
generated: { by: "codex", at: "2026-10-23T14:57:15.192Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-23T14:57:15.192Z
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

一覧は作成時刻降順・ID順の offset pagination。REST は既定100・最大200件、MCP は100件ずつ、`next_offset` が null になるまで読む。これは固定 snapshot の cursor ではないので、並行追加中の全ページが同時点の集合である保証はない。Web は完了済みも含め200件ずつ読み、画面側でカテゴリーと完了表示を分け、5秒ごとに再取得する。[一覧](../../crates/db/src/models/todo.rs#L127-L142)、[Web 取得](../../packages/web-core/src/features/todos/model/todoApi.ts#L13-L25)、[表示](../../packages/web-core/src/features/todos/ui/TodoPanel.tsx#L33-L82)

追加 draft と表示先は Zustand のメモリ上に保持し、pane 切替では残るが、DB に保存した Todo と同じ永続性は持たない。編集は開始時 revision を保持し、競合で再取得しても入力を自動上書きせずエラーを示す。追加失敗でも draft の ID を保持する。[画面状態](../../packages/web-core/src/features/todos/model/todoViewStore.ts)、[追加と編集](../../packages/web-core/src/features/todos/ui/TodoPanel.tsx#L41-L68)、[編集 revision](../../packages/web-core/src/features/todos/ui/TodoPanel.tsx#L276-L315)

### 追加 UI の入力契約

追加欄は Enter で追加し、Ctrl+Enter で選択範囲を改行に置き換える。IME composition 中・keyCode 229 の Enter は追加に使わず、キー repeat でも追加しない。Ctrl+Enter は16,000の入力上限を越える挿入を拒否する。既存項目の編集 textarea はこの追加専用ハンドラーを使わない。[追加操作](../../packages/web-core/src/features/todos/ui/TodoPanel.tsx#L98-L139)・[既存項目の編集](../../packages/web-core/src/features/todos/ui/TodoPanel.tsx#L319-L349)

Enter と追加ボタンは共通の submit 検査を通る。空白だけの本文は送らず、カテゴリが空・空白だけならエラーを表示し draft を保持する。**カテゴリ必須は追加 UI の要件**であり、保存モデル・REST・MCP の任意カテゴリ契約を変更しない。既存未分類項目の表示・編集も残る。[共通検査](../../packages/web-core/src/features/todos/ui/TodoPanel.tsx#L49-L77)・[保存検査](../../crates/db/src/models/todo.rs#L82-L86)・[REST](../../crates/server/src/routes/todos.rs#L43-L59)・[外部 MCP の作成](../../crates/server/src/remote_mcp/todos.rs#L154-L176)。この境界は [LOCAL-26 の仕様判断](../../tests/chat/input-operations-validation.md#変更と仕様判断)にも記録されている。

追加開始時に同期 ref を立て、React の pending 表示前の連打も抑える。pending 中は本文・カテゴリ・ボタンを無効にし、成功時だけ draft を消して一覧を再取得する。失敗時は本文と ID を保持し、成功・失敗のどちらでも settled で ref を解除する。これはその画面内の重複抑止であり、上記の保存時 ID 照合を代替しない。[guard と結果処理](../../packages/web-core/src/features/todos/ui/TodoPanel.tsx#L42-L77)・[入力の無効化](../../packages/web-core/src/features/todos/ui/TodoPanel.tsx#L105-L165)。同じ Enter / Ctrl+Enter の操作でも、[チャットの送信中 draft](session-and-agent-run.md#送信中の編集と-markdown-正規化)は編集可能であり、Todo の pending と区別する。

### カテゴリー配置と読み順

Project の Todos 表示は複数列を許す `grid` を指定し、Workspace の panel は既定の単列を使う。Project 側も幅が狭ければ単列になる。列幅は `auto-fit` と `minmax(min(100%,320px),1fr)` で決まり、カテゴリーは `localeCompare` 順、カテゴリー内の項目は取得順を保つ。[Project の入口](../../packages/web-core/src/features/kanban/ui/KanbanContainer.tsx#L1139-L1147)・[Workspace の入口](../../packages/web-core/src/features/todos/ui/WorkspaceTodoPanel.tsx#L50-L56)・[グループ化](../../packages/web-core/src/features/todos/ui/TodoPanel.tsx#L79-L88)

複数列の配置は sparse CSS Grid の1px行と実測 row span を組み合わせる。短いカテゴリーの後へ次のカテゴリーを配置できるため、通常の行単位 Grid のように前行の最長カテゴリーの終了まで全列を待たせない。DOM はカテゴリー順のままで、上端の位置順、同じ高さなら左から右へ読む。dense backfill と CSS columns を避ける理由は読み順を守るためであり、すべての隙間を埋めることより順序を優先する。[配置実装](../../packages/web-core/src/features/todos/ui/TodoPanel.tsx#L192-L274)・[記録された判断](../../tests/todos/layout-validation.md#change-and-ordering-decision)

測る対象は Grid が割り当てたセルの高さではなく、その内側の自然な内容高である。下余白を持つ wrapper も計測に含め、切り上げた高さを最小1行の span にする。`useLayoutEffect` は初回計測後に `ResizeObserver` を登録するため、折返し・幅・編集欄・削除確認などの高さ変化を再計測する。`grid` または children が変わる際と unmount 時には observer を切断し、設定した `grid-row-end` を取り除く。wrapper 構造や余白を変更するときは、この測定対象との対応を維持する必要がある。[測定と cleanup](../../packages/web-core/src/features/todos/ui/TodoPanel.tsx#L192-L258)

この配置は表示層の責任であり、保存順や revision 更新の契約を追加しない。完了項目の非表示・最後の項目の削除はグループを消し、再表示は現在の取得結果からグループを作り直す。データ取得の境界は [Web と同期](../architecture/web-and-sync.md)、編集時の競合は上記の画面状態を参照する。[グループの導出](../../packages/web-core/src/features/todos/ui/TodoPanel.tsx#L79-L88)・[更新後の再取得](../../packages/web-core/src/features/todos/ui/TodoPanel.tsx#L288-L308)

## 変更時の検証入口

[モデルテスト](../../crates/db/src/models/todo_tests.rs)は並行 revision、所属、文字列、完了・再開、pagination を確認する。[MCP テスト](../../crates/server/src/remote_mcp/tests.rs#L1191-L1300)は scope・receipt再送・revision を確認する。[隔離 E2E](../../tests/todos/isolated.mjs)は実 backend、Web、stdio を組み合わせる検証入口である。これらの存在は今回の実行結果や実 ChatGPT 接続成功を意味しない。


入力操作の [隔離 E2E](../../tests/chat/input-operations.isolated.mjs#L183-L264)は空本文・カテゴリ必須・合成 IME・改行・repeat・保留中の重複追加を対象にする。Todo POST は遅延だけを制御して実 backend に通し、保存済み本文と件数を REST で照合する。チャット側は失敗応答の mock であり、[現行の編集可能契約と試験期待の差](session-and-agent-run.md#入力操作の検証範囲と未解決点)を別に扱う。開発時の [実行記録](../../tests/chat/input-operations-validation.md)は過去の結果で、この Sync は製品テストを再実行していない。

カテゴリー配置の [隔離ブラウザー検査](../../tests/todos/layout.mjs)は `isolated.mjs` から呼ばれ、24個の高低差のあるカテゴリーを実 API で追加する。重なり・横はみ出し・カテゴリー上端順・API と DOM の項目順、短い列への配置、Tab 移動を検査する。1600/1000/390/320px の幅、編集 textarea の拡大と保存、完了表示、削除確認と最後の項目削除を扱う。折畳み操作の試験ではない。[検査内容](../../tests/todos/layout.mjs#L14-L221)・[呼出し](../../tests/todos/isolated.mjs#L332-L334)

[検証手順](../../tests/todos/README.md)は built frontend と server・agent-process-host・vibe-kanban-mcp の事前 build を要求する。試験は独自の `LVK_INSTANCE_DIR` と loopback の自動割当 port を使う。[開発時の記録](../../tests/todos/layout-validation.md)には Chromium E2E の成功と初回 checkbox 即時判定の失敗・修正が残るが、Firefox・WebKit・実機モバイルは未検証である。これは今回の統合済み checkout に対する再実行結果ではない。

今回の Sync は凍結 source `3e644b488173dad2cf2e10a994181f544055845c` の Todo 配置変更を対象に、表示入口・データ順序・計測と cleanup・検査コードを静的に照合した。供給された統合記録は明示的な検証省略を示しており、本 Sync も製品 E2E を再実行していない。無関係な既存説明全体の再検証や、統合後のブラウザー動作確認を主張しない。

設計の意図・本文の版履歴・出典付きの独立追記を残す場合は、同じ Project 表示切替から [設計メモ](design-notes.md)を使う。Todo の完了・削除とは異なる履歴保持の契約を持つ。
