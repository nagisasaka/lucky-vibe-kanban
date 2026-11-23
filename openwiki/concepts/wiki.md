---
type: concept
title: Wiki
description: 統合済み source に関する OpenWiki の知識と保守境界。
tags: [wiki, repository-memory, openwiki]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-231241ac3c842dcd67525890
    resource: repo://crates/server/src/routes/openwiki.rs
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# Wiki

Wiki は統合済み source についての知識を OpenWiki で保守するものである。[Change Manifest](change-manifest.md) や設計メモは意図の手掛かりであり、現在の動作は source、tests、設定で確かめる。[操作の段階](../quickstart.md#操作の段階)

明示的な保守の受付、writer の実行、内容の検証、Git 公開と現在の鮮度は別の事実である。外部 MCP の同期は対象 branch と source を固定し、存在しない Wiki を暗黙に初期生成しない。手順と回復は [OpenWiki 保守](../operations/openwiki-maintenance.md) を参照する。[受付](../../crates/server/src/routes/openwiki.rs)
