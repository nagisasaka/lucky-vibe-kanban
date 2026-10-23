---
type: concept
title: 設計メモ・本文履歴と独立追記
description: Project の設計意図を本文と独立追記として残す契約。版管理、出典、競合、UIとMCPの応答境界、Project削除後の履歴保持を説明する。
tags: [design-notes, project, history, provenance, mcp]
verified:
  - by: openwiki/0.5.1
    at: 2026-10-23T14:57:15.192Z
sources:
  - id: openwiki-source-c2fc034438311f3aa02f5ee5
    resource: repo://crates/db/migrations/20261023110000_retain_design_notes_on_project_delete.sql
  - id: openwiki-source-27eb1d689068b93b5282d2b6
    resource: repo://crates/db/src/models/design_note_read.rs
  - id: openwiki-source-830e28055c4ae6f5464df312
    resource: repo://crates/db/src/models/design_note.rs
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
  - id: openwiki-source-5d1b4189d87b8ac4c58a8f4c
    resource: repo://docs/development/design-notes-validation.md
  - id: openwiki-source-6380bb3355ad242288be2e3b
    resource: repo://packages/web-core/src/features/design-notes/model/designNoteApi.ts
  - id: openwiki-source-9fba069b7c8bc13326ad6d2a
    resource: repo://packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx
generated: { by: "codex", at: "2026-10-23T14:57:15.192Z" }
---

# 設計メモ・本文履歴と独立追記

設計メモは [Project](project-and-issue.md) に複数置ける Markdown の記録で、「何を作りたいか、なぜそう考えたか」と、その後の判断・提案・未決事項を保持する。仕事の完了を管理する [Todo](project-todos.md) や実施要求である Issue とは用途が異なる。Project 画面の「設計メモ」で一覧と本文、履歴、会話からの追記を読む。初版は**そのメモの最初の記録**であり、Project 創始時の思想であるとは限らない。[画面](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx#L94-L121)・[初版と追記の説明](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx#L366-L413)

MCP の利用契約は、メモを実行命令として扱わず、出典の明確な最新の人間の判断を尊重し、AI 提案を本文より優先しないことを求める。AI の自発的記録は `append` を使い、本文の update / restore / integrate は明示的な本文編集意図を必要とする。これは tool の意味上の契約であり、DB が自然言語の承認を判定する機構ではない。書込権限を持つ caller は同じ action API を使う。[両MCPの契約](../../crates/server/src/remote_mcp/design_notes.rs#L24-L48)・[stdio](../../crates/mcp/src/task_server/tools/design_notes.rs#L78-L94)

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

UI は5秒ごとの再取得を行うが、編集中の版と下書きを勝手に更新しない。失敗時は下書きを残し現在版を取得し、比較後の「再適用」を明示操作にする。同じ操作の応答を失ったときは同じ request ID を保持する。これはマウント中の React state による保持であり、ページ再読込後の設計メモ下書き永続化を保証しない。[保存と再取得](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx#L32-L80)・[本文競合](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx#L266-L320)・[追記競合](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx#L552-L591)

## UI REST と MCP の境界

UI は `/api/projects/{project}/design-notes` の REST を使い、一覧・履歴を100件ずつ最後まで取得する。detail は本文と追記、書込成功は完全な結果を返す。両 MCP は同じDBを使うが、別の有界 read projection を共有する。stdio は global router に登録され、scoped orchestrator router には入らない。外部 HTTP は既存の [MCP 認証・operation receipt](../integrations/remote-access.md)を利用し、読取に `lvk:read`、書込には加えて `lvk:write` と live Project の確認が必要である。[UI取得](../../packages/web-core/src/features/design-notes/model/designNoteApi.ts#L20-L39)・[router](../../crates/mcp/src/task_server/tools/mod.rs#L49-L79)・[HTTP scope](../../crates/server/src/remote_mcp/mod.rs#L167-L193)

| MCP 読取 | 続き方と整合性 |
| --- | --- |
| list_design_notes | 本文なしのメタデータを最大20件。`next_cursor` を次の `cursor` にする |
| get_design_note の section=additions | 非表示を含む追記メタデータを最大20件。同じ UUID cursor 方式 |
| get_design_note_history | 本文、または addition_id の履歴メタデータを版番号順に最大20件。`next_after_revision` を渡す |
| get_design_note | 本文または addition_id の完全 snapshot を JSON 文字列 `content_page` として最大4,000 Unicode文字ずつ返す。返された revision と next_offset を固定して続け、結合後に JSON として読む |

一覧は live metadata であり、全件が同時点の snapshot ではない。本文ページは不変の保存版を読む。offset>0 に revision を省略すると拒否される。offset はUTF-8 bytesではなく Unicode文字で、JSON escape をまたぐページを個別に parse しない。SQL の件数制限・substring により、MCP のために本文一覧全体を読み込まない。[共通projection](../../crates/db/src/models/design_note_read.rs#L33-L177)

MCP の書込成功結果は `{note:{id,revision},addition:null|{id,revision}}` に限定し、全文は上の読取で取得する。stdio は REST 結果を投影し、外部 HTTP は小さい成功結果を operation に保存する。古い全文入り operation も get_operation / 再送の返却時に投影し、保存済み証拠や元の確定版を書き換えない。DB/JSON失敗は HTTP operation で `uncertain`、domain 拒否は `failed`。保存済み receipt を解釈できない場合も `receipt_unreadable` の uncertain となるため、新しい request ID で盲目的に重複実行しない。[stdio成功](../../crates/mcp/src/task_server/tools/design_notes.rs#L81-L94)・[HTTP結果](../../crates/server/src/remote_mcp/design_notes.rs#L67-L94)・[旧receipt投影](../../crates/server/src/remote_mcp/issue_write.rs#L97-L139)

## Project を削除した後の保持

設計メモがあることだけを理由に Project 削除を409にする暫定仕様は撤回された。追加 migration は本文・本文履歴・追記・追記履歴・request receipt の全5表を内容のまま移し、FK を live Project から保持専用 `design_note_projects` へ変更する。削除直前 trigger が元ID、削除時名称、Organization、削除日時を保持する。Organization がなければ NULL のままで、別Projectへ移さない。[保持migration](../../crates/db/migrations/20261023110000_retain_design_notes_on_project_delete.sql)

保持は既存APIでの閲覧継続を意味しない。読取・更新・旧 request の再送は live Project を必要とし、削除後は閉鎖する。同じ Project UUID の再作成も trigger が拒否する。一方、Integration / 外部 MCP operation など他機能のFK削除制約は残るため、Project 削除はそれらにより失敗し得る。その場合、保持 identity の更新も削除とともに rollback される。[削除入口](../../crates/server/src/routes/local_remote.rs#L1589-L1608)・[live照合](../../crates/db/src/models/design_note.rs#L173-L196)・[再送の照合](../../crates/db/src/models/design_note.rs#L249-L268)・[原子性回帰](../../crates/server/src/routes/local_remote.rs#L3502-L3626)

削除後の閲覧・持出し・再関連付け・保持期限は LOCAL-99 に委ねると記録されており、新しい閲覧APIや復元UIを追加した状態ではない。暫定409やCASCADE削除案を現行方針として復活させない。[採用方針と未決事項](../../docs/development/design-notes-validation.md#保存アクセスの境界)

## 変更時の検証入口

[DBテスト](../../crates/db/src/models/design_note_tests.rs)は初版保存、復元、独立追記、記録者と編集者、複数SQLite接続の競合、Unicode・多数件・固定版、再送receiptを検証する。[server回帰](../../crates/server/src/remote_mcp/tests.rs#L1541-L1835)は scope、Project境界、書込応答上限、legacy receipt、削除後拒否を扱う。旧DBからのmigrationと全column不変、他FK失敗時の原子性は [Project削除fixture](../../crates/server/src/routes/local_remote.rs#L3429-L3626)で確認する。[隔離ブラウザ試験](../../tests/design-notes/isolated.mjs)は本文・追記競合、下書き再適用、Markdown安全性、実stdio、狭い画面、削除成功とSQL保持を横断する。

今回の Sync はこれらのソースとテスト内容を静的に照合した。開発時の試験結果は[履歴付き検証記録](../../docs/development/design-notes-validation.md)にあり、旧409受入試験は現在の受入条件ではない。今回アプリ試験を再実行した結果や、実Auth0/ChatGPT・稼働DB移行の検証とは扱わない。
