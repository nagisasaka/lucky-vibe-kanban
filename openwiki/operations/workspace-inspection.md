---
type: operation
title: Workspace の閲覧と操作
description: 作業環境の状態、ファイル、差分と実行を確認する。
tags: [workspace, files, inspection]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-22T05:31:45.518Z
sources:
  - id: openwiki-source-6459ca8fb9da2218e17b3ac9
    resource: repo://crates/server/src/routes/workspaces/mod.rs
generated: { by: "codex", at: "2026-11-22T05:31:45.518Z" }
---

# Workspace の閲覧と操作

一覧で名前、branch、Issue 番号を使って対象 [Workspace](../concepts/workspace.md) を探す。番号は部分一致で、複数 Project や Workspace に同じ候補があり得る。Project、branch、用途と履歴を確認する。[検索](../../packages/web-core/src/pages/workspaces/workspaceSearch.ts)

## 実行専用 Workspace の確認

`execution_only` では owner、状態、Session と公開結果を閲覧し、Stop は所有者の処理に委ねる。AgentRun の成功と Integration／Wiki の公開は別の結果である。archived や実行専用環境の閲覧は削除済み worktree を再生成しない。[inspection](../../packages/web-core/src/features/workspace-chat/ui/ExecutionInspectionPanel.tsx)・[状態](../../crates/server/src/routes/workspaces/usage.rs)・[読取](../../crates/services/src/services/container.rs)

## ファイル・差分と Wiki

Repo file API は Workspace membership と canonical root を検査し、親参照や root 外 symlink を拒否する。text preview は上限で切り詰められ得るので全文と決めつけない。差分の空、取得不能、未取得を区別する。[files](../../crates/server/src/routes/workspaces/files.rs)

Wiki panel は現在選択した Workspace の worktree を表示するため、未公開の変更を含み得る。Repo、branch、path と公開状態を確認する。reader は生成や修復を行わない。[panel](../../packages/web-core/src/features/wiki/ui/WorkspaceWikiPanel.tsx)・[保守](openwiki-maintenance.md)

## Preview と開発サーバー

terminal、script、dev server はファイル閲覧と別の副作用を持つ。開始後は process と readiness を確認し、停止要求と実際の停止を区別する。外部からの操作は [Runtime Access](../concepts/runtime-access.md) の設定と scope が必要である。[dev server](../../crates/server/src/remote_mcp/dev_server.rs)

チャット末尾の Workspace 累積トークンは保存された報告の部分集計であり、料金や現在の context 使用率ではない。[集計](../concepts/workspace.md#workspace-累積トークン)
