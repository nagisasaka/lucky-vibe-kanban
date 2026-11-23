---
type: concept
title: Project と Issue
description: 仕事の識別、本文版、Board 状態と archive。
tags: [project, issue, kanban, lifecycle]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-5c693a62e6ccf09ccb60592f
    resource: repo://crates/db/src/models/agent_runtime_kanban.rs
  - id: openwiki-source-db663c02905b9785eff4f4af
    resource: repo://crates/server/src/routes/local_remote/issue_versions.rs
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# Project・Issue と Kanban

[Project](project.md) は関連する仕事、状態列、分類を集める単位である。[Issue](issue.md) は達成したい仕事で、Board の Card として表示する。作業場所は [Workspace](workspace.md)、会話と実行は [Session・Agent Run](session-and-agent-run.md) が持ち、一つの Issue に複数の実施を関連付けられる。[schema](../../crates/db/migrations/20260427000000_local_kanban.sql)・[操作の段階](../quickstart.md#操作の段階)

## Project と仕事の識別

Project の Repo 群は新規 Workspace の作成プリセットであり、既存 Workspace の branch や Repo 構成を所有しない。作成時に選んだ Repo と target branch は Workspace に保存する。プリセットを後で変更しても既存作業場所は変わらない。[Workspace 作成](../../crates/server/src/routes/workspaces/create.rs)・[Workspace](workspace.md)

Issue の永続 identity は UUID である。表示用 `simple_id` は Project key と Project UUID ごとの高水位番号から作る。allocator は caller の `BEGIN IMMEDIATE` 内で番号を取得し、Issue 作成と一緒に commit する。rollback された未公開予約は再利用できるが、commit 済み番号は Issue 削除で戻らない。移行前に削除済みだった番号の全履歴は復元できない。[allocator](../../crates/db/src/models/issue_number.rs)・[migration](../../crates/db/migrations/20261027090100_issue_number_high_water.sql)

Priority、Tag、担当者、Follower と親子・関連・blocking は別の情報である。親は同じ Project に属し、自己参照と親子循環を拒否する。`related` は逆向き重複を同一関係として扱い、`blocking` は方向を持ち循環を許す。これらは [Workflow の制御 edge](workflow-attempt.md) と同じものではない。[関係](../../crates/server/src/routes/local_remote/issue_relations.rs)・[共同作業](issue-collaboration.md)

## 本文版と現在仕様

`issue_versions` は名前付きで編集可能な本文版を保存する。明示的なコピーまたは空版作成で版を増やし、通常保存は同じ版を更新する。閲覧タブと `current_version_id` は別で、現在版は今後の起動入力や統合受付に使う仕様を示す。`local_issues.description` は現在版の互換コピーである。[版 schema](../../crates/db/migrations/20261111220000_issue_versions.sql)・[版 API](../../crates/server/src/routes/local_remote/issue_versions.rs)

版編集は版 revision、現在指定は専用の `current_version_revision` と対象版 revision を照合する。古い更新を別の現在版へ適用しないためである。コメントの review target や統合 revision は別の値である。版操作だけで Run は開始せず、既送信 prompt や受付済み統合 snapshot は書き換えない。[版 API](../../crates/server/src/routes/local_remote/issue_versions.rs)・[レビュー](issue-collaboration.md)

## Board の状態と実行

Status は Project の列であり、process の状態ではない。通常 Issue は、実開始の証拠を持つ最新 Attempt が Running になったとき標準可視列から In Progress に進む。開始前の失敗や Workspace の作成だけでは進まない。成功後は所有 token を保ち、他の関連 Run が活動していなければ In Review へ進む。Agent の成功だけで Done にしない。[同期](../../crates/db/src/models/agent_runtime_kanban.rs)

最新は関連 Workspace の実 process 開始時刻で選び、同時刻の UUID は決定的 tie-break である。手動 status 指定は自動同期の owner を解除し、旧 event の再送で手動変更を戻さない。新しい実開始は再び owner を得られる。独自名や hidden 列、必要な可視移動先がない場合は推測して移動しない。[同期と owner](../../crates/db/src/models/agent_runtime_kanban.rs)・[手動変更](../../crates/db/migrations/20261020190000_kanban_lifecycle.sql)

Auto-merge や OpenWiki が生成する専用 Issue は通常 Agent 同期から除外し、生成 provenance と製品の状態 receipt から投影する。Agent 成功だけで公開済みとはならない。公開、Card Done と専用 Workspace の archive は別の結果である。[生成状態](../../crates/db/migrations/20261027090000_generated_issue_status.sql)・[Auto-merge](formal-integration.md)

Team / Personal は同じ Issue 集合に対する表示 preset であり、権限境界ではない。検索・filter・表示 sort は view 設定で、手動並べ替えや列間移動は共有 Issue の rank/status を変更する。`hideBlocked` は blocker が最後の可視列または hidden 列にあるかを使うため、列順と非表示設定は表示上の解決判定にも影響する。[view](../../packages/web-core/src/shared/stores/useUiPreferencesStore.ts)・[filter](../../packages/web-core/src/features/kanban/model/hooks/useKanbanFilters.ts)

検索の直接一致は title・simple ID・番号で行う。関連展開は読み込み済み同一 Project の `related` を双方向にたどり、親子や blocking を勝手に展開せず、欠落端点を追加取得しない。[検索](../../packages/web-core/src/features/kanban/model/searchKanbanIssues.ts)

## Archive、削除、実施

Issue の `archived` は通常表示から外す属性で、status や Workspace archive と独立する。既存の関連実行は続けられるが、新規 Workspace・候補・Workflow 主 Workspace と新しい link は拒否される。解除後は再び可能になる。[archive schema](../../crates/db/migrations/20261025150000_issue_archive.sql)

Archive の確認は Issue とリンク集合、各 Workspace と任意の Workspace archive 指定を固定する。対象 Workspace を含める場合は実 process と idle を再確認し、履歴と Git branch を保ちながら worktree を削除対象にする。確認 token は caller と対象に束縛され10分で失効する。物理変更後の DB 失敗は rollback だけでファイル復元を証明できず、結果不明として receipt と実状態を調べる。[archive](../../crates/server/src/routes/local_remote/issue_archive.rs)・[Workspace archive](workspace.md)

Issue の物理削除は archive と異なり復元操作ではない。単一 Issue の UI では直接リンクした Workspace の削除を任意に選べ、既定は OFF である。選択時は保護条件を検査し DB commit 後に物理 cleanup を記録する。外部 Workspace は除外し、失敗や結果不明を成功と扱わない。既存の複数削除や remote MCP はこの任意削除を使わない。[削除](../../crates/server/src/routes/local_remote/issue_delete.rs)

通常の link は孤立した interactive Workspace を対象とし、既存 link を暗黙に付け替えない。unlink は関連の削除であって作業ファイルの削除ではない。実施から検証・統合までの順序は[作業フロー](../workflows/task-to-integration.md)に置く。[link](../../crates/server/src/routes/workspaces/links.rs)

Project の [TODO list](project-todos.md) と [Notes](design-notes.md) は Issue と独立する。Project 移送はインスタンス全体や Git の backup ではなく、選択した LVK 所有記録の追加 import である。[移送](project-transfer.md)

採番、状態投影、archive の DB・隔離 fixture は対応する競合と回復境界を検査する。稼働 DB、実 provider や物理端末の受入をこの説明から推定しない。[採番 tests](../../crates/db/src/models/issue_number_tests.rs)・[状態 tests](../../crates/db/src/models/agent_runtime_kanban_tests.rs)・[archive tests](../../tests/issue-archive/VALIDATION.md)
