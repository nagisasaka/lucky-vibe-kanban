---
type: concept
title: 設計メモ・本文履歴と独立追記
description: Project の設計意図を本文と独立追記として残す契約。版管理、出典、競合、UIとMCPの応答境界、Project削除後の履歴保持を説明する。
tags: [design-notes, project, history, provenance, mcp]
sources:
  - id: openwiki-source-c2fc034438311f3aa02f5ee5
    resource: repo://crates/db/migrations/20261023110000_retain_design_notes_on_project_delete.sql
  - id: openwiki-source-27eb1d689068b93b5282d2b6
    resource: repo://crates/db/src/models/design_note_read.rs
  - id: openwiki-source-830e28055c4ae6f5464df312
    resource: repo://crates/db/src/models/design_note.rs
  - id: openwiki-source-5cc18853b4052cffae9ca81e
    resource: repo://crates/db/src/models/text_search.rs
  - id: openwiki-source-104d584b7b5ffa5973f60cde
    resource: repo://crates/mcp/src/task_server/tools/design_notes.rs
  - id: openwiki-source-4c00e6ad62d940e70c2ebbb3
    resource: repo://crates/mcp/src/task_server/tools/mod.rs
  - id: openwiki-source-2a0ddba6878eea1524264906
    resource: repo://crates/server/src/remote_mcp/design_notes.rs
  - id: openwiki-source-9e15762bf7a432b7605f8466
    resource: repo://crates/server/src/remote_mcp/issue_write.rs
  - id: openwiki-source-dbc5a93efbaa86561714413e
    resource: repo://crates/server/src/routes/design_notes.rs
  - id: openwiki-source-1ffb01d1ba3953937e0771bf
    resource: repo://crates/server/src/routes/local_remote.rs
  - id: openwiki-source-6380bb3355ad242288be2e3b
    resource: repo://packages/web-core/src/features/design-notes/model/designNoteApi.ts
  - id: openwiki-source-9fba069b7c8bc13326ad6d2a
    resource: repo://packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx
  - id: openwiki-source-f789c6c2f28c5d2d4ed0588f
    resource: repo://packages/web-core/src/shared/lib/listSearch.ts
  - id: openwiki-source-4917ca78c6137f6aa2fe5c81
    resource: repo://tests/design-notes/markdown-editing.mjs
generated: { by: "codex", at: "2026-11-06T03:28:31.315Z" }
verified:
  - by: openwiki/0.7.2
    at: 2026-11-07T08:56:27.082Z
---

# 設計メモ・本文履歴と独立追記

設計メモは [Project](project-and-issue.md) に複数置ける Markdown の記録で、「何を作りたいか、なぜそう考えたか」と、その後の判断・提案・未決事項を保持する。仕事の完了を管理する [Todo](project-todos.md) や実施要求である Issue とは用途が異なる。Project 画面の「設計メモ」で一覧と本文、履歴、会話からの追記を読む。初版は**そのメモの最初の記録**であり、Project 創始時の思想であるとは限らない。[画面](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx#L94-L121)・[初版と追記の説明](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx#L366-L413)

MCP の利用契約は、メモを実行命令として扱わず、出典の明確な最新の人間の判断を尊重し、AI 提案を本文より優先しないことを求める。AI の自発的記録は `append` を使い、本文の update / restore / integrate は明示的な本文編集意図を必要とする。これは tool の意味上の契約であり、DB が自然言語の承認を判定する機構ではない。書込権限を持つ caller は同じ action API を使う。[両MCPの契約](../../crates/server/src/remote_mcp/design_notes.rs#L24-L48)・[stdio](../../crates/mcp/src/task_server/tools/design_notes.rs#L72-L89)

## 本文と追記の二つの履歴

本文は title / body と revision、編集者、任意の変更理由を持つ。追記は本文と独立した revision を持ち、見出し・内容・区分 `decision` / `proposal` / `open_question`、出典、発言者、記録者、参照本文版、任意の短い引用を保存する。記録者は認証 channel / identity 由来で、caller が指定する発言者・出典とは別である。引用があれば指定本文版に含まれることを検証するが、出典や発言者の記述が真実であることまで証明しない。[REST actor](../../crates/server/src/routes/design_notes.rs#L72-L89)・[外部MCP actor](../../crates/server/src/remote_mcp/design_notes.rs#L60-L74)・[出典検証](../../crates/db/src/models/design_note.rs#L487-L514)

| 操作 | 履歴への効果と競合条件 |
| --- | --- |
| create | 本文 revision 1 と snapshot を作る |
| update | 現在の本文 `expected_revision` を照合し、新しい本文版を保存する |
| restore | 過去版の title / body を新しい本文版にする。版番号を巻き戻さない |
| append | 独立追記 revision 1 を追加する。本文版は増えない |
| correct_addition / set_addition_hidden | 追記の expected revision を照合し、新しい追記 snapshot を残す。元の記録者は保つ |
| integrate | 本文と追記の両 expected revision を照合し、可視追記を出典・発言者・区分・追記ID/版付きで本文末尾に加える。追記自体と過去版は残す |

[変更の分岐と保存](../../crates/db/src/models/design_note.rs#L270-L432)。非表示は削除や秘匿ではなく、UI で再表示でき、MCP の追記メタデータにも含まれる。統合済みフラグや別 request による重複統合の検出はこの処理にはないため、同じ追記の再統合を自動的に安全な no-op と考えない。隠れた追記は統合を拒否する。[統合](../../crates/db/src/models/design_note.rs#L339-L357)・[非表示と表示](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx#L478-L543)

## 競合と再送は保存と同じ transaction に置く

DB は transaction の最初に request 行への書込を行い、SQLite の競合 writer を直列化してから版を読む。receipt のキーは `(project_id, actor, request_id)`。同じ ID と同じ action 入力の再送は、最新状態ではなく**元の確定結果**を返す。入力を変えて同じ ID を再利用すると conflict になる。本文・追記・snapshot・結果保存が同一 transaction であり、途中の失敗を成功済み履歴として残さない。[受付と再送](../../crates/db/src/models/design_note.rs#L249-L269)・[確定](../../crates/db/src/models/design_note.rs#L416-L432)

REST は対象なしを404、revision / request ID 競合を409、不正入力を400へ対応させる。本文は最大100,000 Unicode文字、title / heading は300文字、source / speaker と quote / reason は2,000文字である。[検証](../../crates/db/src/models/design_note.rs#L143-L170)・[HTTPエラー](../../crates/server/src/error.rs#L204-L215)

UI は15秒ごとの再取得を行うが、編集中の版と下書きを勝手に更新しない。失敗時は下書きを残し現在版を取得し、比較後の「再適用」を明示操作にする。同じ操作の応答を失ったときは同じ request ID を保持する。これはマウント中の React state による保持であり、ページ再読込後の設計メモ下書き永続化を保証しない。[保存と再取得](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx#L32-L80)・[本文競合](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx#L266-L320)・[追記競合](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx#L552-L591)

## 生テキスト編集と保存前プレビュー

本文の新規作成・編集と追記の追加・訂正は、Markdown の生テキストを textarea で編集する。入力値をそのままフォーム state に渡し、保存時に既存の action として送るため、編集表示のために Markdown を読み戻して再直列化しない。プレビューと保存済み本文・追記の閲覧は、同じ disabled の `WYSIWYGEditor` を使う。共有チャットの編集契約とは区別する。[入力と描画の接続](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx#L24-L108)・[本文の保存](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx#L263-L295)・[追記の保存](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx#L691-L750)・[チャットの契約](session-and-agent-run.md#送信中の編集と-markdown-正規化)

「編集」「プレビュー」は表示だけを切り替える。textarea は非表示にして保持し、プレビューを選んだ間だけ読み取り専用 renderer をマウントする。切替では書込も revision 更新も行わず、プレビュー中でも保存ボタンによる明示保存が必要である。タブは左右矢印で往復し、Home で編集、End でプレビューを選択してフォーカスを移す。保存中は入力とタブを無効化する。[タブの lifecycle](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx#L44-L105)・[無書込と下書き保持の回帰](../../tests/design-notes/markdown-editing.mjs#L45-L73)

下書きの所有者は引き続き本文・追記のフォームであり、タブ側は内容を初期化しない。タイトル・変更理由・出典などもその state に残る。背景再取得や競合時の「再適用」は既存の基準版更新と組み合わせるため、revision を key にしてフォームを再マウントしたり、props の再取得で入力を上書きしたりすると下書き保持の契約を壊す。[本文の state と基準版](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx#L263-L265)・[明示再適用](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx#L338-L393)・[追記の基準版と state](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx#L629-L699)

本文・追記内容の textarea には HTML `maxlength` を設けない。サーバーは Rust の `chars().count()` で100,000 Unicode文字を検査し、UTF-16単位の HTML 制限によって絵文字を早く拒否することを避ける。本文は空白のみでも保存でき、追記内容は空白のみでは保存できない。生テキスト保持を任意の改行の byte 同一性保証と解釈しない。試験は既存の escape・空白・末尾改行の無編集保存を確認する一方、CRLF の保持または LF 正規化の両方を許す。[サーバー検証](../../crates/db/src/models/design_note.rs#L160-L171)・[追記の保存条件](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx#L781-L784)・[文字列往復と境界](../../tests/design-notes/markdown-editing.mjs#L201-L319)・[改行の許容範囲](../../tests/design-notes/markdown-editing.mjs#L321-L355)

## UI REST と MCP の境界

UI は `/api/projects/{project}/design-notes` の REST を使い、一覧・履歴を100件ずつ最後まで取得する。detail は本文と追記、書込成功は完全な結果を返す。両 MCP は同じDBを使うが、別の有界 read projection を共有する。stdio は global router に登録され、scoped orchestrator router には入らない。外部 HTTP は既存の [MCP 認証・operation receipt](../integrations/remote-access.md)を利用し、読取に `lvk:read`、書込には加えて `lvk:write` と live Project の確認が必要である。[UI取得](../../packages/web-core/src/features/design-notes/model/designNoteApi.ts#L20-L39)・[router](../../crates/mcp/src/task_server/tools/mod.rs#L49-L79)・[HTTP scope](../../crates/server/src/remote_mcp/mod.rs#L167-L193)

| MCP 読取 | 続き方と整合性 |
| --- | --- |
| list_design_notes | 本文なしのメタデータを最大20件。`next_cursor` を次の `cursor` にする |
| get_design_note の section=additions | 非表示を含む追記メタデータを最大20件。同じ UUID cursor 方式 |
| get_design_note_history | 本文、または addition_id の履歴メタデータを版番号順に最大20件。`next_after_revision` を渡す |
| get_design_note | 本文または addition_id の完全 snapshot を JSON 文字列 `content_page` として最大4,000 Unicode文字ずつ返す。返された revision と next_offset を固定して続け、結合後に JSON として読む |

一覧は live metadata であり、全件が同時点の snapshot ではない。本文ページは不変の保存版を読む。offset>0 に revision を省略すると拒否される。offset はUTF-8 bytesではなく Unicode文字で、JSON escape をまたぐページを個別に parse しない。検索なしではSQLの件数制限で metadata のみを読む。検索ありでは保存済み title/body を順に読み、合致したページ分だけを保持する。本文 snapshot は SQL substring で分割する。[共通projection](../../crates/db/src/models/design_note_read.rs#L33-L177)

MCP の書込成功結果は `{note:{id,revision},addition:null|{id,revision}}` に限定し、全文は上の読取で取得する。stdio は REST 結果を投影し、外部 HTTP は小さい成功結果を operation に保存する。古い全文入り operation も get_operation / 再送の返却時に投影し、保存済み証拠や元の確定版を書き換えない。commit前のDB/JSON失敗と確定したcommit拒否は `failed`、commit成立を否定できない失敗は `uncertain` として区別し、安全な診断を返す。保存済み receipt を解釈できない場合も `receipt_unreadable` の uncertain となるため、新しい request ID で盲目的に重複実行しない。[stdio成功](../../crates/mcp/src/task_server/tools/design_notes.rs#L72-L89)・[HTTP結果](../../crates/server/src/remote_mcp/design_notes.rs#L67-L94)・[旧receipt投影](../../crates/server/src/remote_mcp/issue_write.rs#L97-L139)

## Project を削除した後の保持

設計メモがあることだけを理由に Project 削除を409にする暫定仕様は撤回された。追加 migration は本文・本文履歴・追記・追記履歴・request receipt の全5表を内容のまま移し、FK を live Project から保持専用 `design_note_projects` へ変更する。削除直前 trigger が元ID、削除時名称、Organization、削除日時を保持する。Organization がなければ NULL のままで、別Projectへ移さない。[保持migration](../../crates/db/migrations/20261023110000_retain_design_notes_on_project_delete.sql)

保持は既存APIでの閲覧継続を意味しない。読取・更新・旧 request の再送は live Project を必要とし、削除後は閉鎖する。同じ Project UUID の再作成も trigger が拒否する。一方、Integration / 外部 MCP operation など他機能のFK削除制約は残るため、Project 削除はそれらにより失敗し得る。その場合、保持 identity の更新も削除とともに rollback される。[削除入口](../../crates/server/src/routes/local_remote.rs#L1589-L1608)・[live照合](../../crates/db/src/models/design_note.rs#L173-L196)・[再送の照合](../../crates/db/src/models/design_note.rs#L249-L268)・[原子性回帰](../../crates/server/src/routes/local_remote.rs#L3502-L3626)

削除後の閲覧・持出し・再関連付け・保持期限は LOCAL-99 に委ねると記録されており、新しい閲覧APIや復元UIを追加した状態ではない。暫定409やCASCADE削除案を現行方針として復活させない。[migrationに残るアクセス方針](../../crates/db/migrations/20261023110000_retain_design_notes_on_project_delete.sql#L1-L3)

## 変更時の検証入口

[DBテスト](../../crates/db/src/models/design_note_tests.rs)は初版保存、復元、独立追記、記録者と編集者、複数SQLite接続の競合、Unicode・多数件・固定版、再送receiptを検証する。[server回帰](../../crates/server/src/remote_mcp/tests.rs#L1541-L1835)は scope、Project境界、書込応答上限、legacy receipt、削除後拒否を扱う。旧DBからのmigrationと全column不変、他FK失敗時の原子性は [Project削除fixture](../../crates/server/src/routes/local_remote.rs#L3429-L3626)で確認する。[隔離ブラウザ試験](../../tests/design-notes/isolated.mjs)は本文・追記競合、下書き再適用、Markdown安全性、実stdio、狭い画面、削除成功とSQL保持を横断する。

今回の Sync はこれらのソースとテスト内容を静的に照合した。移行前の開発記録では旧409試験から保持・削除後拒否の試験へ変更したとされる。現行受入条件は上記migrationと保持fixtureで確認し、旧409を復活させない。今回アプリ試験を再実行した結果や、実Auth0/ChatGPT・稼働DB移行の検証とは扱わない。


### 生テキスト編集の過去 Sync 記録

過去Syncは source `39b2fe9976efeed810bad80dc38d01e4123bc4c3` と当時の凍結 manifest・全4チャンク（SHA-256照合済み）を対象に、UI の入力・描画・フォーム state、既存の REST 直列化、DB の Unicode 検証、独立したブラウザ試験を照合した。新しい [Markdown 編集 fixture](../../tests/design-notes/markdown-editing.mjs) は既存の [隔離試験](../../tests/design-notes/isolated.mjs)から呼ばれ、4フォームの入力・貼付け、プレビュー無書込、既存文字列の無編集保存、応答消失後の同一 request 再送、検証拒否後の保持、100,000文字境界、狭い画面を扱う。既存の競合・復元・保持契約は継続し、今回の製品差分は REST/MCP/DB や共有 editor を変更していない。

当時のWikiが参照した Change Manifest は開発時の隔離E2E成功、後続の軽量同期でのスタブ smoke と実E2E未再実行、最後の正式 Integration の install / frontend build / check / lint / backend:test 成功をそれぞれ記録している。Workspace Memory の統合前の状態を現在の統合結果へ読み替えない。この保守で行ったのはソース・試験定義・文書差分・リンクの静的照合と OpenWiki lifecycle の検証であり、製品試験、実 Auth0/ChatGPT、実機モバイルを再実行した結果ではない。無関係な過去の Wiki 全体を再検証したとは扱わない。

## 一覧検索と下書きの関係

ブラウザー一覧は取得済みの title/body を trim・小文字化した部分一致で絞る。検索で選択中メモが一覧から消えても、detail と未保存入力を解除しない。追記本文を検索対象へ含める処理ではない。両 MCP の `list_design_notes.search` は同じ保存済み title/body を検索するが、空白は literal に保持し、Unicode 小文字化後の部分一致で `%` や `_` を wildcard にしない。検索条件を固定して UUID cursor を進める。[画面](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx#L119-L200)・[ブラウザー述語](../../packages/web-core/src/shared/lib/listSearch.ts#L1-L8)・[MCP検索](../../crates/db/src/models/design_note_read.rs#L80-L108)・[共有述語](../../crates/db/src/models/text_search.rs#L1-L10)。

変更時は DB の search fixture と `tests/search/isolated.mjs` の選択・下書き保持を確認する。今回の統合では旧 contenteditable を textarea へ追従させた fixture も含まれ、製品の保存・版管理契約を緩和した変更ではない。


## 一覧・詳細の再取得周期

一覧と選択中の詳細は15秒周期で再取得する。書込み直後の再取得と既存の下書き・版・競合保護は維持する。[UI](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx#L131-L181)
