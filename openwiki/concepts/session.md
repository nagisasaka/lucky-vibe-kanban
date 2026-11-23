---
type: concept
title: Session
description: Workspace 内で coding agent と行う独立した会話。
tags: [session, workspace, agent]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-3e5425718d87b437a12edd60
    resource: repo://crates/db/src/models/session.rs
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# Session

Session は [Workspace](workspace.md) 内の [coding agent](coding-agent.md) との会話である。同じ Workspace の Session は作業ファイルを共有するが、会話 context は独立する。複数の [Agent Run](agent-run.md) を含み、Run が終わっても会話を継続できる。[操作の段階](../quickstart.md#操作の段階)

Session の ID と provider 固有の session ID は別である。Follow-up、Retry と provider の切替の違いは [Session と Agent Run](session-and-agent-run.md) を参照する。[Session model](../../crates/db/src/models/session.rs)
