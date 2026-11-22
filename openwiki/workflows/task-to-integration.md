---
type: workflow
title: Issue から実行・レビュー・統合まで
description: 要求から作業、成果確認、統合と公開までの順序。
tags: [workflow, issue, integration]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-22T05:31:45.518Z
sources:
  - id: openwiki-source-595d5d0308d670e38ef27fbf
    resource: repo://crates/server/src/routes/integrations/admission.rs
generated: { by: "codex", at: "2026-11-22T05:31:45.518Z" }
---

# Issue から実行・レビュー・統合まで

[Issue](../concepts/project-and-issue.md) は要求、[Workspace](../concepts/workspace.md) は作業場所、[Session と Agent Run](../concepts/session-and-agent-run.md) は会話と実行である。Issue に複数 Workspace や [Workflow Attempt](../concepts/workflow-attempt.md) があり得る。

1. Issue から Repo と target branch を選び、単一 Agent または固定 preset を開始する。作成、link、Session 準備、起動は全て一つの transaction ではないため、失敗時は既存状態を確認する。[作成](../../crates/server/src/routes/workspaces/create.rs)
2. setup が必要な場合は設定された順序と gate を確認する。parallel setup は完了を待つ保証ではない。実行中の質問、承認、停止、続行は対象 Run に対して行う。[setup gate](../../crates/server/src/routes/sessions/setup_gate.rs)
3. [ファイルと差分](../operations/workspace-inspection.md)を確認し、必要な検証とレビューを行う。Agent の終了、Issue の列、検証合格は異なる証拠である。同じ Workspace の複数 Session はファイルを共有する。
4. [正式 Integration](../concepts/formal-integration.md) では採用 Workspace と固定 source commit、local target と base を選ぶ。受付後の変更は黙って含めない。担当は候補、検証、必要な Wiki 更新、公開要求を進める。公開要求の受付と host による公開成功を分け、失敗や不明時は保存された状態と対象を確認する。[受付](../../crates/server/src/routes/integrations/admission.rs)
5. [GitHub PR](../integrations/github-review.md) や明示 push は別の外部公開操作である。Git の結果、Issue の完了、Workspace archive、Wiki の鮮度をそれぞれ確認する。archive では未 commit の作業が失われ得る。[Wiki 保守](../operations/openwiki-maintenance.md)・[Workspace](../concepts/workspace.md)
