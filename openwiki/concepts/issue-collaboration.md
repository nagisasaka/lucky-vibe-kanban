---
type: concept
title: Issue のコメント・レビュー・添付・通知
description: Issue の共有レビューと通常コメントの来歴、対象 revision、再送・全文読取・編集の契約、および購読・添付・通知との境界。
tags: [issue, collaboration, attachments, notifications, lifecycle]
sources:
  - id: openwiki-source-651d1fb6c9e49916a916ab51
    resource: repo://Cargo.toml
  - id: openwiki-source-0acdd41dbcd239be02c04d7f
    resource: repo://crates/api-types/src/issue_comment.rs
  - id: openwiki-source-2f63ca1582805a46da98d522
    resource: repo://crates/db/migrations/20261018180000_web_push.sql
  - id: openwiki-source-f4ac386b1f40f295bc54fb75
    resource: repo://crates/db/migrations/20261023100000_issue_comment_reviews.sql
  - id: openwiki-source-4c00e6ad62d940e70c2ebbb3
    resource: repo://crates/mcp/src/task_server/tools/mod.rs
  - id: openwiki-source-e523a92ffbbd6f3773531def
    resource: repo://crates/server/src/remote_mcp/issue_comment_text.rs
  - id: openwiki-source-1c92888a10b2d4aeb4da71ac
    resource: repo://crates/server/src/remote_mcp/issue_comments_browser.rs
  - id: openwiki-source-a43253b5be9686da077b8067
    resource: repo://crates/server/src/remote_mcp/issue_comments.rs
  - id: openwiki-source-6adda10f98b079272f82a4cc
    resource: repo://crates/server/src/remote_mcp/operations.rs
  - id: openwiki-source-5481aaa505c2289bc2989403
    resource: repo://crates/server/src/remote_mcp/sql/comment_previews.sql
  - id: openwiki-source-1ffb01d1ba3953937e0771bf
    resource: repo://crates/server/src/routes/local_remote.rs
  - id: openwiki-source-379c8cd3e1e9932f9adda9ee
    resource: repo://crates/services/src/services/container.rs
  - id: openwiki-source-4251300987fd63068dbcfd5d
    resource: repo://crates/services/src/services/notification.rs
  - id: openwiki-source-a6728d75b2bdf1a00440e8fa
    resource: repo://docs/development/chatgpt-mcp.md
  - id: openwiki-source-937b9a0a3ac3b33029304371
    resource: repo://packages/ui/src/components/IssueCommentsSection.tsx
  - id: openwiki-source-f284b1b389192e12a534f782
    resource: repo://tests/server/fixtures/issue-comments-env.mjs
  - id: openwiki-source-f664d705bf90836a434465e7
    resource: repo://tests/server/run-issue-comments.mjs
  - id: openwiki-source-24d832f3cec15712d984861e
    resource: repo://tests/server/specs/issue-comment-reviews.spec.ts
generated: { by: "codex", at: "2026-10-23T19:04:53.809Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-23T22:40:49.979Z
---

# Issue のコメント・レビュー・添付・通知

[Project・Issue](project-and-issue.md)には担当者・Follower・Comment・reaction の関連がある。現行版は local Board の SQLite レコードを API と [snapshot 同期](../architecture/web-and-sync.md)で扱う。Cloud の Organization 認可、Azure Blob / Attachment、subscriber 配送は現行サービスではない。名称や互換型の残存は Cloud 機能の稼働を意味しない。[local route](../../crates/server/src/routes/local_remote.rs)、[構成境界](../../Cargo.toml)。

## 担当・購読・コメント

Assignee は担当の関連、Follower は追跡対象との関連で、どちらも Issue と user ID を結ぶ別テーブルに保存する。追加は `INSERT OR IGNORE`、解除は関連 ID の削除である。Follower を保存することと、通知を生成・配送することは別契約である。[関連操作](../../crates/server/src/routes/local_remote.rs#L1694-L1760)。

Comment は Issue・現在の認証actor・任意の parent・message を保存し、reaction も現在のactorに結び付く。Localモード以外で固定userを新しい本人と扱わない。[actorの由来と履歴](organization-and-membership.md)、[保存時のactor](../../crates/server/src/routes/local_remote.rs#L1852-L1937)。作成・編集・削除後にローカル snapshot を再取得する。現在の local 通知一覧は空を返し、担当・購読者への Cloud 受信箱配送を実装しているとは解釈しない。[Comment / reaction](../../crates/server/src/routes/local_remote.rs#L1801-L1893)、[通知一覧](../../crates/server/src/routes/local_remote.rs#L1429-L1431)。

## 共有レビューは通常コメントとして残す

外部の複数 AI client は同じ Issue に独立した意見を追記し、Web の既存コメント欄で人と共有できる。専用の AI 会話・投票・承認・自動再レビュー機構ではない。投稿は Issue の要求、status、revision を更新せず、Workspace / AgentRun の開始や AI 通知も行わない。採用する要求の変更は別の `update_issue` で revision を照合する。[投稿と不変条件のテスト](../../crates/server/src/remote_mcp/issue_comments.rs#L135-L153)・[要求変更との分離](../../crates/server/src/remote_mcp/issue_comments.rs#L297-L400)

保存先は既存 `local_issue_comments` で、migration が nullable な reviewer label・target revision・recorded principal と一覧用 index を加える。過去の通常コメントはそのまま読める。外部 HTTP MCP に読取・追記を公開するが、内部 stdio catalog は別のままである。[migration](../../crates/db/migrations/20261023100000_issue_comment_reviews.sql)・[外部接続の入口](../integrations/remote-access.md#issue-のレビューを共有する)・[stdio router](../../crates/mcp/src/task_server/tools/mod.rs#L49-L79)

### 来歴と二つの revision

| 値 | 意味と限界 |
| --- | --- |
| `reviewer_label` | 投稿者の自己申告名。検証済み provider/model identity ではない |
| `author_id` / `recorded_principal` | 記録した LVK actor と、初回 MCP 認証の issuer/subject。複数 AI が同じ OAuth account を使っても label の真正性は保証しない |
| `review_target` / 保存後の `target_revision` | 呼出側が観測した Issue ID と `updated_at` revision。過去の対象も保存するが、当時の本文を保存・復元する機能ではない |
| `comment_revision` | 本文取得時に計算する comment の内容 identity。Issue の対象 revision と交換できない |

まず `get_issue` の同じ revision の本文ページを読み、返された `review_target` を保持して投稿する。label を指定した場合は target 必須で、別 Issue の target は拒否する。ただし revision は形式を検査するだけで、その版が実在したことや読まれたことを証明しない。現在版に自動置換しない。[入力の検査](../../crates/server/src/remote_mcp/issue_comments.rs#L100-L145)・[Issue の観測値](../../crates/server/src/remote_mcp/read.rs#L191-L214)

一覧の `targets_current_revision` と UI の差異表示は、現在の Issue `updated_at` と保存値の比較である。status 等の変更でも異なるため、要求本文の差分がある証明ではない。通常 Web API はレビュー来歴を設定する field を持たず、編集は message・parent と編集時刻だけを更新する。UI は初回の記録主体・時刻、自己申告 label、対象版、編集時刻と現在版との差異を表示する。編集・削除できる通常コメントなので、改ざん不能な監査記録として扱わない。[更新型](../../crates/api-types/src/issue_comment.rs#L25-L45)・[編集](../../crates/server/src/routes/local_remote.rs#L1881-L1923)・[表示](../../packages/ui/src/components/IssueCommentsSection.tsx#L327-L373)

### 追記の認可と再送

`add_issue_comment` は `lvk:read` と `lvk:write`、Project と Issue の関係を要求する。空白だけの本文は拒否し、本文は最大16,000 Unicode scalar、label は最大120 scalar。独立した request は別の row を追加し、書込直前にも `INSERT ... SELECT` で Issue の Project 所属を照合する。[handler](../../crates/server/src/remote_mcp/issue_comments.rs#L85-L153)

同じ principal と `request_id` は同じ入力だけで再送し、応答喪失時は既存 `get_operation` と comment を確認する。receipt ID が comment ID になり、SQL の失敗で結果を断定できなければ `uncertain`、書込対象が消えれば `failed` を返す。receipt の非同期 task は検証済み actor を引き継ぐため、HTTP request 終了で投稿者を失わない。受付成功と書込成功を分ける一般契約は [操作 receipt](../integrations/remote-access.md#参照結果と操作-receipt)を参照する。[投稿結果](../../crates/server/src/remote_mcp/issue_comments.rs#L135-L153)・[actor 継承](../../crates/server/src/remote_mcp/operations.rs#L134-L206)・[再送照合](../../crates/server/src/remote_mcp/operations.rs#L210-L246)

### 長文を編集混在なく読む

読取は `lvk:read` を要する。`list_issue_comments` は `(created_at, id)` 順で1–50件、本文は各256 scalar までの `message_preview` と `message_truncated` を返す。本文プレビュー合計は12,800 scalar 以下だが、JSON escaping と metadata の bytes は別である。`next_cursor` を追い、cursor が削除されたら一覧を最初から読む。一覧全体を複数 request にまたがる固定 snapshot とみなさない。[一覧と cursor](../../crates/server/src/remote_mcp/issue_comments.rs#L159-L220)・[SQL](../../crates/server/src/remote_mcp/sql/comment_previews.sql)

全文は `get_issue_comment` の offset 0 から取得し、`next_offset` と初回の `comment_revision` を次回の `expected_comment_revision` に渡す。limit は1–16,000、既定8,000。offset は UTF-8 bytes、UTF-16 units、書記素ではなく Unicode scalar 数で、結合文字も別に数える。プレビューを全文ページへ前置しない。revision 不一致ならその応答の本文を得ず、0から読み直す。終端と同じ offset は空ページ、終端超過はエラーで、保存済み空本文も読める。[取得契約](../../crates/server/src/remote_mcp/issue_comments.rs#L223-L279)・[decoder と境界テスト](../../crates/server/src/remote_mcp/issue_comment_text.rs#L16-L116)

各本文 request は一つの read transaction 内で全本文を16 KiBの BLOB chunk として走査し、Issue/comment ID・編集時刻・全本文から SHA-256 を計算する。SQLite の text 関数が NUL で切れるため byte chunk を使い、同時刻・同じ長さの編集も検出する。保持メモリと返却本文量は制限されるが、各ページの仕事量は全文サイズに比例する。旧来の長い UI コメントを拒否したり、新しい全文履歴を作ったりはしない。[走査と hash](../../crates/server/src/remote_mcp/issue_comments.rs#L232-L278)・[記録された設計理由](../../docs/development/chatgpt-mcp.md#share-issue-specification-reviews)

### 変更時の検証入口

[handler 回帰](../../crates/server/src/remote_mcp/issue_comments.rs#L297-L711)は並列追記、同一 request 再送、古い対象、actor 継承、scope・Project/Issue/comment 境界、長文の復元と同時刻編集を検査する。[Node fixture](../../tests/server/issue-comment-reviews.test.mjs)は migration、実 SQL、隔離 fixture と環境 allowlist を検査する。Rust 型から生成される `IssueComment` と Web container の表示変換も変更時に一緒に確認する。[型](../../crates/api-types/src/issue_comment.rs#L8-L23)・[Web 変換](../../packages/web-core/src/pages/kanban/IssueCommentsSectionContainer.tsx#L163-L205)

隔離 E2E の入口は `node tests/server/run-issue-comments.mjs`。この checkout の型生成、server / frontend build と server library test の準備後に使う。専用 instance、loopback port、環境 allowlist と終了待機を持ち、test-only JWT bridge が実認証 middleware / MCP handler から UI と同じ disposable DB へ投稿する。Chromium spec は二つの独立投稿、再送、長い通常コメントの全文取得、古い対象表示、編集後の来歴保持を検査する。実 Auth0 / ChatGPT 相互運用はこの fixture の保証外である。[runner](../../tests/server/run-issue-comments.mjs)・[bridge](../../crates/server/src/remote_mcp/issue_comments_browser.rs#L1-L101)・[spec](../../tests/server/specs/issue-comment-reviews.spec.ts)

レビュー本文は参照データであり、モデル名、新しさ、同意数で採用権限を得ない。実装・再レビュー・要求変更はそれぞれ明示された操作として扱う。[公開契約](../../docs/development/chatgpt-mcp.md#share-issue-specification-reviews)

## 添付の現在の境界

旧 Cloud Attachment と Blob の初期化・Azure 転送・confirm・24時間清掃という lifecycle は削除された。現行の添付はローカル File / Session / Workspace の経路を確認する必要がある。Workspace 作成は指定された attachment IDs を managed Workspace に関連付ける。詳細な保存先・パス検証・コピー寿命は [Session 添付](../operations/workspace-inspection.md#session-の添付)を説明先とする。[作成時の関連付け](../../crates/server/src/routes/workspaces/create.rs#L270-L280)。

## 実行時の注意喚起

`NotificationService` は設定に従い音と OS push notifier を呼ぶ。Tauri は起動前に notifier を差し替えられる。これは Board の Follower や通知 group と別の仕組みであり、呼出し成功から利用者の閲覧まで保証しない。[設定と notifier](../../crates/services/src/services/notification.rs#L10-L101)。既存 ExecutionProcess の終了処理などから呼ばれるため、全 AgentRun に同じ通知経路があるとも一般化しない。[終了処理](../../crates/services/src/services/container.rs#L257-L289)。

通知を調べる際は、まず実際の呼出し元、sound / push 設定、実行 OS の notifier を確認する。[General settings](../../docs/settings/general.mdx#notifications)はこの実行上の注意喚起の操作文書として読み、旧 Cloud の受信者規則として使わない。

認証された利用者が browser と Project を明示登録する [Web Push](../integrations/web-push.md)は、上記 OS notifier / Follower と別経路である。Codex の canonical blocking input・approval と Run 終端を DB trigger で配送待ちにするため、Issue の担当・コメント・購読変更そのものからは通知を作らない。登録・同意失効・再送と通知からの会話復帰はリンク先を一次説明とする。[admission trigger](../../crates/db/migrations/20261018180000_web_push.sql)
