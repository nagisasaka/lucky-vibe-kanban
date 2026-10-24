---
type: concept
title: Astra 廃止と共有機能の移行境界
description: 内蔵 Project Chat の廃止、専用履歴を削除する migration、外部 MCP と Workspace 質問回答に残る契約を説明する。
tags: [astra, migration, project, compatibility]
sources:
  - id: openwiki-source-b50ba7975e31fb9a56845b2c
    resource: repo://crates/db/migrations/20261016000000_remove_project_chat.sql
  - id: openwiki-source-92a1ceb146fe14016a595480
    resource: repo://crates/db/tests/retire_project_chat.rs
  - id: openwiki-source-a13fe4db1eee073d0a7e2c4d
    resource: repo://crates/server/src/main.rs
  - id: openwiki-source-71f837a8aa36443ad9688945
    resource: repo://crates/server/src/remote_mcp/mod.rs
  - id: openwiki-source-65c0f36670a161dc05040e69
    resource: repo://crates/server/src/routes/mod.rs
  - id: openwiki-source-4d617b0e2fa188c6ad63ae56
    resource: repo://crates/server/src/routes/sessions/questions.rs
  - id: openwiki-source-e0c2de9e01617a8949b78360
    resource: repo://packages/web-core/src/features/kanban/ui/KanbanContainer.tsx
generated: { by: "codex", at: "2026-10-17T03:50:48.298Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-24T01:51:44.796Z
---

# Astra 廃止と共有機能の移行境界

内蔵 Astra / Project Chat は廃止された。Project 画面は Board を直接表示し、server router と起動処理に専用 chat API・設定 API・worker はない。以前の discuss / operate、モデル設定、Codex ChatGPT login による内部推論は現行機能ではない。[Project 画面](../../packages/web-core/src/features/kanban/ui/KanbanContainer.tsx)・[API の構成](../../crates/server/src/routes/mod.rs)・[起動](../../crates/server/src/main.rs)

会話から LVK を操作する外部入口は [認証付き MCP](../integrations/remote-access.md)である。その transport は内部会話モデルや model session を作らず、既存の製品操作に接続する。外部クライアントの認証・scope を、旧 Astra の server 内 Codex 認証で代替しない。[MCP の責任](../../crates/server/src/remote_mcp/mod.rs#L1-L58)

## 専用履歴の削除と残る状態

前進 migration `20261016000000_remove_project_chat.sql` は、専用の八つの通知 trigger と `project_chat_operations`、`project_chat_messages`、`project_chat_events`、`project_conversations` を削除する。履歴を外部 ChatGPT に移す処理ではなく、DB upgrade で専用履歴を失う変更である。元の migration を削除・書換えず、適用済み履歴から前進させる。[migration と意図](../../crates/db/migrations/20261016000000_remove_project_chat.sql)

Project・Issue・Workspace・Session と `agent_question_answers` は共有状態として残る。upgrade test は既存会話を含む DB を更新し、これらの保持、`agent_input_still_pending` trigger、更新後の Board 書込・質問回答・外部キー整合性、再度 migration を実行した場合を検査する。旧 `project_chat` 名だけを根拠に、導入時に追加された共有機能まで削除してはいけない。[移行テスト](../../crates/db/tests/retire_project_chat.rs#L5-L89)

## 質問回答と操作 receipt はそれぞれの所有者へ

Workspace UI と外部 MCP は同じ質問受付を利用する。質問回答の新旧 receipt marker と二重配送防止は [Session の質問回答](session-and-agent-run.md#非同期質問への回答と配送証拠)を正本とする。旧 `Astra operation` marker がソースに残るのは配送済み履歴との互換性であり、内部アシスタントが動く証拠ではない。[互換処理](../../crates/server/src/routes/sessions/questions.rs#L253-L324)

外部 MCP の request receipt は [外部操作の寿命と不確実な結果](../integrations/remote-access.md#参照結果と操作-receipt)を持つ。実行状態は [AgentRun](session-and-agent-run.md)、統合・検証は [正式 Integration](formal-integration.md)、知識の公開は [Repository memory](repository-memory.md)が所有する。廃止された Astra の会話 checkpoint を、それらの完了証拠として復活させない。

[ChatGPT MCP の実装記録](../../docs/development/chatgpt-mcp.md)にある Astra と過去 test 件数は廃止前の記録と明記されている。外部アカウント接続の成功や、過去の設定ファイルの実在・消去は、この source 照合では確認していない。
