---
type: concept
title: 対話ヘルプ
description: 稼働版の公開 source に基づく質問と版固定読取。
tags: [help, source, mcp, distribution]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-d40201a53108a0c612f229ef
    resource: repo://crates/server/src/routes/help.rs
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# 稼働版に基づく対話ヘルプ

対話ヘルプは稼働版に対応する公開 source、Wiki と履歴を参照して LVK の一般的な使い方を説明する。既存 UI と外部 MCP は同じサービスを使う。質問から開発を自動開始せず、変更が必要なら通常の [Project・Issue・Workspace](project-and-issue.md) の流れへ進む。[help service](../../crates/server/src/routes/help.rs)・[操作の段階](../quickstart.md#操作の段階)

## 質問と会話

UI のガイドから質問を送ると、既定 coding agent の認証・利用条件が適用される。新規会話は asset directory の help 用 DirectFolder Workspace と Session を使い、Project を作らない。Git 内の作業場所は拒否するため、source 開発では `LVK_INSTANCE_DIR` を checkout 外に置く。[開始](../../crates/server/src/routes/help.rs)

会話は既存 Session の REVIEW 設定を使い、初回は Initial、継続は FollowUp である。継続では保存された executor と provider session を使う。受付は回答成功ではなく、実行中、待機、成功、失敗、取消、crash、監査失敗を区別する。画面を閉じても取消せず、削除は終端後に Workspace 削除へ委譲する。[サービス](../../crates/server/src/routes/help.rs)・[画面](../../packages/web-core/src/shared/components/InteractiveHelp.tsx)

REVIEW は強制 read-only sandbox ではない。ヘルプの目的は公開資料の調査・説明に限定し、質問だけで実装、build、deploy、再起動や新 Project 作成を始めない。利用者の Repo や秘密への包括許可にもならない。[help prompt](../../crates/server/src/routes/help.rs)

## 直接 source を読む

調査箇所が分かるときは外部 MCP の `query_repository` に `source: "lvk"` を指定する。Project ID は不要で、target 省略時は配布 commit、履歴は commit を明示する。任意 host path や Workspace mode を対象にする機能ではない。一般質問には `ask_help` を使い、返された Session を `get_help_conversation` で確認する。`ask_help` は冪等ではないため、結果不明時に盲目的に再送しない。[repository reader](../../crates/server/src/repository_read.rs)・[help tools](../../crates/server/src/remote_mcp/help.rs)・[外部アクセス](../integrations/remote-access.md)

## 版と配布の検証

Server は既定 `/opt/lvk/source`、または `LVK_SOURCE_PATH` の source を process ごとに検査する。既知の40桁 build revision、非 shallow 履歴、HEAD と binary revision の一致を要求し、欠落・不一致を明示エラーにする。起動時や質問時の fetch/pull は行わない。GitHub の最新 branch や変更中の checkout を稼働 binary と同一視しない。[検査](../../crates/server/src/bundled_source.rs)

配布 image は公開 commit と到達可能な祖先を同梱する。全 branch・tag は保証せず、同梱 source の編集は稼働 binary を変更しない。[同梱と検証手順](../../deploy/server/SOURCE.md)・[Dockerfile](../../Dockerfile)

実 image の受入は、許可された隔離環境で immutable digest、実行 UID、binary revision と source HEAD、祖先の log/diff、認証付き読取を確認する。保持 volume を使う更新、実 provider の質問・継続・取消、実 OAuth/MCP の scope と失敗も別々に確認する。Docker-free fixture や mock browser の成功をこれらの代わりにしない。秘密を証拠へ含めず、所有 test process の停止と cleanup を確認する。[配布検証](../../deploy/server/SOURCE.md)・[ヘルプ fixture](../../tests/help/README.md)

同梱 source の unit fixture は版不一致や shallow 履歴の拒否を検査するが、最終 image や実 provider の成功を証明しない。[tests](../../crates/server/src/bundled_source.rs)
