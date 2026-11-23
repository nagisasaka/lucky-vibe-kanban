---
type: concept
title: Repository shared storage
description: 登録 Repo ごとに Workspace 間で共有する Git 外の領域。
tags: [repository, storage, workspace]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-7d5590729f3e0f42d994b0e8
    resource: repo://crates/workspace-manager/src/shared_resources.rs
  - id: openwiki-source-23775c3de52f3ab95a13cb8b
    resource: repo://README.md
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# Repository shared storage

Repository shared storage は同じ登録 Repo の [Workspace](workspace.md) 間で使う Git 外の領域である。複数作業場所の成果物を受け渡すときに読む。[操作の段階](../quickstart.md#操作の段階)

`.lvk/shared/tmp` は一時受渡し、`cache` は再生成可能な中間物、`persistent` は利用者が保持する成果物に使う。別 Repo の領域を混ぜず、秘密を commit しない。内部の復旧用 tmp/cache は別管理である。[README](../../README.md)・[実装](../../crates/workspace-manager/src/shared_resources.rs)

Run 終了だけでは tmp を削除しない。受渡しが終わり、不要かつ使用中でないことを確認して整理する。必要な結果は保持先へ移す。自動 TTL はない。
