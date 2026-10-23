# 設計メモの検証と後続TODO

人間の本文と追記は別に保存する。本文の更新・復元・明示統合は新しい本文版、
追記の訂正・非表示・再表示は新しい追記版を作る。初版と訂正前の出典は残す。
revision 1はそのメモの最初の保存であり、Project創始時の記録とは限らない。
最新の明確な人間の判断を尊重するが、AI提案が新しいだけで本文を覆さない。

## API / MCP

REST: `/api/projects/{project}/design-notes` の GET（100件、offset）と POST。
POSTは `{request_id, operation}`、operationのactionは create/update/restore/
append/correct_addition/set_addition_hidden/integrate。各変更に必要なrevisionを渡す。
`/{note}/history` と `/{note}/additions/{addition}/history` は初版から100件ずつ返す。
同じrequest_id・同じ認証主体・同じProject・同じ入力の再送は保存済み結果を返す。
保存済みの同じIDに異なる入力を指定した場合と、古いrevisionは409。競合時は下書きを保持し、現在版を確認する。

UI用RESTの返却形式は維持する。MCPは同じDBへの小さいread projectionを使う。

内部stdio / 外部HTTP MCPは list_design_notes / get_design_note /
get_design_note_history / write_design_note を提供する。外部書込みは既存のoperation
receiptを返すため get_operation で結果を確認する。外部は lvk:read と lvk:write、
設定されたProject範囲に従う。内部stdioは既存の内部認証を使う。
MCP一覧は本文なしの最大20件、next_cursorをcursorとして継続する。
get_design_noteのsection=additionsで追記メタデータを同じ方式で取得する。
履歴は最大20版のメタデータ、next_after_revisionをafter_revisionとして継続する。
get_design_noteは本文、addition_id指定時は追記の保存snapshotを最大4000 Unicode文字の
content_pageとして返す。next_offsetと返却revisionで続きを取得し、全ページを結合して
JSONとして読む。offset>0にはrevision必須。revision指定は不変の過去版を読み、途中で
編集されても混ざらない。一覧集合はliveであり同時追加時は再走査する。

自発的AI記録はappendを使い、本文更新・復元・統合には明示された本文編集意図が必要。
これは新しい承認キューではなく、MCPの利用契約である。

## 試験手順

リポジトリ規約に従いsccacheを使い、Cargoの並列数を制限する。
以下の重いCargo操作は順次実行する。

```sh
cargo test -p db design_note --lib
cargo test -p server design_note_mcp --lib
cargo test -p server http_discovery_authentication_and_tool_call_roundtrip --lib
pnpm run generate-types
pnpm run generate-types:check
pnpm run prepare-db
pnpm run prepare-db:check
pnpm run web-core:check
pnpm run local-web:check
pnpm run local-web:lint
pnpm --dir packages/local-web run build
cargo build --bin server --bin agent-process-host --bin vibe-kanban-mcp
node tests/design-notes/isolated.mjs
```

E2Eはこのcheckoutのtarget/debugのbinaryとbuilt frontendを使用し、新規
LVK_INSTANCE_DIR・SQLite・loopbackのOS割当ポートで起動する。現行サーバーや共有DBは
使わず、有料モデルも呼ばない。finallyで開始した子プロセスを停止・待機し、一時DBを削除。
スクリーンショット3枚とログは `.build/design-notes/retention-e2e/` に残す。
`LVK_CHROMIUM_EXECUTABLE`で既存Chromiumの絶対パスを指定できる。

ブラウザ検査は作成・編集・複数メモ、初版・全版・復元、MCP追記と再送、
UI/UI・UI/MCP競合、訂正前出典・非表示・再表示・本文統合、Project境界、
Markdownの危険リンク、1440/390/320px、再読込を実APIとDOMで照合する。
外部MCPの権限とreceiptはRust統合試験で別に検証する。
実Auth0/ChatGPT接続、Firefox/WebKit、実機モバイルはこの試験の対象外。

## 別管理の後続TODO（未着手）

関連TODO: `7e3f9f96-f5c3-414d-abc0-c36598391738`

- [ ] 設計メモMCPが実装・公開され、実際に利用可能であることを確認した後、
  `plugins/lvk/skills/lvk-work/SKILL.md` に短い利用原則を反映する。
  自発的追記、本文非上書き、出典・発言者と記録者の区別、確定/提案/未決の区別、
  重複回避、初版/履歴参照を扱う。現時点ではSkillを変更しない。

永久削除・保持期限は未合意。履歴を削除する仕様は追加していない。

## 追加レビュー修正の検証待ち

追跡可能な報告: [設計メモの検証・追加レビュー修正](../../docs/development/design-notes-validation.md)。
追加修正時はLOCAL-88との競合を避け軽量チェックのみで停止した。その後ユーザーの
heavy並行実行承認を受け、Rust/HTTP/MCP、型生成、SQLx、Lint/Clippy/build、隔離E2Eを
実行して成功。最新の結果・対象commit・未検証範囲は上記の追跡報告を参照。
軽量SQL smoke: `python3 tests/design-notes/bounded-read-sql.py`。

MCP書込みの成功結果は `{note:{id,revision},addition:null|{id,revision}}` のみ。
stdioは直接これを返し、外部HTTPはoperation receiptのsucceeded resultに保存する。
再送は元の確定版を返す。全文は返却ID/revisionを使ってbounded readで取得する。
UI RESTのDesignNoteResult契約は変更しない。

## Project削除時の暫定保持（2026-10-23の明示承認）

設計メモを理由にProject削除を拒否しない。本文・追記・全版・出典・request receiptを
元Project IDとともに残し、削除時の名称とOrganizationを保持する。
削除済みProjectのメモは既存REST/MCPから参照・更新できず、同じProject IDの再作成も拒否する。
他機能のIntegration/MCP operation等による既存の削除制約は維持する。
保持後の閲覧・持ち出し・再関連付け・保持期限はLOCAL-99
(`7360a413-e8f5-447b-a252-ba74de005976`)で決定する。復元UIは今回追加しない。

E2Eは設定画面からの削除成功・一覧からの消失に加え、隔離SQLiteの全保存columnを
削除前後でSHA-256比較し、元識別情報、FK整合性、再削除、削除後と別Projectからの拒否を確認する。
`sql-snapshot.py`はtask-ownedの隔離E2E DBのみをread-onlyで開き、本文や出典の内容は出力しない。
