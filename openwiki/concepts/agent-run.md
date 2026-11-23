---
type: concept
title: Agent Run
description: Session 内の実行依頼と試行、状態の関係。
tags: [agent-run, session, lifecycle]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-d20a82e2192a07c687b838cb
    resource: repo://crates/db/src/models/agent_runtime.rs
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# Agent Run

Agent Run は [Session](session.md) 内の一つの実行依頼である。実行を追跡し、再試行や終了を判断するときに読む。Run と実プロセスの寿命は同じではない。[操作の段階](../quickstart.md#操作の段階)

Run の各試行は Attempt として識別される。Retry は同じ Run の新しい試行であり、Follow-up は同じ Session の新しい Run である。Resume と Restart の違いや要求 ID の扱いは [Session と Agent Run](session-and-agent-run.md) を参照する。

Run は待機、起動、実行、入力・承認待ち、取消を経て終端状態に至る。終端表示だけでは実プロセスの終了や成果の統合を証明しない。永続イベント、監査と回復の経路は [Agent Runtime](../architecture/agent-runtime.md) にある。

[状態契約](../../crates/executors/src/runtime/contracts.rs)・[永続モデル](../../crates/db/src/models/agent_runtime.rs)
