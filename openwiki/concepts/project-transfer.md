---
type: guide
title: Project transfer
description: SQLite 移送の対象、手順、原子性と履歴の限界。
tags: [project, transfer, sqlite, privacy, history]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-24T08:19:31.399Z
sources:
  - id: openwiki-source-eec300ba0fd0bf7307068218
    resource: repo://crates/db/src/project_transfer.rs
  - id: openwiki-source-b7caca432ab4ead6087a0935
    resource: repo://packages/web-core/src/shared/components/ProjectTransferPanel.tsx
  - id: openwiki-source-00110cdcfec890895b99690a
    resource: repo://packages/web-core/src/shared/dialogs/settings/settings/RemoteProjectsSettingsSection.tsx
generated: { by: "codex", at: "2026-11-24T08:19:31.399Z" }
---

# Project の SQLite 移送

Project 移送は [Project と Issue](project-and-issue.md) の一部の LVK 所有記録を単一 SQLite に export し、別配置へ新しい Project として追加する機能である。インスタンス全体の backup、Git の移送、実行中 process や native Session の再開ではない。既存 Project・Issue・Repo 設定を上書きしない。[DB core](../../crates/db/src/project_transfer.rs)・[操作の段階](../quickstart.md#操作の段階)

## 対象と除外

固定 allowlist は Project の名前・時刻、status、Issue 本文・名前付き版・コメント・内部関係・Tag、対応に必要な Repo identity と表示名を含む。内部 ID は新しい ID に対応付け、Issue 番号を保ち、`simple_id` は明示した新 Project key を使う。外部への関係や親・返信等は除外数に記録し、残った参照が不正なら拒否する。[snapshot](../../crates/db/src/project_transfer.rs)

Notes、TODO、Workflow 定義、担当者・Follower・reaction、Git object・branch・worktree・未 commit 差分、Repo path・script、認証、環境、provider の native session や memory は含まれない。添付 bytes も含めず、Project と選択された完了 Workspace の添付は名前・size の欠落 summary を持つ。Issue のみに紐づく添付は現行 summary の収集対象外であり、一覧の完全性を保証しない。[coverage](../../crates/db/src/project_transfer.rs)・[移送詳細](../../crates/db/PROJECT_TRANSFER.md)

会話本文は既定 OFF で明示 opt-in が必要である。所属と terminal 状態、最新 Attempt、canonical projection と state の identity・version・cursor が整合する完了 Run のみを対象とし、user message と final assistant message の本文を残す。thinking、tool 入出力、生 event、旧 raw log は含まない。自由記述には秘密や旧 path があり得るため、共有前に内容を確認する。完全な sanitization は保証しない。[履歴選別](../../crates/db/src/project_transfer/history.rs)

DB 確定 Manifest は会話 opt-in と独立して、対象 Run・Workspace・登録 Repo と payload が一致するものを選ぶ。import 後は専用の read-only archive に置き、実行可能な Run、provider Session、queue や process registry を復元しない。移送先 Repo の canonical Manifest / Wiki 消費 stream にも再公開しない。[履歴](../../crates/db/src/project_transfer/history.rs)・[Repository Memory](repository-memory.md)

## 移送する

1. 移送先に必要な Git Repo を別途準備し、LVK に登録する。必要な commit や Git の移送は別の操作であり、この機能は clone、commit、push をしない。
2. 移送元のプロジェクト設定で Project を選び、「Project の移送」（英語では Project transfer）から export する。会話を含める場合だけ明示し、対象外データと欠落添付を確認する。
3. 移送先のプロジェクト設定の同じパネルでファイルを選んで preview し、件数、除外参照、欠落と coverage を確認する。一意の新 Project key と、各元 Repo に対応する登録済み Repo を一対一で指定する。既存 Project がなくても設定画面から import を始められる。
4. preview と同じ bytes を import する。ファイルを選び直したら preview をやり直す。SHA-256 は bytes の一致確認であり真正性署名ではない。
5. 成功後に Project 一覧の更新を待ち、追加された Project と専用履歴を確認する。更新が遅くても commit 済み import を失敗と判断して再実行しない。[UI](../../packages/web-core/src/shared/components/ProjectTransferPanel.tsx)・[API](../../crates/server/src/routes/project_transfer.rs)

移送入口は利用頻度の低い操作を日常の右サイドバーから分けるため、プロジェクト設定に集約している。右サイドバーには重複した入口を残さない。export と移送履歴の閲覧は選択した Project に結び付き、import は Project 未選択でも使える。import は選択中の Project を上書きせず、新規 Project を追加する。[設定への接続](../../packages/web-core/src/shared/dialogs/settings/settings/RemoteProjectsSettingsSection.tsx)・[移送パネル](../../packages/web-core/src/shared/components/ProjectTransferPanel.tsx)・[採用仕様](../../docs/changes/2026-11-24-LOCAL-487-195ec158.md)

設定で Project の選択を変えるとパネルを再生成し、選択ファイル・preview・履歴などのローカル状態をリセットする。操作途中で切り替えた場合は対象とファイルを再確認する。入口移設は既存APIの権限、確認、Repo対応付けやデータ形式を変更しない。

## 形式と失敗

形式 `lvk-project-issues` version 2 は固定 `package` table の一行 JSON で、version 1 も読める。ファイルは32 MiB、JSON は16 MiBまで。未知 field/version、実行可能 schema、参照不正・cycle、symlink・非通常ファイル・過大入力を拒否する。read-only immutable、trusted schema 無効、query-only で開き、入力 schema SQL を移送先で実行しない。[入力検証](../../crates/db/src/project_transfer.rs)

Export は一つの source read transaction から snapshot を作り、既存出力を置換しない。Import は全検証後に `BEGIN IMMEDIATE` で新 Project、archive、receipt を一括保存する。途中や最終制約失敗は receipt を含め rollback する。export UUID または canonical 内容 hash の重複は拒否し、receipt は Project 削除後も残る。[transaction](../../crates/db/src/project_transfer.rs)

Import した本文・版・コメントと archive は旧 URL・画像・path を解決しない literal 表示を使う。判定の loading/error 中も安全側に保つ。[editor](../../packages/web-core/src/shared/components/ProjectIssueEditor.tsx)・[archive](../../packages/web-core/src/shared/components/TransferArchiveText.tsx)

[DB fixture](../../crates/db/tests/project_transfer.rs) は再 ID、除外、重複、破損と rollback、[API tests](../../crates/server/src/routes/project_transfer/tests.rs) は認証・bytes 束縛を検査する。合成 HTTP の browser fixture は稼働 DB の移行や実 provider の再現を証明しない。[browser fixture](../../tests/project-transfer/README.md)
