# LOCAL-161 外部エディターURI連携の実装・検証

基準HEAD: `eda9ff3f89c337b4010496ccc59a2c6219378ed7`。
実装Workspace: `f9b98f72-b1df-4428-a96e-0cc4d584c053`。
通常の開発検証であり、正式Integration・mainマージ・デプロイは実施しない。

## 受入条件との対応

- `EditorConfig.entries` の `null`／欠落は未初期化、`[]` は意図的な空一覧。
  通常のConfig読込・保存でブラウザhostnameを初期VS Code URIへ一度固定する。
  初期行も通常の名前・URIデータで、専用初期化APIやロックは追加しない。
- 設定・オンボーディングで名前とURIの追加・編集・削除・並べ替えが可能。
  表示名は空白のみ不可、80文字以内、trim後の完全一致で重複不可。
  URIは8192 UTF-8バイト以内。構文不正は設定保存時に理由を返す。
  設定画面の既存deep mergeで削除行が残らないよう、editor一覧は全置換する。
- URI展開はpath／query値の2箇所。pathはスラッシュを保持、query値は全体を
  percent-encodeする。既存の固定 `%XX` は保持し、変数内の `%` は `%25` にする。
  path展開が意図せず`//`から始まるauthorityを作る場合も構文エラーとする。
  scheme／authority／query名／fragmentの変数、未知変数、ファイルのない
  `<lvk_file_path>` は適用不可。URI schemeの安全性審査は追加しない。
- サーバーAPIはリンク候補を返すだけ。CLI起動、Customコマンド実行、IDE別分岐、
  拡張導入、availability APIを撤去。ブラウザには解決済みの実リンクを出し、
  明示クリックで委譲する。リンクには `noopener noreferrer` を付ける。
  再確認中／エラー時に古いキャッシュのリンクを出さない。
- 単一Repoはworktree、複数RepoのWorkspace操作はroot、Repo操作は対象worktree、
  Repo設定画面は登録済み元Repo。ファイルは所属Repoと相対パスから解決し、
  既存の相対パス正規化とcanonical境界検査を再利用する。
  差分のRepo名接頭辞は送信前に除き、プレビューと同じRepo相対形式にする。
- 実体参照には `inspection_root` を使用する。
  `container_for_inspection` は非archive時にworktreeを作成し得るため使わない。
  欠落したディレクトリ・ファイルはエラーにし、復元・作成・touchしない。
- 旧IDEアイコン・翻訳・生成型・設定ガイドを整理。URI自体をonboarding analyticsへ
  送らず、登録件数のみ記録する。

## 検証結果

時刻はUTC（JSTは翌2026-10-25の+9時間）。Rustは全コマンドで
`CARGO_BUILD_JOBS=2`、既存sccache、Workspace固有の`target/`を使用。

| 開始〜終了（UTC） | コマンド | 結果 |
| --- | --- | --- |
| 16:42:58〜16:43:07 | `cargo test -p services services::config` | 合格：16件 |
| 16:27:27〜16:31:52 | `cargo test -p server --lib routes::workspaces::integration::editor_tests` | 合格：2件 |
| 16:24:26〜16:24:30 | `pnpm --filter @vibe/web-core exec vitest run src/shared/lib/externalEditors.test.ts src/shared/types/actions.test.ts src/features/workspace-files/ui/WorkspaceFileInspection.test.tsx` | 合格：32件 |
| 16:38:35〜16:38:41 | `node --test tests/chat/external-editors.test.mjs` | 合格：1件（複数シナリオ） |
| 16:38:33〜16:38:57 | `pnpm run web-core:check` | 合格 |
| 16:40:03〜16:40:30 | `pnpm run local-web:check` | 合格 |
| 16:32:44〜16:33:27 | `pnpm run generate-types:check` | 合格 |
| 16:44:25〜16:44:45 | `cargo clippy -p services -p server --lib -- -D warnings` | 合格 |
| 16:40:03〜16:40:13 | `pnpm --filter @vibe/local-web exec eslint --config .eslintrc.cjs --no-ignore <変更したweb-core本体TS/TSX>` | 終了0：エラー0、変更前からの警告1 |
| 16:29:10〜16:29:16 | `node scripts/check-unused-i18n-keys.mjs` | 合格 |
| 16:45:14〜16:45:15 | `pnpm --filter @vibe/web-core exec prettier --check <変更したWebファイルと外部エディターfixture>` | 合格：変更したWeb本体・fixture |
| 16:45:15〜16:45:16 | `rustfmt --check --edition 2024 --config skip_children=true <変更したRustファイル>` | 合格：変更したRustファイル |

ブラウザ試験は実際の設定コンポーネント・Config controller・選択ダイアログを
Chromiumへ読み込み、APIと保存先を隔離fixtureに置換したもの。初期値保存、編集・
並べ替え、空一覧保持、ファイル／Repoリクエスト、適用不可理由、遅延応答後の
明示クリック、Webリンクのopener切断、古いリンクの無効化を確認した。
390px／1100px、ライト／ダークの横はみ出し検査を行い、画面画像も確認した。
設定画面本体を通した2件登録→全削除→保存と、保存失敗理由の表示も確認した。

Rustのサーバーテストは実パス解決関数を一時ディレクトリ上で検証する。
実DB・稼働サーバー・外部IDEプロセスは使わない。

## 途中で判明した制限

- 最初の `pnpm run generate-types` は型ファイル生成まで出力されたが、呼出しの
  終了コードは143だった（原因未特定）。これを成功として数えず、後続の生成チェックは合格した。
- 旧Configの全体移行テストをv2まで拡大した最初の試行はv5で失敗した。
  変更前から、v7の移行入口はv6形式を直接deserializeし、それ以前まで連鎖しない。
  本件ではこの別問題の移行基盤を変更しない。v6〜v9は全体読込みと他設定の保持、
  v2〜v5は各旧schemaで未知IDE／Custom値が読込みを壊さないことを検証した。
  v5以前から現行Configへの一括移行は今回の保証対象外。
- 変更したweb-core本体への追加ESLint検査には差分ビューの
  `topBandCandidatesRef.current` に関する既存警告1件がある。
  `git show HEAD:.../ChangesPanelContainer.tsx` を同じESLintへstdinで渡し、
  変更前にも同じ警告が出ることを確認した。無関係な修正は加えていない。
  初回にはlocal-webのtsconfig対象外のVitestファイルを渡した設定エラーもあり、
  本体lintとVitest実行を分けた。

## 未実施・保証しないこと

- Windows Chromeからの実IDE起動、WSL／VPS SSH認証、Docker Attach、IDE内フォルダ表示。
  Dev Containersへの直接ネストURIも保証しない。
- 稼働LVKバイナリ・DB・起動構成・recoveryへの変更、既存他プロセスの停止。
- フルE2E、全Rustテスト、正式Integration、mainへの統合、デプロイ、Wiki更新。
  ブラウザfixtureの合格を実機接続や稼働サーバーでのE2E成功とは扱わない。

## 開発生成物

Workspace内の `target/`（約25 GiB）と `node_modules/`（約981 MiB）は
再検証で再利用する通常の開発出力として保持する。sccacheの実保存先は指定の
共有cacheと一致し、Rustのcache hitも観測した（統計は共有クライアントの集計）。
一時スクリーンショット・作業ログ・ビルド用のダミーfrontend indexは完了時に削除する。
本番frontendの配布ビルドやデプロイは行っていない。
