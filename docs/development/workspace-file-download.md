# Workspace raw download filename investigation

調査基準 SHA: `45fdda98d586940ef45411ac6fcd2872e9143fad`。
関連 TODO: `4d7d9b5a-bf67-42b6-864b-5816788cf875`。

## 確認した経路と原因

- `WorkspaceFilePreviewHeader.tsx` のダウンロードアイコンは `content.raw_url` を通常の `<a target="_blank">` で開く。`download` 属性はない。`UnsupportedFileViewer.tsx` の raw リンクも同様。
- `workspace_file_raw_url` は `/api/workspaces/{id}/files/raw?repo_id=...&path=...` を生成する。元名はクエリにあり、URLパスの末尾は `raw`。`scopeWorkspaceFileRawUrl` は現在このURLをそのまま返す。
- 基準 SHA の `serve_workspace_file_raw` は非画像に `application/octet-stream` と `Content-Disposition: attachment` のみを指定し、`filename` と `filename*` を両方欠いている。安全な画像はインラインで、同ヘッダー自体がない。
- 確認できた不具合は**保存名メタデータの欠落**。[HTML の保存名選択手順](https://html.spec.whatwg.org/multipage/links.html#downloading-resources)では、ヘッダーの名前情報も有効な `download` 属性の候補もなければURL由来の名前にフォールバックできる。この経路でURL末尾の `raw` が候補になる説明は成立するが、URLからの具体的な名前選択はブラウザ実装依存である。報告者の skill.md の操作、環境、実際の保存名は今回再現していない。

`filename` と `filename*` が両方ある場合、対応クライアントは後者を優先する（[RFC 6266 §4.3](https://www.rfc-editor.org/rfc/rfc6266#section-4.3)）。[RFC 8187 §3.2.1](https://www.rfc-editor.org/rfc/rfc8187#section-3.2.1) に従い UTF-8 のバイトをパーセントエンコードし、空白は `%20` とする。フォーム用の `+` は空白の表現として使わない。

## 最小修正と不変条件

既存raw応答に、正規化済み要求パスのbasenameから `filename="SKILL.md"; filename*=UTF-8''SKILL.md` を追加した。日本語等の正確な名前は `filename*` に保持し、古いクライアント向け `filename` はASCIIの安全な代替にする。引用符・バックスラッシュ・制御文字・パーセントを代替し、ヘッダー注入や古いブラウザの二重解釈を避ける。空白・大文字小文字・拡張子を意図的に変換しない。

画像も `inline` のまま名前情報を付与する。HTML/SVG等は従来どおりattachment/octet-stream/nosniff。新規エンドポイント、フロントのBlob経由ダウンロード、依存ライブラリ追加はない。応答構築のみを局所関数に分けて実ストリームの全バイト比較を可能にした。

認証の `Access.layer`、Workspace読込ミドルウェア、`WorkspaceRepo::find_by_workspace_and_repo_id` による所属確認、相対パス正規化、canonicalize後のルート配下検査、ファイルopenとディレクトリ拒否は変更していない。名前はパス全体やsymlink解決先名ではなく、一覧と同じ論理パスのbasenameを使う。既存の非UTF-8ファイル名表現、ブラウザ/OSによる危険名の保存時正規化の仕様は拡張しない。

## 検証と引継ぎ

- 局所ファイルテスト: ASCII、`SKILL.md`、日本語、空白、記号、改行注入、basename/大小文字、NUL拒否、相対/絶対/Windows drive/親パス拒否、symlink escape拒否。
- 実ファイル応答テスト: プレビュー上限より大きい全256バイト値を含むファイルを読み、body全体が元バイト列と一致すること、length/cache/nosniffとHTML/SVG/画像の表示方針を確認。
- 認証境界テスト: 本番と同じAccess layerをrawパスと実応答構築に適用し、未認証/無効cookieの401と、有効Web sessionでの名前・bodyを確認。全本番routerや実Project/Workspace DBを組み合わせるE2Eではない。
- 既存Workspaceミドルウェアの拒否テストにも `/files/raw` を追加。所属確認自体は無変更のソース照合であり、新たなProject認可ルールは設けない。

実行結果（2026-10-26）:

- `node scripts/dev-cargo.mjs test -p server --lib routes::workspaces::files::tests -j 3`: **14件成功**。共有Cargoロック待ちを含め9分25秒、テスト本体0.03秒。
- 上記が生成した `server-721a8eab9263a24f` テスト実行ファイルを、`middleware::model_loaders::usage_tests::public_workspace_and_session_routes_reject_before_handler_side_effects --exact --nocapture` で直接実行: **1件成功**。同一成果物を使い、別の重いCargoビルドを増やさず確認した。
- `node scripts/dev-cargo.mjs fmt -- crates/server/src/routes/workspaces/files.rs crates/server/src/middleware/model_loaders_tests.rs`: 成功。
- `git diff --check`: 成功。
- scoped testでserverと依存のコンパイルを確認。別途のcheck/clippy、全workspace test/buildは並行作業の負荷と本差分の範囲を考慮して省略し、正式Integrationに委ねる。

ブラウザ実測、実Auth0、正式Integration、release build、main merge、稼働サーバー再起動は本Workspaceで実施しない。統合担当 `prepare_all_unmerged_integration` / 上位AIへ引き継ぐ。
