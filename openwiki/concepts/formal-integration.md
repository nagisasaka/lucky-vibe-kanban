---
type: concept
title: Auto-merge
description: 固定 source と target の統合、検証、Wiki と公開。
tags: [integration, git, validation, lifecycle, recovery]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-595d5d0308d670e38ef27fbf
    resource: repo://crates/server/src/routes/integrations/admission.rs
  - id: openwiki-source-5720d865d1c837f0e6b7dab7
    resource: repo://crates/server/src/routes/integrations/conversation.rs
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# Auto-merge の選択・検証・公開

Auto-merge は一つの Repository の local target に、採用した複数の [Card](project-and-issue.md) と [Workspace](workspace.md) の commit を統合する操作である。専用 Issue・Workspace・Session を作り、同じ Codex 会話が候補作成、競合修復、検証判断と有効な Wiki 更新を担当する。Host は証拠、writer の収束と Git 公開を確認する。[受付](../../crates/server/src/routes/integrations/admission.rs)・[実行](../../crates/server/src/routes/integrations/conversation.rs)

受付、実行、完了、公開と内容受入は[別の段階](../quickstart.md#操作の段階)である。通常の作業からの操作順は[Issue から統合まで](../workflows/task-to-integration.md)を参照する。

## 採用するものと受付

各 Card について一つの採用 Workspace と expected commit を指定する。受付は Project・Issue の関連、Workspace が実際に持つ Repo と target branch、source HEAD の一致を確認する。DirectFolder は対象外である。採用されるのは固定 Git object であり、未 commit の作業や別 Workspace を暗黙に含めない。[source 検査](../../crates/server/src/routes/integrations/admission.rs)

受付時の target、Issue の現在版と本文、利用可能な Change Manifest 参照、追加指示、検証と Wiki の方針を固定する。同じ request ID の再送は元の選択と設定が一致する場合に既存結果を返す。外部 MCP では元入力全体の receipt も照合する。[受付](../../crates/server/src/routes/integrations/admission.rs)・[外部入口](../../crates/server/src/remote_mcp/launch.rs)

候補は専用 private branch に作る。受付 base と全 source が祖先として残り、期待 SHA と clean な作業木を満たす必要がある。競合の有無によらず同じ担当会話を継続し、旧 checkpoint adoption や source ごとの子 job を前提にしない。[candidate guard](../../crates/server/src/routes/integrations/conversation.rs)

## 検証の選択と証拠

Target の `.lvk/integration-validation.json` は受付時に参考コマンドとして固定される。必須の host workflow ではなく、担当者は差分、リスク、同じ SHA に結び付いた既存証拠とユーザーの指示から検証を選ぶ。実施と省略を分け、判断者・理由・残リスクを記録する。旧 SHA の pass を修復後の候補上の実行と呼ばない。[設定](../../.lvk/integration-validation.json)・[受付](../../crates/server/src/routes/integrations/admission.rs)

`verify` は expected clean SHA に対する管理 script を開始し、request ID、process、command、前後 SHA と結果を残す。同じ ID と同じ入力の再送は新しい process を起動しない。自己申告は host 実行の代わりにならず、exit 0 も意味的な網羅性の証明ではない。失敗を残し、修復後にはその候補に適用できる新しい証拠か、理由付きの省略判断が必要である。[検証と証拠](../../crates/server/src/routes/integrations/conversation.rs)

設定編集は登録 checkout の `.lvk/integration-validation.json` を更新し、保存自体は実行・stage・commit・push を行わない。`commands` は1〜32個の非空文字列で、明白な placeholder は拒否される。revision は branch、HEAD、ファイルの存在と内容に結び付く。保存失敗後も置換が起きていないとは断定せず、現在ファイルを確認してから再編集する。新規受付が対象 branch の内容を読むため、変更は通常の Git 手順で反映する。[設定 API](../../crates/server/src/routes/integration_validation.rs)

## Wiki と公開

Wiki が有効ならコード検証後、同じ候補 branch で OpenWiki lifecycle を完了し、Wiki だけを別 commit にする。コード SHA、Wiki SHA と最終候補を区別し、Wiki 後続 commit をコード試験済みと偽らない。初回 `openwiki_begin` の前に `prepare_wiki` でコード証拠と writer を照合し、原本 journal を固定する。生成 setup 差分の復元は host が証拠と identity を検証した上で行う。[準備](../../crates/server/src/routes/integrations/conversation.rs)・[Wiki 保守](../operations/openwiki-maintenance.md)

公開要求の受付は Git 適用ではない。担当 Run・子 process・script・writer の収束、Wiki 完了証拠、候補と検証を host が確認してから進む。実 target が完成候補の祖先なら完成候補を変更せず CAS で反映する。祖先でなければ候補と証拠を保持して未公開の In Review に止まり、自動 rebase や追従をしない。[公開](../../crates/server/src/routes/integrations/runtime.rs)・[Git publication](../../crates/git/src/publication.rs)

取消と publication intent は DB 上で排他するが、Git の ref・index・files と DB は一つの transaction ではない。中断後は保存済み intent と実 Git を照合する。適用済み候補を推測で再 merge せず、結果不明は回復対象として保持する。公開 intent 後の取消は Git undo ではない。[公開と回復](../../crates/server/src/routes/integrations/runtime.rs)

公開後の Card Done は別の条件付き処理である。採用 Workspace の現在 HEAD が固定 source と異なる、観測不能、または同 Workspace に非終端 Run がある場合、その Card の自動 Done は保留される。公開自体は取り消さない。生成 Issue の Done、Wiki receipt、専用環境の archive と物理 cleanup も各々の結果を確認する。[完了](../../crates/db/src/models/integration.rs)・[生成状態](../../crates/db/migrations/20261027090000_generated_issue_status.sql)

## 任意の Issue 本文アーカイブと回復

`issue_archive_enabled` は既定 OFF で Wiki 設定と独立する。ON で受付した場合、固定された採用 Issue の現在仕様本文を候補内の `docs/changes/` に保存する。コメント、会話、添付実体は対象外であり、保存資料は当時の意図であって現在の実装や実行許可の証明ではない。アーカイブ準備は検証・Wiki 準備前に行い、その commit を検証対象 SHA とする。[設定](../../crates/services/src/services/config/versions/v9.rs)・[準備と検証](../../crates/server/src/routes/integrations/issue_archive.rs)

中断後は同じ会話を明示的に再開できる。保持された編集・index・過去の失敗を勝手に reset や成功への書換えをせず、現在の operation receipt、writer と Git を確認する。Wiki では過去に監査された同じ所有者の run ID を引き継ぎ、監査欠損や未完了 writer を成功と扱わない。[会話](../../crates/server/src/routes/integrations/conversation.rs)・[Wiki proof](../../crates/server/src/routes/openwiki/completion.rs)

隔離 fixture は実 Git・SQLite と管理 command を使って競合、drift、取消や receipt 回復を検査するが、実 provider や公開認証済み UI の受入を証明しない。[integration tests](../../crates/server/src/routes/integrations/conversation_tests.rs)
