---
type: operation
title: OpenWiki の生成・同期・回復
description: Repository Wiki の所有、生成、検証と公開。
tags: [openwiki, maintenance]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-22T18:40:23.680Z
sources:
  - id: openwiki-source-7277adee5c65e8d79143e722
    resource: repo://crates/services/src/services/openwiki/bootstrap.rs
  - id: openwiki-source-fa966dc92e5b940e2e3155e3
    resource: repo://crates/services/src/services/openwiki/completion.rs
generated: { by: "codex", at: "2026-11-22T18:40:23.680Z" }
---

# OpenWiki の生成・同期・回復

OpenWiki は統合済み source を対象とする repository memory である。[Change Manifest と source checkpoint](../concepts/repository-memory.md) は意味の手掛かりであり、現在の実装は source、tests、設定で検証する。単独保守は明示要求で開始し、pending event や再起動だけから新しい writer を自動生成しない。[受付](../../crates/server/src/routes/openwiki.rs)

## 開始と所有

Repository settings で有効化し、local target branch と言語を選ぶ。未解決の source integration や、選択 branch に必要な統合 commit がない場合は開始しない。既存 Wiki は通常 Sync、初期生成は Bootstrap を使う。外部 MCP の Sync は既存 Wiki を対象とし、暗黙に初期生成しない。専用 [execution-only Workspace](../concepts/workspace.md#操作用途と実行所有者) と owner は準備中から保護される。[受付](../../crates/server/src/routes/openwiki.rs)・[存在判定](../../crates/services/src/services/openwiki.rs)

Bootstrap は固定 source に対する生成、独立 review、必要な refinement と host 公開の各段階を分ける。writer は保存された指示と ignore を守り、登録済み OpenWiki integration の lifecycle を完了する。review の完了は指摘がないことと同義ではない。通常 Sync は同じ独立 review を毎回実施する方式ではない。[Bootstrap 契約](../../crates/services/src/services/openwiki/bootstrap.rs)

Generate は初期生成の完了を必要とする。Refine は操作不要と検証できた場合に変更なしでも完了できるが、開始した操作はいずれも監査された `openwiki_finish` の complete が必要である。既定テンプレートは有効な旧知識と入口リンクを守りながら、読者の目的に沿った構成を許す。[writer 契約](../../crates/services/src/services/openwiki/bootstrap.rs)・[既定指示](../../assets/openwiki-instructions.md)

## 受付・実行・公開・鮮度

MCP からは現在の対象 branch と source commit を読み、安定 request ID と期待値を指定して同期する。応答不明なら同じ入力と request ID の receipt を確認する。受付成功や AgentRun 成功だけでは host の publication 完了にならず、owner、Run、checkpoint と保存結果を照合する。過去の成功も現在の source に対する鮮度の保証ではない。[MCP](../../crates/server/src/remote_mcp/repository_wiki.rs)・[receipt](../../crates/server/src/remote_mcp/operations.rs)

自動マージ中の Wiki 更新は同じ候補で code 検証の後に行い、code commit と Wiki commit、最終公開を区別する。変更途中の setup や journal を手で修復して guard を回避せず、所有者の回復経路を使う。[正式 Integration](../concepts/formal-integration.md)・[setup](../../crates/services/src/services/openwiki/setup/integration.rs)

中断後の監査では、同じ所有 Run の正しい再開と終了を確認する。再開前の `next_page` に対する既知の非変異拒否は厳密な引数と監査結果が一致する場合に限って回復でき、未知のエラーや異なる Run は公開証跡として受け入れない。[完了検査](../../crates/services/src/services/openwiki/completion.rs)
