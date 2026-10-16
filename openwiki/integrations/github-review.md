---
type: integration
title: GitHub PR とレビューの境界
description: 現行のローカル PR 作成・監視と、削除された Cloud Review / GitHub App の区別。
tags: [github, review, organization, webhook, cloud]
sources:
  - id: openwiki-source-65c0f36670a161dc05040e69
    resource: repo://crates/server/src/routes/mod.rs
  - id: openwiki-source-27c2433bab843f01a51418b1
    resource: repo://crates/server/src/routes/workspaces/pr.rs
  - id: openwiki-source-72948db7f01925b4b2ff17af
    resource: repo://crates/services/src/services/pr_monitor.rs
generated: { by: "codex", at: "2026-10-16T12:21:28.631Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-16T14:34:12.007Z
---

# GitHub PR とレビューの境界

現在の PR 操作は local Workspace と `GitHostService` を起点とする。旧 Cloud Review、組織 GitHub App installation、webhook 起動、R2 upload / review worker / callback は現行構成から削除されている。`!reviewfast` や旧 App 設定を有効な起動契約として扱わない。[現行 router](../../crates/server/src/routes/mod.rs)、[Workspace PR](../../crates/server/src/routes/workspaces/pr.rs)。

## PR の作成と保存

PR 作成は対象 Repo / branch / remote を検査し、[Repository Memory の source 完了と Wiki guard](../concepts/repository-memory.md#source-統合と-wiki-反映を分ける)を通し、作業 branch を push してから GitHostService に title・body・base・draft 等を渡す。GitHub の場合は GitHub CLI の認証を使う。これは Cloud App の installation を要求する経路ではない。[手動操作の案内](../../docs/integrations/github-integration.mdx)、[処理順](../../crates/server/src/routes/workspaces/pr.rs#L206-L345)。

push、remote PR 作成、local PR レコード作成は一括 transaction ではない。remote 作成後の local 保存失敗はログに残るため、エラーや表示欠落を理由に新規 PR を盲目的に作り直さず、既存 PR と branch を確認する。unsupported provider・CLI 未導入・認証失敗は区別した結果を返す。[結果と失敗](../../crates/server/src/routes/workspaces/pr.rs#L302-L402)。既存 PR を branch から付ける経路も別にある。[既存 PR の関連付け](../../crates/server/src/routes/workspaces/pr.rs#L404-L534)。

## 観測と完了を分ける

PR monitor は open PR の remote status を調べ、closed / merged を local レコードへ保存する。merged Workspace に open PR が残らず pinned でない場合は archive を試みる。archive 失敗はログとなり、remote merge を元へ戻すものではない。[monitor](../../crates/services/src/services/pr_monitor.rs#L119-L202)。

外部 merge の観測は local target の更新、正式 Integration の検証、Card Done、Wiki 更新の証明ではない。Memory event を統合済みと判断する祖先関係と receipt の境界は [外部 PR と Repository Memory](../concepts/repository-memory.md#source-統合と-wiki-反映を分ける)を参照する。

## 三種類の review

Workspace の agent review はその Session / AgentRun の作業であり、[実装から PR まで](../workflows/task-to-integration.md)の手順に属する。[正式 Integration](../concepts/formal-integration.md)の解決差分 review は候補 commit に結び付く公開 gate、[OpenWiki の独立 Review](../operations/openwiki-maintenance.md)は凍結ソースと知識の照合工程である。いずれも撤去された Cloud review worker の成功 callback とは異なる。レビュー結果だけでテスト、公開、通知到達まで完了したとは扱わない。
