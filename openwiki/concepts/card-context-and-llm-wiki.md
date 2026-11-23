---
type: concept
title: Card context と LLM Wiki
description: 保存本文、並列参照と固定 source の境界。
tags: [issue, context, parallel-work, repository-memory]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-391f34050223d13f679b3793
    resource: repo://crates/server/src/routes/parallel_context.rs
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# 保存済み本文と並列作業の参照

[Issue](project-and-issue.md) の本文は人間の要求を保存する通常のテキストである。旧 Card Context と Pipeline の専用 UI・供給経路は退役している。保存文、起動時のガイド、他 Workspace の公開情報を変更するとき、それぞれの境界を区別する。[操作の段階](../quickstart.md#操作の段階)

## 保存本文と送信

専用欄や hidden 部分の再結合を前提にせず、独自文、編集済み文、未知の Pipeline は通常本文として扱う。旧設定や本文を一括削除・移行する契約ではない。会話の表示も本文全体を通常 renderer へ渡し、Markdown の HTML comment が画面に見えることまでは保証しない。[Issue editor](../../packages/web-core/src/pages/kanban/KanbanIssuePanelContainer.tsx)・[会話表示](../architecture/web-and-sync.md)

保存 Issue から prompt を合成する限定経路では、既知の LVK 生成共有案内だけを送信時に除去する。CRLF を正規化した内容の SHA-256 が既知5版に完全一致する必要があり、marker の存在だけで本文を捨てない。編集済み、未知、旧形式、不完全または入れ子の envelope は保持する。fence に見える行があれば引用を保護するため入力全体を保持する。任意の Review 入力を掃除する helper ではない。[helper と回帰](../../crates/executors/src/executors/provider_adapter.rs)

## 現在の identity とガイド

通常起動では現在の Run・Repo identity と短い契約を組み立て、Manifest と並列参照の詳細を内容に対応した on-demand guide に置く。ガイドは既存の内容と一致すれば再利用し、未知の内容や symlink を上書きしない。stdio `get_context` は引数なしでは metadata を返し、`coding`、`parallel` を明示した場合に該当案内を返す。[ガイド](../../crates/utils/src/agent_guides.rs)・[context tool](../../crates/mcp/src/task_server/tools/context.rs)・[実行時指示](../../crates/executors/src/env.rs)

[Repository shared storage](workspace.md) の受渡し用 tmp は Run 終了だけで消えず、受渡し完了後に不要かつ非使用であることを確認して整理する。Git の著者 identity と [Change Manifest](repository-memory.md) の現在 identity は別である。[README](../../README.md)

## 並列活動と固定した source

Parallel discovery は登録 Repo に属する interactive な worktree Workspace、現在の Issue link、実行数、観測時の branch HEAD と確定 Manifest 参照を返す。peer の worktree を再作成せず、共通 Repo の branch を読む。活動と Manifest は独立した snapshot であり、Manifest がないことは活動がない証明ではない。現在の Issue link を過去 event の帰属に読み替えない。[discovery](../../crates/services/src/services/parallel_context.rs)・[Manifest の正本](../../crates/db/src/models/repository_state/manifests.rs)

Workspace と Manifest は別 cursor でページングし、既定25件、最大100件を読む。memory 読取失敗は `memory_error` として返し、空結果と区別する。DirectFolder には自動共有リンクや peer discovery がない。Wiki の target・source・publication commit は観測情報であり、現在の作業木に最新 Wiki が配達された証明ではない。[discovery](../../crates/services/src/services/parallel_context.rs)

固定 snapshot API は指定 commit の Git object を読む。一覧は100件ずつで、本文は通常ファイルの UTF-8、128 KiB 以下に限る。symlink や submodule を辿らず、範囲外には理由を返す。組合せの試行は返された detached preview で行い、peer の作業木や target を変更しない。[snapshot と preview](../../crates/server/src/routes/parallel_context.rs)・[統合フロー](../workflows/task-to-integration.md)

旧 `.llm-wiki` は現在の OpenWiki と別である。旧互換 filter や Pipeline endpoint の撤去は、既存ファイルや保存 Issue の自動削除を意味しない。[router](../../crates/server/src/routes/mod.rs)・[Repository Memory](repository-memory.md)

保存文の helper と discovery の fixture は選択された境界を検査する。実モデルが案内を読んだことや全 peer の活動を観測できたことの証明ではない。[prompt fixture](../../tests/chat/prompt-context.test.mjs)・[discovery tests](../../crates/services/src/services/parallel_context.rs)
