---
type: concept
title: Issue のコメント・レビュー・添付・通知
description: Issue の共同作業、レビューの来歴と revision、再送、添付と通知の境界を説明する。
tags: [issue, collaboration, review, attachments]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-22T05:31:45.518Z
sources:
  - id: openwiki-source-0acdd41dbcd239be02c04d7f
    resource: repo://crates/api-types/src/issue_comment.rs
  - id: openwiki-source-a43253b5be9686da077b8067
    resource: repo://crates/server/src/remote_mcp/issue_comments.rs
generated: { by: "codex", at: "2026-11-22T05:31:45.518Z" }
---

# Issue のコメント・レビュー・添付・通知

[Issue](project-and-issue.md) には担当者、Follower、コメントと reaction を関連付けられる。共同レビューやコメントの取得・編集を変更するときは、意見の記録と要求の変更、通知の配送を区別する。[local API](../../crates/server/src/routes/local_remote.rs)・[操作の段階](../quickstart.md#操作の段階)

## コメントと共有レビュー

Assignee と Follower は Issue と user の別々の関連である。Follower の保存は通知配送の保証ではない。Comment は Issue、現在の認証 actor、任意の親と本文を持ち、reaction も actor に結び付く。旧 Cloud 通知 inbox の空状態と [Web Push](../integrations/web-push.md) は別である。[local API](../../crates/server/src/routes/local_remote.rs)・[通知 hook](../../packages/web-core/src/shared/hooks/useNotifications.ts)

外部 MCP の共有レビューは既存の通常コメントとして保存する。Issue の要求・status・revision を変更せず、Agent Run の開始や自動再レビューもしない。レビューは意見であり、モデル名、新しさや賛同数は採用権限を与えない。要求変更は別の Issue 更新である。[MCP comments](../../crates/server/src/remote_mcp/issue_comments.rs)

`reviewer_label` は自己申告であり、検証済み model identity ではない。`recorded_principal` は記録した認証主体で、同じ OAuth account を共有する client は同じ所有者となる。`review_target` は観測した Issue の `updated_at` revision であり、過去本文の保存や実際に読んだ証明ではない。現在 revision との差は status 等でも生じ、本文差分の証明にはならない。[入力と一覧](../../crates/server/src/remote_mcp/issue_comments.rs)・[コメント型](../../crates/api-types/src/issue_comment.rs)

コメントは Issue 全体に属し、名前付き本文版へ移し替えない。現在仕様を切り替えても保存済み review target を現在版へ付け替えない。[本文版](project-and-issue.md)・[版 API](../../crates/server/src/routes/local_remote/issue_versions.rs)

## 読取・編集・再送

一覧は時系列の preview を最大50件、各256 Unicode scalar まで返す。全文は `get_issue_comment` の offset 0 から取得し、後続ページに初回の `comment_revision` と `next_offset` を渡す。不一致なら0から読み直す。offset は bytes や UTF-16 units ではなく Unicode scalar である。preview を全文ページへ前置しない。複数 request の一覧全体は固定 snapshot ではない。[tool contract](../../crates/server/src/remote_mcp/issue_comments.rs)・[text decoder](../../crates/server/src/remote_mcp/issue_comment_text.rs)

外部 HTTP MCP の編集は read/write scope、Project・Issue・comment の所属、保存済み issuer/subject の一致と `expected_comment_revision` を要求する。本文と更新時刻だけを変え、author、parent、label と review target を保持する。通常 Web 編集の契約は別であり、コメントは編集・削除可能なため改ざん不能な監査記録ではない。[編集](../../crates/server/src/remote_mcp/issue_comments.rs)・[Web 更新型](../../crates/api-types/src/issue_comment.rs)

追記・編集の再送は同じ principal、request ID と同じ入力を使う。応答を失ったら operation と現在のコメントを確認する。`failed` と `uncertain` は異なり、結果不明のまま新しい ID で重複投稿しない。[operation](../../crates/server/src/remote_mcp/operations.rs)・[外部 MCP](../integrations/remote-access.md)

## 添付と通知

現行添付は local File・Session・Workspace の経路であり、旧 Cloud Blob の lifecycle ではない。保存先、パス検証とコピーの寿命は[Workspace の添付](../operations/workspace-inspection.md)を参照する。[作成](../../crates/server/src/routes/workspaces/create.rs)

OS notifier とブラウザーの Web Push は別の経路である。OS 通知の受信先は server 実行環境に依存し、呼出し成功は人が読んだ証明ではない。Web Push は browser と Project の明示登録を使い、Agent Run の特定 event を対象とする。Issue の Follower やコメントを保存しただけで Cloud inbox 配送が成立するわけではない。[通知 service](../../crates/services/src/services/notification.rs)・[Web Push](../integrations/web-push.md)

コメントの scope・所有者・再送・長文の fixture と隔離 browser 試験は、実 Auth0 / ChatGPT との相互運用を証明しない。[handler tests](../../crates/server/src/remote_mcp/issue_comments.rs)・[browser spec](../../tests/server/specs/issue-comment-reviews.spec.ts)
