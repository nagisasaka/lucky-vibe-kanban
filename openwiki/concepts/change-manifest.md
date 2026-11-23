---
type: concept
title: Change Manifest
description: Run の目的と判断を Git の実際の変更に結び付ける記録。
tags: [manifest, repository-memory, agent-run]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-21149aa9001849998cda42a2
    resource: repo://crates/db/src/models/repository_state/manifests.rs
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# Change Manifest

Change Manifest は [Agent Run](agent-run.md) の目的、変更、判断と未決点を伝える記録である。統合や Wiki の更新時に変更の意図を調べるために使う。現行動作の判断には source、tests、設定も確認する。[操作の段階](../quickstart.md#操作の段階)

Host は semantic draft を Git から得た変更と source commit に結び付ける。変更がなければ空の Manifest を作らない。draft が欠落・不正な場合の Git-only checkpoint は、意図や検証が不明であることを明示する。[完了処理](../../crates/services/src/services/repository_memory.rs)

確定記録は instance DB に保存され、同じ identity の異なる内容は拒否される。保存先、統合時の選択、Wiki receipt と回復の境界は [Repository Memory](repository-memory.md) を参照する。[DB model](../../crates/db/src/models/repository_state/manifests.rs)
