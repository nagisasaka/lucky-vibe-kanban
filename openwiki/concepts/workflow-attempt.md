---
type: concept
title: Workflow Attempt とグラフ
description: Issue の複数工程、固定 preset、実行と待機の関係。
tags: [workflow, attempt, graph]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-22T18:59:30.190Z
sources:
  - id: openwiki-source-a517c0f8e44d2cf0c440fbfc
    resource: repo://crates/server/src/routes/workflows.rs
generated: { by: "codex", at: "2026-11-22T05:31:45.518Z" }
---

# Workflow Attempt とグラフ

Workflow は工程の定義、Workflow Attempt は [Issue](project-and-issue.md) の一つの実施を表し、共有 [Workspace](workspace.md) とノードの [Session](session-and-agent-run.md) を結ぶ。Workflow Run は起動された定義の記録、NodeExecution は特定ノードと iteration の記録である。Agent Run の一回の retry と Workflow Attempt 全体は別単位である。[モデル](../../crates/db/src/models/workflow.rs)

## 固定 preset の開始

新しい仕事は共通開始画面で単一 Agent または固定 preset を選ぶ。実装後の独立レビュー、多角調査後の統合、棚卸し後の削減案はいずれも報告して停止し、採用・削除・Git 統合・公開を自動で許可しない。工程の provider、設定、利用者追記と Project 既定追記の revision を開始時に固定する。開始応答が不明なら同じ request ID と入力で再送する。[preset 定義](../../crates/workflow/src/presets.rs)・[開始と defaults](../../crates/server/src/routes/workflow_presets.rs)・[MCP](../../crates/server/src/remote_mcp/workflow.rs)

新規の任意 graph／Arena の作成・起動は終了した。既存の定義、履歴、retry と viewer は残るため、保存された旧 graph の挙動を固定 preset と混同しない。[入口](../../crates/server/src/routes/workflows.rs)

## 実行と合流

資源付き Attempt は draft、Workspace 準備、必要な Agent Session の固定を経て ready になる。開始時の graph snapshot を Run が使い、その後の定義編集で進行中の解釈を変えない。資源付きの新規開始は archive 済み Issue に対して拒否される。[準備](../../crates/server/src/routes/workflows.rs)・[runtime](../../crates/server/src/workflow_runtime/runner.rs)

Graph は Start、End、Agent、Condition、HumanGate、Transform、Arena などを表す。検証は一意な node、存在する edge、到達可能性を要求し、循環を拒否する。固定 preset の `wait_for_all` は全入力成功後に一度だけ合流するが、旧 graph の既定は一つの成功で ready となる。Agent は通常同じ Workspace のファイルを共有し、[Arena](arena.md) の候補は別 Workspace に隔離される。[graph](../../crates/workflow/src/graph.rs)・[validation](../../crates/workflow/src/validation.rs)・[planner](../../crates/workflow/src/planner.rs)

Condition の不正な判断は人の判断待ちに移る。HumanGate の承認は続行、拒否は失敗と下流の skip を伴う。node retry と provider の attempt retry は別で、対象や回復条件が異なる。詳細な dispatch・取消・再起動は [Workflow Runtime](../architecture/workflow-runtime.md) を参照する。[runner](../../crates/server/src/workflow_runtime/runner.rs)
