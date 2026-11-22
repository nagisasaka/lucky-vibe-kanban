---
type: concept
title: Astra・Project Chat の廃止
description: 旧内蔵会話の削除と、残る質問回答・外部 MCP・対話ヘルプの境界。
tags: [astra, migration, project, compatibility]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-22T05:31:45.518Z
sources:
  - id: openwiki-source-b50ba7975e31fb9a56845b2c
    resource: repo://crates/db/migrations/20261016000000_remove_project_chat.sql
  - id: openwiki-source-4d617b0e2fa188c6ad63ae56
    resource: repo://crates/server/src/routes/sessions/questions.rs
generated: { by: "codex", at: "2026-11-22T05:31:45.518Z" }
---

# Astra・Project Chat の廃止

内蔵 Astra / Project Chat は廃止され、Project 画面は Board を直接表示する。旧 discuss / operate、専用モデル設定と server 内の専用会話 worker を現行機能と扱わない。過去のデータや名前を調査するときにこのページを読む。[画面](../../packages/web-core/src/features/kanban/ui/KanbanContainer.tsx)・[router](../../crates/server/src/routes/mod.rs)・[操作の段階](../quickstart.md#操作の段階)

## 履歴と残る機能

前進 migration は専用通知 trigger と `project_chat_operations`、`project_chat_messages`、`project_chat_events`、`project_conversations` を削除する。履歴の外部サービスへの移行ではなく、DB upgrade で専用履歴を失う変更である。[migration](../../crates/db/migrations/20261016000000_remove_project_chat.sql)

Project、Issue、Workspace、Session と agent の質問回答は共有状態として残る。旧名を理由に共有機能まで削除しない。質問回答に残る歴史的 `Astra operation` marker は配送済み receipt の互換性であり、内部 assistant が動いている証拠ではない。[移行試験](../../crates/db/tests/retire_project_chat.rs)・[質問回答](../../crates/server/src/routes/sessions/questions.rs)

会話から LVK を操作する外部入口は[認証付き MCP](../integrations/remote-access.md)である。一般質問の[対話ヘルプ](interactive-help.md)は既存 coding agent の Session と稼働版の公開 source を使い、旧 Project Chat の専用会話モデルではない。設計意図は [Notes](design-notes.md)、実行状態は [Agent Run](session-and-agent-run.md)、統合済み知識は [Repository Memory](repository-memory.md) にそれぞれ属する。

移行 fixture は専用表の削除と共有状態の保持を検査する。実外部アカウント接続や旧設定ファイルの消去は証明しない。[tests](../../crates/db/tests/retire_project_chat.rs)
