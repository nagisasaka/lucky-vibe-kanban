---
type: concept
title: Project のSQLite移送
description: GitとネイティブSessionを分離し、Project所有記録と任意の完了会話を単一SQLiteで追加importする範囲、操作、原子性と限界を説明する。
tags: [project, transfer, sqlite, privacy, history]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-17T00:44:00.706Z
sources:
  - id: openwiki-source-eec300ba0fd0bf7307068218
    resource: repo://crates/db/src/project_transfer.rs
  - id: openwiki-source-bfeb31abecf9b81f73b98a39
    resource: repo://crates/db/src/project_transfer/history.rs
  - id: openwiki-source-1422e82323630001e2bf5970
    resource: repo://crates/server/src/routes/project_transfer.rs
  - id: openwiki-source-1302907005698efad4e6a002
    resource: repo://packages/web-core/src/shared/components/ProjectIssueEditor.tsx
  - id: openwiki-source-e09c19f6f13510573a8f7685
    resource: repo://packages/web-core/src/shared/components/TransferArchiveText.tsx
  - id: openwiki-source-3e18850057323970085dcd11
    resource: repo://packages/web-core/src/shared/hooks/useImportedProject.ts
generated: { by: "codex", at: "2026-11-17T00:44:00.706Z" }
---
# Project のSQLite移送

LOCAL-396の移送は、[Project・Issue](project-and-issue.md)のLVK所有記録を別の配置へ持ち運ぶための機能である。全体backup、Git移送、実行中処理やnative Sessionの再開ではない。既存のProject/Repoを上書きせず、新しいProjectとして追加する。[DBコア](../../crates/db/src/project_transfer.rs)・[API](../../crates/server/src/routes/project_transfer.rs)

## 運ぶ範囲と所有境界

Projectの名前・時刻、status、Issueの本文/版/コメント/内部関連/tag、対応に必要なRepo identityと表示名がallowlistである。内部IDは新しいIDへ一貫して対応付け、Issue番号を維持しつつsimple IDには移行先で明示した新しいProject keyを使う。Project外の関連/親/返信等は除外数へ計上し、残った参照閉包が不正なら拒否する。共有Repoやrepository全体の保存領域を丸ごとexportしない。[snapshotと検証](../../crates/db/src/project_transfer.rs)

Design Notes、Todo、Workflow定義、担当者/follower/reaction等の非allowlistデータは未実装・対象外である。全Project情報の完全移送とは扱わない。Git object・branch・worktree・未commit差分、Repoのpath/script、認証・環境・全体設定、HOME内のCodex/Claudeセッション/メモリも含まない。利用者が移行先Repoを別途準備し、未commit変更のcommitや必要なGit移送を手動で行う。機能はclone/commit/pushを実行しない。

添付実体は梱包しない。Projectと選択済み完了Workspaceの添付は元の名前・sizeによる欠落summaryだけを持ち、旧path/URL/hashや生きた添付関連を移植しない。曖昧なlegacy添付の帰属を推測しない。import済み欠落summaryはreceiptから再exportへ引き継ぐ。[添付選別](../../crates/db/src/project_transfer.rs)

## 完了履歴・Manifestと秘密

会話本文のexportは既定OFFで、利用者が明示opt-inする。Project–Issue–WorkspaceとSessionの帰属が揃い、Run/最新attempt/canonical projection/state JSONのidentity・version・終端status/cursorが一致した完了記録だけを採用する。active・stale・破損・曖昧な記録は除外する。[選別](../../crates/db/src/project_transfer/history.rs#L62-L107)

会話はcanonical user Message.contentと、final_outputのassistant Message.contentだけを残す。thinking、途中commentary、tool入出力、provider生event、request envelope、agent_turns.input_message、native参照をコピーしない。旧raw logを読んで不足を補うfallbackはない。構造化secret/authはallowlist外だが、自由記述に秘密や過去pathが残る可能性はある。共有前の内容確認が必要で、完全な本文sanitizationを保証しない。[本文選別](../../crates/db/src/project_transfer/history.rs)

Manifestは会話opt-inと独立して、採用可能な完了RunのIDとWorkspace Repo登録に一致するDB確定レコードだけを含む。payloadのRepo/Workspace/eventと任意task帰属も検証し、必要な歴史的Repoを参照閉包に追加する。保存するのはtypedな意味記録、時刻、commit ID、target branchと検証済み相対changed pathである。draft・復旧journal・Wiki中間物や共有Repo全体の履歴は含めない。[Manifest選別](../../crates/db/src/project_transfer/history.rs)・[正本の責務](repository-memory.md)

## import後に作るもの

完了会話とManifestは `project_transfer_history` のread-only表示archiveに保存し、Issue/Workspace/Session/Run/Repo参照を一貫して再ID化する。実行可能なagent_runs/turns、provider Session、queue、process registry、launch gateは挿入しない。既存の実行timelineを復元したようには表示せず、専用履歴欄から関連Issueへ遷移する。[archive保存と読取](../../crates/db/src/project_transfer/history.rs)

import Manifestは履歴として閲覧できるが、移行先Repoのcanonical Manifest公開・Wiki消費streamへ登録しない。古い成果証拠と新しいlocal Runを混同しないための境界である。

archive本文とimportしたIssue本文/版/コメントは文字列のまま閲覧・編集し、旧URL/画像/添付pathを解決しない。非importが確認できるまで、判定中やerror時もinertに保つ。元の本文文字列を無差別置換する方式ではない。[renderer](../../packages/web-core/src/shared/components/TransferArchiveText.tsx)・[editor](../../packages/web-core/src/shared/components/ProjectIssueEditor.tsx)・[判定](../../packages/web-core/src/shared/hooks/useImportedProject.ts)

## 操作と失敗時の扱い

1. 移行先で必要なRepoを準備し、認可されたLVKのProject utilitiesを開く。既存ProjectなしのimportはProject設定の移送欄から始められる。
2. export元の対象Projectで、会話を含めるかを明示選択してSQLiteをexportする。非allowlistデータと添付欠落の範囲を確認する。
3. 移行先でSQLiteファイルを選び、previewする。対象件数・除外参照・欠落添付とcoverageを読み、新しい一意Project keyと各元Repoの移行先登録Repoを対応付ける。
4. previewと同じbytesをimportする。ファイルを選び直すとpreviewは失効する。APIもSHA-256を照合し、変更bytesや不完全なRepo対応を拒否する。
5. 成功後はProject一覧の更新を待ち、新しいProjectと専用履歴を開く。refreshが遅い場合はreloadし、commit済みimportを再実行しない。

[移送UI](../../packages/web-core/src/shared/components/ProjectTransferPanel.tsx)・[client](../../packages/web-core/src/shared/lib/projectTransferApi.ts)。APIは既存の認証/CSRF境界内にあり、export/preview/history/reportにはlvk:read、importにはlvk:writeを要求する。レスポンスはno-storeで、SQLite内部pathやraw decode errorを利用者へ返さない。[認可とbytes束縛](../../crates/server/src/routes/project_transfer.rs)

exportは一つのsource read transactionからsnapshotを取り、privateな一時SQLiteを既存出力を置換せず公開する。importは全入力検証後、一つのBEGIN IMMEDIATEで新Project・履歴・receiptを挿入する。途中/final制約失敗ならreceiptまでrollbackし、既存Project/Repoを更新しない。export UUIDとcanonical内容hashの双方で重複を拒否し、Projectを削除してもreceiptを残す。拒否をkey変更やUUID差替えで迂回しない。[transaction](../../crates/db/src/project_transfer.rs)

## 形式と検証の限界

単一SQLiteは固定package tableの一行JSONを持ち、formatはlvk-project-issues、versionは2。Issue-onlyのversion 1も読める。未知version/field/schema、実行可能schema object、不正参照/cycle/identity、symlink/非regular/過大入力を拒否する。上限はファイル32MiB、JSON16MiB。read-only immutable・trusted_schema OFF・query_onlyで隣接WAL等に依存せず、入力schema SQLを移行先へ実行しない。SHA-256はpreviewとのbytes一致であり真正性署名ではない。[入力検証](../../crates/db/src/project_transfer.rs#L339-L420)

[DB往復試験](../../crates/db/tests/project_transfer.rs)は別配置SQLite/Git fixture、旧root削除後reopen、他Project排除、opt-in、秘密allowlist、再ID/リンク、欠落summary、破損入力とrollbackを扱う。[API試験](../../crates/server/src/routes/project_transfer/tests.rs)は認証/CSRFとbytes束縛を、[ブラウザfixture](../../tests/project-transfer/browser.mjs)は合成HTTPで実componentの操作・inert表示・明暗/狭幅を扱う。成功・失敗・SHA別再実行省略は[固定統合の記録](../operations/development.md#local-384396406339の固定統合検証)へ分けた。実provider/native履歴の完全再現、稼働DB移行や認証済み製品一体E2Eの成功とは扱わない。
