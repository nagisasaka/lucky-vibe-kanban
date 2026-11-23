---
type: reference
title: LVK 用語集
description: 仕事、作業場所、会話、統合と記憶の用語。
tags: [glossary, concepts, terminology]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-d20a82e2192a07c687b838cb
    resource: repo://crates/db/src/models/agent_runtime.rs
  - id: openwiki-source-3e5425718d87b437a12edd60
    resource: repo://crates/db/src/models/session.rs
  - id: openwiki-source-23775c3de52f3ab95a13cb8b
    resource: repo://README.md
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# LVK 用語集

この用語集は LVK の画面と実行基盤で使う名前を揃える。詳しい寿命や変更時の契約は各リンク先を参照する。操作の状態は[受付から内容受入まで](../quickstart.md#操作の段階)を区別する。

| 用語 | 意味 |
| --- | --- |
| Kanban | 仕事の流れを可視化して管理する方法。[Kanban](kanban.md) |
| Kanban board | 各 Project の仕事を列に並べる主画面。[Kanban board](kanban-board.md) |
| Card (Kanban card) | Issue が Board 上に現れた項目。[Card](card.md) |
| Column (lane) | Board を状態ごとに分ける列。Project ごとに変更できる。[Column](column.md) |
| Issue | 実際の仕事。本文に仕様と人間の意図を記録する。[Issue](issue.md) |
| Project | 開発作業の大きな整理単位。複数の Git Repository を関連付けられる。[Project](project.md) |
| Workspace | coding agent の作業場所。通常は一つ以上の Repo worktree を含み、DirectFolder もある。Issue に属する場合と独立する場合がある。[Workspace](workspace.md) |
| Session | Workspace 内の coding agent との一つの会話。同じ Workspace の Session は作業木を共有するが会話 context は独立する。[Session](session.md) |
| Agent Run (Run) | Session 内で agent が開始して終了する一単位。Retry は同じ Run 内の新しい Attempt である。[Run](agent-run.md) |
| Coding agent (agent) | Workspace で実際の作業を行う AI。LVK は Codex と Claude Code を扱う。[coding agent](coding-agent.md) |
| Coordinating AI | MCP から LVK を操作し、人との仕様相談、Issue 作成、委任と進捗確認を行う上位 AI。[上位AI](coordinating-ai.md) |
| Change Manifest | Run の目的、変更と判断を記録するデータ。確定した Git 状態と関連付け、統合や Wiki 更新に意図を渡す。[Change Manifest](change-manifest.md) |
| Auto-merge | 選択した commit を対象 branch に統合し、検証と Wiki 更新を経て公開する機能。[Auto-merge](formal-integration.md) |
| Repository shared storage | 同じ登録 Repo の Workspace 間で共有する Git 外の領域。persistent は保持成果物、cache は再生成可能な中間物、tmp は一時的な受渡しに使う。[Repository shared storage](repository-shared-storage.md) |
| TODO list | Project 内で共有するメモ。Issue とは独立し、任意で Issue を参照できる。[TODO](project-todos.md) |
| Notes (design notes) | Project の目的や設計判断を本文と出典付き追記に記録するもの。[Notes](design-notes.md) |
| Wiki | 統合済み source に関する知識。OpenWiki で保守し、source・tests・設定を現行動作の根拠にする。[Wiki](wiki.md) |

Settings の prompt 用 Tag と Issue の分類 Tag は別モデルである。前者は入力に展開する名前付き text、後者は Project の Issue を分類する名前と色である。[prompt Tag](../../crates/db/src/models/tag.rs)・[分類 Tag](../../crates/api-types/src/tag.rs)

REVIEW execution mode、Issue の共有レビュー、OpenWiki の独立 Review は別の操作である。前者は agent の作業意図、共有レビューは Issue のコメント、独立 Review は Wiki 内容の検査であり、名前が同じでも承認や公開を自動的に意味しない。[provider](../integrations/agent-providers.md)・[Issue の共同作業](issue-collaboration.md)・[Wiki 保守](../operations/openwiki-maintenance.md)
