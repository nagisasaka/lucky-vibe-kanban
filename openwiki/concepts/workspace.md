---
type: concept
title: Workspace
description: 作業場所、Repository、所有権と履歴の寿命。
tags: [workspace, repository, worktree]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-5f083da2f629bcd57e386ad4
    resource: repo://crates/db/src/models/workspace.rs
  - id: openwiki-source-2a2f483b690a2eb00cd79616
    resource: repo://crates/services/src/services/workspace_lifecycle.rs
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# Workspace と Repository

Workspace は作業場所、branch、接続 Repo と複数の [Session](session-and-agent-run.md) を束ねる実施環境である。Issue と関連付けられるが、同一ではない。[操作の段階](../quickstart.md#操作の段階)を区別する。Repo は登録された source と既定設定、WorkspaceRepo はその Workspace で使う Repo と target branch を表す。複数 Repo は共通 root の子 worktree にあり Git 履歴と操作は各 Repo で独立する。root 自体は Git Repo ではない。[Workspace](../../crates/db/src/models/workspace.rs)・[Repo](../../crates/db/src/models/repo.rs)・[接続](../../crates/db/src/models/workspace_repo.rs)

Worktree は LVK 管理の場所、DirectFolder は利用者が選んだ既存ディレクトリを使う。物理削除の可否は kind だけでなく ownership で決まり、External container path は削除しない。DirectFolder は Git root や Repo 内の子 directory を拒否し、通常の自動 commit や Repo Memory の対象にしない。[所有権](../../crates/db/src/models/workspace.rs)・[作成](../../crates/server/src/routes/workspaces/create.rs)

## 操作用途と実行所有者

`interactive` は通常開発、`execution_only` は [正式 Integration](formal-integration.md) や [OpenWiki 保守](../operations/openwiki-maintenance.md) の所有者が確保した実行環境である。用途は kind、ownership、archive と別の軸である。予約中は通常の chat、Git、terminal や変更操作を制限し、所有者専用の入口で制御する。終了後も履歴と owner を残し、process と cleanup の収束を確認してから用途を戻す。blocked や recovery_required を通常編集に解放しない。[用途](../../crates/db/src/models/workspace_usage.rs)・[境界](../../crates/services/src/services/workspace_usage.rs)

## Git commit identity

Git author／committer は LVK の actor や provider の identity と別である。新 commit は対象 Repo の有効な Git identity と明示的 override を尊重し、不足時に停止する。利用者の名前を推測したり fallback を設定したりしない。rebase は元 author を保持する。必要なら利用者が対象 Repo で `git config --local user.name` と `user.email` を設定する。[resolver](../../crates/git/src/identity.rs)・[操作](../../crates/git/src/lib.rs)

## 共有領域と寿命

[Repository shared storage](repository-shared-storage.md) は Repo ごとに一時受け渡し、再生成可能 cache、利用者が保持する persistent に分かれる。異なる Repo の store を混ぜず、必要な結果を一時領域だけに置かない。実機・DB の共有は worktree 分離とは別であり、必要に応じて環境や作業順序を分ける。

archive は明示操作で、忙しい処理や所有中の環境を確認する。archive 状態と worktree 物理削除は別の結果であり、削除失敗や filesystem 操作後の DB 失敗では実状態と receipt を確認する。履歴閲覧は worktree を再生成せず、未 commit 内容が削除後に復元される保証はない。明示削除は process や監査ログも削除対象になり得る。[archive](../../crates/server/src/routes/workspaces/archive.rs)・[削除](../../crates/workspace-manager/src/workspace_manager.rs)・[確認手順](../operations/workspace-inspection.md)

## Workspace 累積トークン

累積値は取得できた provider 報告の部分集計であり料金、全消費の保証、現在の context 占有率ではない。同じ provider thread の resume を重複加算せず、異なる thread は合計する。未報告は unknown のままで、有効な報告があっても completeness は partial である。[集計](../../crates/db/src/models/workspace_tokens.rs)・[保存](../../crates/db/src/models/agent_runtime.rs)
