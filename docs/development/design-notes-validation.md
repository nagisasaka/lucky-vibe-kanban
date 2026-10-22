# 設計メモの検証・追加レビュー修正

## 最終検証（2026-10-23 JST）

ユーザーのheavy並行実行承認後、以下の検証を実行して成功した。以前の「GO待ち／未実行」は下記の経緯記録であり、この最終結果で更新する。

製品ソースは `c4b992d2b7c170f47fae5449b9f802d81fbb2f19` から変更不要。E2E手順の一覧返却形式と削除確認ダイアログの指定を修正し、削除拒否UI・履歴不変assertionを追加した。クリーンな受入確認commitは `7e1a3b4cf62944de6005c159f428523364eabdf6`。`git HEAD == serverBuild.revision`、両dirty=falseをassertしたChromium E2Eが成功。文書更新後の最終HEADについても同じ一致条件で再実行することを完了条件とする（最終実行ログ `.build/design-notes/final/e2e-final.log`）。

### 実行コマンドと結果

Cargoは `CARGO_BUILD_JOBS=2` と既存sccacheを使用。重いコマンドはこのWorkspace内で逐次実行。Playwrightは単一Nodeプロセス・単一browser、並列workerなし（実質workers=1）。

| コマンド | 結果 |
| --- | --- |
| `cargo test -p db design_note --lib` | 8件成功。長文/Unicode、44メモ・43追記・44版のページ継続、上限、不正ページ、固定revision、本文と追記の履歴、複数SQLite接続競合、再送を含む。 |
| `cargo test -p server design_note_mcp --lib` | 2件成功。scope/Project境界、初回/再送/get_operation、成功receipt保存結果256 bytes未満・operation応答4096 bytes未満、legacy全文receipt投影、正本の最大10万文字本文＋追記の保持。 |
| `cargo test -p server project_delete_temporarily_protects_design_history_without_data_loss --lib` | 1件成功。409と暫定理由、Project/関連子/本文/履歴/追記/receiptの全column不変、メモなしProject削除。 |
| `cargo test -p server http_discovery_authentication_and_tool_call_roundtrip --lib` | 1件成功。外部HTTPの認証・scope challenge回帰。 |
| `pnpm run generate-types:check` | 成功。生成型・schemaに差分なし。 |
| `pnpm run prepare-db:check` | 成功。隔離一時DBへ全migration適用、metadata整合。既存のpotentially unused queries警告あり。 |
| `cargo clippy -p db -p server -p mcp --all-targets -- -D warnings` | 成功。 |
| `pnpm run web-core:check` / `pnpm run local-web:check` | 各成功。 |
| `pnpm run local-web:lint` | 成功。 |
| `pnpm --dir packages/local-web exec eslint ../web-core/src/features/design-notes ../web-core/src/features/kanban/ui/KanbanContainer.tsx ../web-core/src/features/todos/model/todoViewStore.ts ../web-core/src/shared/dialogs/settings/settings/RemoteProjectsSettingsSection.tsx --ext ts,tsx --config .eslintrc.cjs --max-warnings 0` | 成功。 |
| `pnpm --dir packages/local-web run build` | 成功。既存Browserslist/Tailwind/chunkサイズ警告あり。 |
| `cargo build --bin server --bin agent-process-host --bin vibe-kanban-mcp` | 成功。既存targetを再利用。commit凍結後はserverのbuild identity更新のみを再ビルド。 |
| `node tests/design-notes/isolated.mjs` | 成功。Chromium 147.0.7727.15、隔離DBとOS割当port 41547。 |
| `node --check tests/design-notes/isolated.mjs` / `git diff --check` | 成功。 |

各ログは `.build/design-notes/final/` に保持。SQLx CLIは既存のこのWorkspaceのtarget内0.8.6 binaryを再利用し、再インストール・重複binary作成を避けた。generated filesやquery metadataは変更していない。

### 実ブラウザの操作・観察

- 1440px一覧＋本文、390/320px一覧→詳細→一覧、light/dark、再読込を確認。初版/各版/復元、追記訂正/訂正前出典/非表示/再表示/明示統合を検証。
- UI/UI・UI/MCP競合の拒否、下書き保持・現在版比較・明示再適用を検証。編集中に追記を非表示にされても下書きを保持。
- CSRF拒否、REST/MCPの別Project参照拒否、同一request_id再送、異なる入力の再利用拒否、安全なMarkdownを確認。
- 実stdio MCPで10万Unicode文字の本文と追記を保存。後続の本文更新後も再送は元の確定id/revisionを返し、receiptは256 bytes未満。bounded readで元の本文・追記全文を復元。
- 設定画面→Projectの削除メニュー→確認画面→削除を操作。実DELETEの409と履歴保護理由を画面に表示し、削除成功表示が出ないこと、Projectが残ること、本文detail・全本文版・全追記版が操作前後で一致することを確認。
- 画像はdesktop-dark/mobile-light/delete-refusedの3枚に限定し、目視でも確認。DOM/API assertionが主な検証で、画像だけで合否を判断していない。

初回の追加UI試験は、既存KeyboardDialogに`role=dialog`がないためlocatorで失敗した。実画面の見出しから確認内容を特定するよう試験のみ修正。製品側の拒否処理・assertionを緩めず再実行して成功した。

### リソース・片付け・未検証範囲

工程前に `/proc/meminfo`、CPU差分、load、memory PSI、swap増減、disk空きを記録。19:55 UTCのswap約1.44GiB増加とmemory full avg10=10.2で次工程を自動停止。待機後、available 13.6GiB・swap減少・圧迫低下を確認して再開した。停止は検証失敗ではなく工程開始ガード。観測ログは `.build/design-notes/final/resources.jsonl`。

自分の試験server/agent-process-host/MCP残存プロセス0、一時instance DBディレクトリ0、prepare_db.sqliteなしを確認。既存target（約35GB）・依存関係・built frontend（約77MB）は継続作業用に保持。証拠ログ/画像はtask-owned `.build/design-notes/` に保持。共有cache、稼働サーバー/DB、他Workspaceは変更していない。

実Auth0/ChatGPTアカウント、Firefox/WebKit、実機モバイル、Tauri、全workspace一括試験、正式Integration、稼働DBへのmigrationは未検証。外部MCPはRust/HTTP fixture、実ブラウザは隔離local認証＋stdio MCPで検証。有料モデル呼出しは追加していない。ユーザーからc4b992d2の独立再レビューでP1/P2解消・追加ブロッカーなしの報告を受領。今回の変更は試験と報告のみ。

Project永久削除方針は人間判断待ち。暫定409を維持し、main merge/deploy/Wiki/Skill更新は実施していない。

---

## 以下は実装・追加レビュー時点の経緯記録


今回の追加修正はコード修正と軽量チェックまで。LOCAL-88が重い検証を使うため、ユーザー指示によりRustコンパイル・型生成・ブラウザE2Eの前で停止する。**修正後のRust/API/UI実行結果は未検証**であり、前回成功を今回の成功とは扱わない。この報告書を含む修正commitは `git log -1 --format=%H -- docs/development/design-notes-validation.md` で特定できる。

## 今回の修正

### MCPの読み出し量

[共通projection](../../crates/db/src/models/design_note_read.rs)を内部stdio・外部HTTP MCPから使用する。保存先はUIと同じテーブル・履歴。ユーザーが認めた「MCP read projectionのみを小さくする」方針を採用し、UI用RESTの既存返却形式・書込み結果形式は変更しない（MCP書込み結果は後述の追加修正で小さくした）。MCPは従来の大量本文list/history・無制限additions detailを呼ばなくなる。

- `list_design_notes`: 本文なしのメタデータ、既定・最大20件。`next_cursor`を`cursor`として続ける。
- `get_design_note`, `section=additions`: 非表示を含む追記メタデータ、最大20件。同じcursor方式。返却されたid・revisionを指定して追記内容を読む。
- `get_design_note_history`: 本文または`addition_id`の履歴メタデータ、最大20版。`next_after_revision`を`after_revision`として続ける。
- `get_design_note`: 本文、または`addition_id`で指定した追記の完全な保存snapshotをJSON文字列の`content_page`として取得。既定・最大4000 Unicode文字。`next_offset`と返却`revision`で続け、ページ文字列を結合してからJSONとして解釈する。小分けでも出典・編集者・変更理由を捨てない。

既存の [read.rs](../../crates/server/src/remote_mcp/read.rs) のメタデータkeyset・SQL `substr`/`length`による文字ページと、[evidence_read.rs](../../crates/server/src/remote_mcp/evidence_read.rs) のJSON文字列ページの契約に合わせた。既存helperはremote専用なので、stdioからも同じSQLを使える薄いDB projectionに置いた。別正本・新しいドキュメント管理機構は作らない。

文字offsetはUTF-8 byte offsetではなくUnicode scalar単位。JSONのescape列や結合文字をページ境界で分ける場合も、文字列を結合すれば元snapshotを復元できる。4000文字のchunk自体はJSONエスケープを含め最大約24KB。メタデータ件数・title/heading長も保存時の制限で有界。SQL取得は最大21行（継続判定用1行を含む）、本文は1snapshotのsubstringのみで、本文100KB×100件をMCPのためにロードしない。

offset>0には保存済みrevisionが必須。初回のrevision・文字列・長さを1 SQLで取得し、後続は不変snapshotを指定する。読取中の訂正・非表示・本文更新で別版が混ざらず、過去版として読み切れる。一覧はlive metadataであり全件同時点snapshotではない。UUID順なので編集による並び替えは起きないが、同時追加の全件把握には一覧を再走査する。この制約をレスポンスに明記する。履歴はrevision順。Project/Note所属を確認し、他Projectのcursorも拒否する。

### Project削除の暫定保護

[delete_project](../../crates/server/src/routes/local_remote.rs)で、設計メモまたはretry receiptがあるProjectの削除を条件付きDELETEで抑止し、明示的なHTTP 409と履歴保護理由を返す。最初のSQLが書込みなので、事前確認とDELETEの間にメモが追加される競合窓を作らない。拒否時は既存の関連データへのCASCADEも始まらない。メモなしProjectの従来の削除動作は維持する。migrationのNO ACTIONは変更しない。

[設定画面](../../packages/web-core/src/shared/dialogs/settings/settings/RemoteProjectsSettingsSection.tsx)は削除transactionのpersistedを待ってから成功を表示し、失敗時はAPI理由を表示する。従来は非同期削除を待たず成功表示していた。既存の[エラー解析](../../packages/web-core/src/shared/lib/electric/collections.ts)がレスポンスmessageとstatusを保持する。

## Project全体削除の製品判断（未合意）

既存の[削除確認UI](../../packages/web-core/src/shared/dialogs/org/DeleteRemoteProjectDialog.tsx)は「Projectと全Issueを永久削除、取り消せない」と案内する。[local kanban migration](../../crates/db/migrations/20260427000000_local_kanban.sql)もProject配下のIssue・status等をCASCADEする。一方、設計メモの要求は「通常の編集・復元で初版と履歴を失わせない」であり、Project全体の永久削除・保持期限は未合意。この2つから『メモがあるProjectは恒久削除不可』とも『既存の削除確認がメモ全履歴の破棄まで承認した』とも断定できない。

| 選択肢 | 利点と影響 |
| --- | --- |
| Projectの明示的な永久削除に、設計メモ・追記・全履歴・retry receiptも含める | 既存のProject所有関係・削除操作と整合し、孤立データや新管理画面を作らない。全履歴が失われるため、確認文に対象を明記する人間の製品判断が必要。 |
| Projectを消しても設計メモ履歴を保持する | 初期意図をProject終了後も参照できる。ただし所有先・アクセス権・検索/閲覧・保持期限の設計が新たに必要で、今回のKISSの範囲を広げる。 |
| 暫定409を維持し、判断まで削除を止める | 今回の採用。データ保護を優先できるが、削除したいユーザーの操作を制限する。永久仕様として固定しない。 |

**推奨（未承認・未実装）**: 通常操作では全履歴を保持しつつ、人間がProject全体の永久削除を明示した場合は設計メモも削除対象とする案を第一候補にする。既存の削除モデルと整合し、新管理機構を要しないため。ただし履歴破棄を含む確認文と製品判断をユーザーが承認するまでは今回の暫定409を維持する。CASCADE追加、アーカイブ、別保管庫、保持期限は今回導入していない。

## 今回のチェックと回帰試験

実行済み:

| コマンド | 結果 |
| --- | --- |
| `python3 tests/design-notes/bounded-read-sql.py` | 成功。本番SQL文字列＋実migration＋合成in-memory SQLiteで、44メモ・43追記・44追記版、10万Unicode文字、更新後も固定版を復元、拒否DELETE前後のDB全dump一致、メモなしProject削除を確認。Rust/APIの代替検証ではない。 |
| `rustfmt --edition 2024 --config skip_children=true --check <変更Rustファイル>` | 成功。対象はこのcommitで変更したRustファイル。コンパイルはしない。 |
| `pnpm --dir packages/local-web exec prettier --check ../web-core/src/shared/dialogs/settings/settings/RemoteProjectsSettingsSection.tsx` | 成功。 |
| `pnpm --dir packages/local-web exec eslint ../web-core/src/shared/dialogs/settings/settings/RemoteProjectsSettingsSection.tsx --ext ts,tsx --config .eslintrc.cjs --max-warnings 0` | 成功。 |
| `node --check tests/design-notes/isolated.mjs` | 成功。実行はしていない。 |
| `git diff --check` | 成功。 |

追加したRust回帰試験（**未実行**）:

- `bounded_reads_preserve_unicode_long_content_and_pinned_revisions`: 10万Unicode文字の全文復元、更新後の旧版pin、ページ制限・revision/offset・Project境界の不正入力拒否。
- `bounded_metadata_pages_cover_many_notes_additions_and_histories`: 複数ページの全件到達、重複なし、本文なし、返却量、長い追記の過去出典・非表示前snapshot。
- `project_delete_temporarily_protects_design_history_without_data_loss`: 409と一時保護理由、Project/他のCASCADE子/本文/本文履歴/追記/追記履歴/receiptの全column比較、メモなしProject削除。

既存のMCP統合試験とE2Eは新しい読取契約に合わせて完全snapshotを復元してから元のassertionを行う。元の本文・初版・出典・件数assertionは維持。E2EにはDELETE 409と前後detail不変の確認を追加した。新規独立AIレビューは今回再依頼していない。

後続指示後に行う検証: `cargo test -p db design_note --lib`、`cargo test -p server design_note_mcp --lib`、`cargo test -p server project_delete_temporarily_protects_design_history_without_data_loss --lib`、認証HTTP回帰、関連Clippy・型検査/型生成確認・SQLx確認・frontend/binary build、その最終commitでの隔離 `node tests/design-notes/isolated.mjs`。設定画面の削除拒否表示もブラウザで確認が必要。今回はこれらを開始していない。

## 前回実装の検証記録（今回の変更には適用しない）

対象 `f0c44235bfc35475370bcdbcbac07c53e6a2db3a`。DB試験5件、外部MCP統合1件、HTTP認証1件が成功。`generate-types`/`:check`、`prepare-db`/`:check`、web-core/local-web型検査、local-web/変更箇所Lint、関連3crate Clippy、frontend build、server/agent-process-host/vibe-kanban-mcp buildが成功したというこのWorkspaceの前回記録。

隔離Chromium 147.0.7727.15、専用port、新規LVK_INSTANCE_DIR/SQLiteで、ソースとbinaryが同commit・dirty=falseを照合。複数メモ、初版/履歴/復元、MCP追記/再送、UI/UI・UI/MCP競合と下書き保持、訂正/非表示/統合、Project境界、安全なMarkdown、1440/390/320px、light/dark、再読込を検証。独立GPT-6 Astraレビュー指摘を修正し、試験プロセスと一時DBを片付けた。詳細ログ/画像はローカル `.build/design-notes/validation.md` に残る。

実Auth0/ChatGPT、Firefox/WebKit、実機モバイル、Tauri、全workspace一括試験、正式Integrationは前回も未検証。今回もmain merge/deploy/Wiki/Skill更新、稼働サーバー・共有DBへの変更は行わない。

## 追加再レビュー: MCP書込み結果の上限

今回もLOCAL-88のheavy検証との競合を避け、軽量修正・チェックのみ。Rust試験・型生成・ブラウザE2Eは後続GO待ち。

MCP成功結果の型を `DesignNoteReceipt { note: { id, revision }, addition: null | { id, revision } }` に統一した。本文・追記内容・title・source等の可変長文字列を含めない。UUIDと整数だけなので最大本文10万文字＋追記10万文字でも256 bytes未満の結果になる。stdioは既存RESTの完全なDesignNoteResultを受け取り、この型へ投影してMCPに返す。UI RESTの契約・保存先・DBの冪等性receiptは維持する。

外部HTTP MCPはoperation実行の成功時に投影してから結果を保存する。初回受付のoperation wrapper、再送、get_operationのstatus/operation_id/request_idは従来どおり。成功結果だけが小さいreceiptとなる。failed/uncertainの扱いや同じrequest_idのfingerprint検証は変更しない。過去の全文入り成功operationも共通receipt返却箇所でこの型に投影するため、取得・再送で全文を返さない。既存保存証拠は書き換えない。不正な保存結果は従来のreceipt_unreadable/uncertain経路で扱う。

revisionは**その書込みで確定した版**であり、再送時の現在版ではない。全文は `get_design_note` にnote_id・返却revision（追記はaddition_idと追記revision）を渡し、既存のbounded readで取得する。両MCPのtool descriptionに返却形と読取経路を明記した。Project削除の暫定409は変更なし。

追加回帰試験（未実行）:

- DB: `mcp_write_receipt_is_small_and_replays_committed_versions`。最大長の本文/追記、初回receipt上限、後続更新後も初回確定版を再送、重複なし、UI用結果の全文保持とbounded read復元。
- 外部MCP: `design_note_mcp_write_initial_retry_and_operation_are_bounded`。初回・再送・get_operationのwrapper上限、operation保存結果上限、全文の非返却、legacy全文入りreceiptの取得/再送投影、確定status維持、正本の全文/件数保持。
- 既存isolated E2Eに最大長の実stdio書込み・後続更新後の再送一致・receipt上限・bounded read全文復元を追加。既存assertionは維持。

今回実行した軽量チェック: 変更Rustファイルの `rustfmt --edition 2024 --config skip_children=true --check`、`node --check tests/design-notes/isolated.mjs`、`python3 tests/design-notes/bounded-read-sql.py`、`git diff --check` は成功。SQL smokeは既存read/deleteの回帰確認であり、今回のMCP write transport実行の証明ではない。Rustコンパイル・上記Rust試験・実MCP/ブラウザ実行は未実施。
