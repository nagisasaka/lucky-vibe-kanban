---
type: concept
title: Issue の添付・購読・通知
description: local Board の担当・購読・コメントと実行通知、Session 添付との境界。
tags: [issue, collaboration, attachments, notifications, lifecycle]
sources:
  - id: openwiki-source-651d1fb6c9e49916a916ab51
    resource: repo://Cargo.toml
  - id: openwiki-source-1ffb01d1ba3953937e0771bf
    resource: repo://crates/server/src/routes/local_remote.rs
  - id: openwiki-source-379c8cd3e1e9932f9adda9ee
    resource: repo://crates/services/src/services/container.rs
  - id: openwiki-source-4251300987fd63068dbcfd5d
    resource: repo://crates/services/src/services/notification.rs
generated: { by: "codex", at: "2026-10-16T12:21:28.631Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-16T12:21:28.631Z
---

# Issue の添付・購読・通知

[Project・Issue](project-and-issue.md)には担当者・Follower・Comment・reaction の関連がある。現行版は local Board の SQLite レコードを API と [snapshot 同期](../architecture/web-and-sync.md)で扱う。Cloud の Organization 認可、Azure Blob / Attachment、subscriber 配送は現行サービスではない。名称や互換型の残存は Cloud 機能の稼働を意味しない。[local route](../../crates/server/src/routes/local_remote.rs)、[構成境界](../../Cargo.toml)。

## 担当・購読・コメント

Assignee は担当の関連、Follower は追跡対象との関連で、どちらも Issue と user ID を結ぶ別テーブルに保存する。追加は `INSERT OR IGNORE`、解除は関連 ID の削除である。Follower を保存することと、通知を生成・配送することは別契約である。[関連操作](../../crates/server/src/routes/local_remote.rs#L1694-L1760)。

Comment は Issue・local user・任意の parent・message を保存し、reaction は Comment と local user に結び付く。作成・編集・削除後にローカル snapshot を再取得する。現在の local 通知一覧は空を返し、担当・購読者への Cloud 受信箱配送を実装しているとは解釈しない。[Comment / reaction](../../crates/server/src/routes/local_remote.rs#L1801-L1893)、[通知一覧](../../crates/server/src/routes/local_remote.rs#L1429-L1431)。

## 添付の現在の境界

旧 Cloud Attachment と Blob の初期化・Azure 転送・confirm・24時間清掃という lifecycle は削除された。現行の添付はローカル File / Session / Workspace の経路を確認する必要がある。Workspace 作成は指定された attachment IDs を managed Workspace に関連付ける。詳細な保存先・パス検証・コピー寿命は [Session 添付](../operations/workspace-inspection.md#session-の添付)を説明先とする。[作成時の関連付け](../../crates/server/src/routes/workspaces/create.rs#L270-L280)。

## 実行時の注意喚起

`NotificationService` は設定に従い音と OS push notifier を呼ぶ。Tauri は起動前に notifier を差し替えられる。これは Board の Follower や通知 group と別の仕組みであり、呼出し成功から利用者の閲覧まで保証しない。[設定と notifier](../../crates/services/src/services/notification.rs#L10-L101)。既存 ExecutionProcess の終了処理などから呼ばれるため、全 AgentRun に同じ通知経路があるとも一般化しない。[終了処理](../../crates/services/src/services/container.rs#L257-L289)。

通知を調べる際は、まず実際の呼出し元、sound / push 設定、実行 OS の notifier を確認する。[General settings](../../docs/settings/general.mdx#notifications)はこの実行上の注意喚起の操作文書として読み、旧 Cloud の受信者規則として使わない。
