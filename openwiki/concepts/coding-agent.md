---
type: concept
title: Coding agent
description: Workspace で作業する Codex と Claude Code の実行境界。
tags: [agent, provider, execution]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-1222b138c478c7fe70625991
    resource: repo://crates/executors/src/executors/mod.rs
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# Coding agent

Coding agent は [Workspace](workspace.md) で実際に作業する AI である。LVK は Codex と Claude Code を扱い、provider の通信を共通の [Agent Run](agent-run.md) に変換する。[操作の段階](../quickstart.md#操作の段階)

Provider のログインは LVK サーバーへのログインと別である。発見された設定や利用可能な候補は、認証や実行成功の証明ではない。実行 account、profile、permission、MCP の設定は [Agent providers](../integrations/agent-providers.md) を参照する。

同じ Workspace の複数 Session はファイルを共有するが、会話の context は別である。[Session](session.md)・[provider 実装](../../crates/executors/src/executors/mod.rs)
