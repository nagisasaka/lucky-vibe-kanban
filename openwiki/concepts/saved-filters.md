---
type: concept
title: 保存フィルター
description: Project共有の機械条件・自然文条件を管理し、現行Issueを評価する仕組みと状態・HTTP/MCP契約。
tags: [kanban, saved-filters, evaluation, mcp]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-24T02:54:42.940Z
sources:
  - id: openwiki-source-197f7022bc1d59dbb53ead2d
    resource: repo://crates/api-types/src/saved_filter.rs
  - id: openwiki-source-ceed2af7d9627956a38fb9ba
    resource: repo://crates/db/migrations/20261123000000_saved_filters.sql
  - id: openwiki-source-aec073797bcc6b43b7c362ce
    resource: repo://crates/db/src/models/saved_filter.rs
  - id: openwiki-source-a07fe658f0e63a34c99525fa
    resource: repo://crates/server/src/remote_mcp/saved_filters.rs
  - id: openwiki-source-b02a4d937f51a665dc209301
    resource: repo://crates/server/src/routes/local_remote/saved_filter_snapshot.rs
  - id: openwiki-source-a02cfa17823c0fd55d106715
    resource: repo://crates/server/src/routes/saved_filters/machine.rs
  - id: openwiki-source-3c467ccf60af806a1327ad3c
    resource: repo://crates/server/src/routes/saved_filters/mod.rs
  - id: openwiki-source-0da1783d644b088d236daeda
    resource: repo://crates/server/src/saved_filter_semantics.rs
  - id: openwiki-source-e6aa2afe51c506239753c56e
    resource: repo://crates/server/src/saved_filter_semantics/engine.rs
  - id: openwiki-source-f7410fb786e9a362721ca134
    resource: repo://packages/local-web/src/app/navigation/AppNavigation.ts
generated: { by: "codex", at: "2026-11-24T02:54:42.940Z" }
---

# 保存フィルター

保存フィルターはProjectで共有する名前付き条件である。対象Issueを固定保存するのではなく、機械条件と任意の自然文・補足contextから現行の対象集合を導出する。安定IDは名前や条件を編集しても変わらず、定義のrevisionが増える。[型と契約](../../crates/api-types/src/saved_filter.rs)

## 画面で使う

設定の「カンバンボードのフィルター」でProjectを選んで作成・編集し、ボードの選択欄から適用する。共有リンクは `/projects/{project}?savedFilter={id}` で、同じ定義を参照する。削除済み・参照できない定義は説明を表示し、通常表示へ戻せる。選択と解除は同一Project内のWorkspace・Sessionを維持する。[カンバン](kanban-board.md)・[管理画面](../../packages/web-core/src/shared/components/saved-filters/SavedFiltersManager.tsx)

選択中は一時検索・Team/Personal・タグなどによる追加絞り込みを外し、保存条件の結果を表示する。条件に合うhidden列や子Issueも対象になる。解除すると通常表示へ戻る。保存条件の変更は共有定義の更新であり、Issue本文・関係の編集ではない。[ボード表示](../../packages/web-core/src/features/kanban/ui/KanbanContainer.tsx)

## 機械条件の意味

異なる項目はAND、同一項目の複数選択は原則ORで結合する。空の選択は制限しない。タグはany/allを選べる。担当者は選択ユーザーと「未割当」をORで結合する。archiveの既定はactiveで、WorkspaceではなくIssueのarchiveを判定する。hidden列を自動除外する条件はない。[機械評価](../../crates/server/src/routes/saved_filters/machine.rs)

検索文字列はタイトル・本文への大文字小文字を区別しないリテラル部分一致で、空白もそのまま扱う。`simple_id` は別項目の完全一致である。通常のボード検索のタイトル・番号や関連Issue展開とは意味が異なる。

作成・更新期間のabsoluteはRFC3339の開始以上・終了未満で、少なくとも片側が必要になる。両側があれば開始は終了より前でなければならない。relativeは正の秒数で、サーバー現在時刻を基準に `[now-seconds, now]` を使う。画面のローカル日時入力は送信時に変換される。[入力画面](../../packages/web-core/src/shared/components/saved-filters/SavedFilterEditor.tsx)

関係条件は同じProjectの対象Issueを基準とし、対象自身を除く。parent/childrenは一段、descendantsは循環を防ぎながら子孫を辿る。relatedは双方向の一段。blockingは対象をブロックする側、blocked_byは対象によってブロックされる側である。他の条件に合わない中間Issueも関係探索に使う。[条件型](../../crates/api-types/src/saved_filter.rs)

## 自然文の評価と状態

サーバーはProjectの全Issue、状態、タグ、担当、関係を同じSQLite snapshotから読む。archive済みや機械条件に合わないIssueも自然文の文脈に含める。担当者名は実際の担当者の公開表示名であり、メールなどの私的プロフィールを送る用途ではない。[snapshot](../../crates/server/src/routes/local_remote/saved_filter_snapshot.rs)

自然文が空なら機械評価だけで完了する。自然文があれば既存のWorkspace・Session・Review Runを使う。実行場所はGit外のinstanceデータ配下 `saved-filter-evaluations/{workspace_id}` で、設定のexecutor profileを使う。専用daemonやクライアント画面の常時起動は不要だが、未使用フィルターを常時再評価する仕組みではない。[評価実行](../../crates/server/src/saved_filter_semantics.rs)

定義と証拠の変更でcacheを無効化し、generationによる比較更新で古い評価の上書きを防ぐ。局所変更と全体比較・時刻依存を区別して再評価し、モデル回答が必要な入力を全て覆うことを検査する。出力streamの先頭だけを完全結果とみなさない。[cache engine](../../crates/server/src/saved_filter_semantics/engine.rs)・[stream検査](../../crates/server/src/saved_filter_semantics/stream.rs)

| state | 意味 |
| --- | --- |
| unevaluated | 自然文の評価結果がまだない |
| evaluating | 評価が進行中 |
| complete | 評価が完了した |
| failed | 起動・provider・結果検査などが失敗した |

`stale` は別の属性であり、前回結果を表示している場合などに付く。未評価・初回評価中・失敗の空配列を、完了した0件として扱わない。Issue本体を読み込めていない間も確定一覧にしない。[表示状態](../../packages/web-core/src/shared/components/saved-filters/SavedFilterStatus.tsx)・[hook](../../packages/web-core/src/shared/hooks/useSavedFilter.ts)

結果GETはAIを起動せず、evaluateが起動を許す。同一入力の失敗をpollで無限再試行せず、明示的な `retry_failed` で再試行する。永続予約後にRunが見つからない状態は自動再送せず、120秒超で起動中断の失敗にする。通常のReviewとは異なる推論専用の権限制約は[provider境界](../integrations/agent-providers.md)を参照する。

## HTTPとMCP

HTTPの起点は `/api/projects/{project_id}/saved-filters`。GET一覧・POST作成、`/{id}` のGET・PUT・DELETE、`/{id}/results` と `/{id}/link` のGET、`/{id}/evaluate` のPOSTがある。UI・stdio MCP・remote MCPは共通の定義型と評価サービスを使う。[HTTP](../../crates/server/src/routes/saved_filters/mod.rs)・[MCP契約](../../crates/api-types/src/saved_filter_mcp.rs)

MCPの読取は `list_saved_filters`、`get_saved_filter`、`get_saved_filter_results`、`get_saved_filter_link`。変更・評価は `create_saved_filter`、`update_saved_filter`、`delete_saved_filter`、`evaluate_saved_filter` である。更新・削除には `expected_revision`、変更要求には再利用可能な `request_id` を渡す。同じID・同じ入力の再送は元の結果を返す。受付receiptは処理完了ではなく、`get_operation` と結果取得で確認する。[remote実装](../../crates/server/src/remote_mcp/saved_filters.rs)・[stdio転送](../../crates/mcp/src/task_server/tools/saved_filters.rs)

結果ページングは全体評価の後に適用する。既定100・上限500で、`next_offset` に従う。offsetが0より大きいときは直前の `evaluation_revision` を `expected_evaluation_revision` に渡す。欠落は400、評価変更は409となるので0から読み直す。取得済みページだけを母集団に評価してはならない。

読取には `lvk:read`、変更・評価には追加で `lvk:write` が必要で、MCPはProject許可範囲も検査する。HTTP変更は既存の認証・CSRF境界を通る。MCPリンクは設定された公開URLを反映できるが、他の利用者の画面を遠隔操作するものではない。[接続と権限](../integrations/remote-access.md)

## 永続化と検証範囲

[移行](../../crates/db/migrations/20261123000000_saved_filters.sql)は定義・request receipt・semantic cacheの表を追加する。既存Issueのデータを変換する処理ではない。フィルター削除はcacheを連動削除し、Issueは消さず、再送判定用receiptを保持する。[保存処理](../../crates/db/src/models/saved_filter.rs)

[移行互換試験](../../crates/db/src/migration_compat.rs)は既存データ保持と再適用、[サービス試験](../../crates/server/src/routes/saved_filters/tests.rs)は機械条件・ページ境界・隔離HTTP、[意味評価試験](../../crates/server/src/saved_filter_semantics/tests.rs)はcache・失敗・再試行を扱う。ブラウザfixtureは[編集](../../tests/saved-filters/editor.spec.ts)・[統合表示](../../tests/saved-filters/integration.spec.ts)・[遷移](../../tests/saved-filters/navigation.spec.ts)を扱う。fixture合格は実provider推論・外部認証・実サーバー全画面の受入完了を意味しない。実環境用の試験は別設定で明示的に実行する。
