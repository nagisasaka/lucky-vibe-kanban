---
type: concept
title: Notes
description: Project の設計意図、本文と独立追記の寿命。
tags: [design-notes, project, history, provenance]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-830e28055c4ae6f5464df312
    resource: repo://crates/db/src/models/design_note.rs
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# Notes の本文と独立追記

Notes（設計メモ）は [Project](project-and-issue.md) の目的、設計意図、判断や未決事項を記録する。実施要求の Issue や共有メモの [TODO list](project-todos.md) と用途が異なる。初版はそのメモの最初の記録であり、Project 創始時の意図とは限らない。記録を編集・参照するときは[操作の段階](../quickstart.md#操作の段階)も区別する。[画面](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx)

## 本文と追記の寿命

本文は title、body、revision、編集者と任意の変更理由を持つ。追記は本文と独立した revision を持ち、`decision`、`proposal`、`open_question`、出典、発言者、記録者、参照本文版と任意の引用を保存する。認証から決まる記録者と、caller が指定する発言者・出典は別である。引用は対象本文版に含まれるか検査するが、出典の記述そのものの真実性を証明しない。[モデル](../../crates/db/src/models/design_note.rs)・[actor](../../crates/server/src/routes/design_notes.rs)

| 操作 | 効果 |
| --- | --- |
| create / update | 初版または期待 revision を照合した新しい本文版を保存する |
| restore | 過去版を新しい本文版として保存し、版番号を巻き戻さない |
| append | 独立追記の初版を追加し、本文版を増やさない |
| correct / hide | 追記の期待 revision を照合し、新しい追記履歴を保存する |
| integrate | 本文と追記の両 revision を照合し、可視追記を出典付きで新しい本文版へ加える |

非表示は削除や秘匿ではない。隠れた追記は統合できず、統合済み追記自体や過去版は残る。別 request による同一追記の再統合を自動 no-op と仮定しない。[変更処理](../../crates/db/src/models/design_note.rs)

## 競合、再送、編集

書込みと receipt は同じ transaction に入り、キーは Project・actor・request ID である。同じ ID と同じ入力の再送は元の確定結果を返す。同じ ID に異なる入力を使うと競合する。revision 競合時は draft を保持して現在版を読み、比較して明示的に再適用する。[書込み](../../crates/db/src/models/design_note.rs)・[UI](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx)

本文と追記は Markdown 生テキストとして編集し、プレビュー切替だけでは保存しない。本文は空でもよいが、追記内容は空白のみを拒否する。本文・追記は最大100,000 Unicode 文字、title・heading は300文字である。編集途中の React state はページ再読込を越える永続 draft の保証ではない。[検証](../../crates/db/src/models/design_note.rs)・[編集](../../packages/web-core/src/features/design-notes/ui/DesignNotePanel.tsx)

MCP の意味上の契約では、メモを命令ではなく出典付きの文脈として扱う。AI の提案と人間の判断を区別し、本文の update・restore・integrate は明示的な本文編集の意図を要する。自然言語の承認を DB が判定する仕組みではない。[MCP](../../crates/server/src/remote_mcp/design_notes.rs)

## 読取と削除後

UI REST と両 MCP は同じ DB を使うが、MCP は有界な読取 projection を使う。一覧・追記 metadata と履歴は最大20件ずつ、本文 snapshot は最大4,000 Unicode 文字ずつ返す。後続の本文ページは返された revision と offset を固定し、分割 JSON 文字列を結合してから解析する。書込み成功の MCP receipt は ID と revision であり、全文取得とは別である。[読取](../../crates/db/src/models/design_note_read.rs)・[receipt](../../crates/db/src/models/design_note.rs)

Project 削除時は本文、履歴、追記、追記履歴、request receipt と削除時の Project identity を保持する。一方、既存 API は live Project を要求し、削除後の閲覧や書込みを提供しない。同じ Project UUID の再利用も拒否する。他機能の外部キーで削除が失敗すれば保持 identity の更新も rollback する。削除後の閲覧・持出し・再関連付け・保持期限は未実装の課題である。[保持 migration](../../crates/db/migrations/20261023110000_retain_design_notes_on_project_delete.sql)

DB とブラウザーの fixture は版競合、再送、保持、Markdown 編集を検査するが、実 Auth0 や稼働 DB の移行を証明しない。[DB tests](../../crates/db/src/models/design_note_tests.rs)・[browser tests](../../tests/design-notes/README.md)
