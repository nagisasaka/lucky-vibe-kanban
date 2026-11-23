---
type: integration
title: GitHub review
description: Repository 資格情報、push、PR と観測の境界。
tags: [github, pull-request, credentials]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-27c2433bab843f01a51418b1
    resource: repo://crates/server/src/routes/workspaces/pr.rs
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# GitHub 認証・PR とレビュー

現在の PR 操作は Workspace と GitHostService を使う。旧 Cloud Review worker や webhook callback を現行の開始条件としない。[操作の段階](../quickstart.md#操作の段階)を区別する。[PR route](../../crates/server/src/routes/workspaces/pr.rs)

## リポジトリ別 PAT の所有と管理

Settings → Repositories で登録 Repo の remote／push URL に対応する GitHub.com repository を選ぶ。credential は正規化した owner/repository 単位で instance 内に共有され、Workspace や member 単位ではない。管理者が fine-grained PAT を保存・置換・削除し、一覧は configured のみ返す。保存時の形式検査や接続確認は push 権限の証明ではない。保存先と backup は秘密を含み、削除は GitHub 側の revoke ではない。[API](../../crates/server/src/routes/github_credentials.rs)・[保存](../../crates/utils/src/github_credentials.rs)

PR には実行環境に `gh` が必要である。対象 token が未登録なら同じ実行 account の system 認証を使う。登録 token が壊れている場合や認証拒否時に別 account に黙って fallback しない。LVK の Git／gh wrapper による対象限定の注入は任意の agent shell に配布する契約ではない。[Git wrapper](../../crates/git/src/cli.rs)・[gh wrapper](../../crates/git-host/src/github/cli.rs)

## Push、PR、観測

PR 作成は source と Wiki の必要条件を確認し、branch を push して remote PR を作り、local record を保存する。これらは一つの transaction ではない。応答や local 保存が失敗した場合、重複作成前に remote の既存 PR と branch を確認する。push URL と PR base の remote は異なり得る。[処理](../../crates/server/src/routes/workspaces/pr.rs)・[Memory](../concepts/repository-memory.md)

登録 Repository の明示 push は別操作で、preview の source SHA、destination、remote SHA と履歴を確認してから実行する。応答不明時は同じ request ID の receipt と remote を確認し、別 ID で盲目的に再送しない。remote 観測は誰が push したかや local 統合検証の証明ではない。[push service](../../crates/server/src/repository_push.rs)・[Git 操作](../../crates/git/src/repository_push.rs)

PR monitor の merged 観測は Workspace の自動 archive、正式 Integration の検証、Issue 完了、Wiki 更新を意味しない。Agent review、[正式 Integration](../concepts/formal-integration.md) の判断、[OpenWiki 独立レビュー](../operations/openwiki-maintenance.md) はそれぞれ対象と完了条件が異なる。[monitor](../../crates/services/src/services/pr_monitor.rs)
