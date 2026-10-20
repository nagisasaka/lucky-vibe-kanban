# LOCAL-23 検証記録（2026-10-20）

対象: cb1ac088-2c9c-442e-814c-7f001e710a6c / 125e1ae5-adbd-4313-9b89-2ffd6cb3d196 / 2267e45f-3611-40c6-be9f-fd40e9f5575f。

停止前の変更は checkpoint `8bce1740` に保存されていた。中断された検査は成功に数えず、再開後は重いコマンドをすべて逐次実行した。Rustはこのコマンドだけ `-j 1` とし、グローバル設定は変更していない。

## 変更と根拠

- Feedback: `shared/actions/index.ts` と `shared/command-bar/actions/pages.ts` の不要なアクションを除去。
- Discord: `AppBar.tsx` と `SharedAppLayout.tsx` の表示・人数取得を除去。
- GitHub: README.md、README.ja.md、origin が一致する `nagisasaka/lucky-vibe-kanban` を `packages/ui/src/lib/repository.ts` に共通定義。リンクとAPI取得先に使用。
- Local: AppBarの固定見出しに加え、実backendで発見したOrganization名由来のProjectヘッダーとモバイルメニュー見出しを除去。Organizationデータ、Workspaceのパス／ブランチ表示、既存ナビゲーションは維持。
- スターは0を表示。取得中、HTTP／通信失敗、不正／欠落値はバッジなし。失敗を0に変換しない。

## 実行コマンドと結果

以下は再開後の最終ソースに対する実績。Navbar修正後はweb-core型検査、local-web lint、フロントエンドbuildを再実行した。

| コマンド                                                                                                                                   | 結果            |
| ------------------------------------------------------------------------------------------------------------------------------------------ | --------------- |
| `pnpm run ui:check`                                                                                                                        | 成功            |
| `pnpm run web-core:check`                                                                                                                  | 成功            |
| `pnpm run local-web:lint`                                                                                                                  | 成功            |
| `pnpm run ui:lint`                                                                                                                         | 成功            |
| `pnpm --dir packages/local-web run build`                                                                                                  | 成功（tsc含む） |
| `RUSTC_WRAPPER=/usr/local/bin/sccache SCCACHE_DIR=<canonical shared cache>/sccache cargo build -j 1 --bin server --bin agent-process-host` | 成功、15分17秒  |
| `LVK_CHROMIUM_EXECUTABLE=/opt/lvk-browsers/chromium-1217/chrome-linux64/chrome node tests/header/isolated.mjs`                             | 成功、9ケース   |
| `git diff --check`                                                                                                                         | 成功            |

フロントエンドbuildには既存のBrowserslist、Tailwind content、chunk size警告がある。Rust workspace全体のtest/clippyはフロントエンド変更の範囲外として実行していない。

## E2Eの再現・期待・観測

自Workspaceのbuilt frontendと`target/debug/server`を使用。`LVK_INSTANCE_DIR`に専用の一時data/runtime/SQLiteを作成し、HOST=127.0.0.1、backendとpreviewはOS自動割当ポート。Playwrightの各ケースは独立BrowserContextで実行する。live LVK、nginx、他Workspaceは使用しない。

1. 実APIでProjectと独立Workspaceを作成し、両画面へ遷移。Feedback、Discord、単独Localが存在せず、GitHubリンクが公式LVK URLであることを確認した。
2. GitHub APIだけをmockし、取得中→0、0、1234、HTTP 403、負数、値欠落、通信失敗を再現。順に「なし→0」「0」「1.2k」、以降バッジなしを観測した。
3. 各ケースでProject／Workspaceボタンを操作し、往復遷移とCreate projectボタンの維持を確認。コマンドバーにもFeedbackがない。
4. dark/light、および390×844のモバイルメニューのWorkspaces入口と、作成済みWorkspaceからProjectへの戻りを確認した。入口確認後は作成ダイアログの自動表示と競合しないよう既存Workspaceを開く。
5. `/onboarding` は現行ConfigProviderのlocal設定で常にバイパスされ、RootRedirectPageにより唯一のProjectへ遷移することを確認した。到達不能なLandingPage内のSNSリンク変更は静的確認のみであり、ブラウザでのクリック成功とは扱わない。

実backend範囲は設定保存、Project／Workspace作成、取得、画面遷移。GitHubのライブ件数取得・到達性は未確認であり、mockの成功を実GitHub成功とはしない。

## 失敗履歴とcleanup

初回の実backend E2EはOrganization名由来のLocal表示を検出して失敗し、製品コードを修正した。以降、ドラッグ要素と実ボタンの同名セレクタ、Workspace初期遷移の期待値、localで非表示のonboardingに対する試験仮定、モバイルの自動作成ダイアログとの競合を修正した。これらの失敗や停止前の中断は成功扱いしていない。

popup待機の未処理Promise rejectionで一度残留した自己サーバー／CLI検出プロセスは所有範囲を確認してSIGTERMで停止し、一時instanceを削除した。試験はPromise処理とサーバーprocess group cleanupを補強済み。最終実行ではChromiumを閉じ、サーバーの正常終了、一時instance削除、自己サーバー／Chromium／probe残留なしを確認した。

証拠はignored `.build/header-validation/` の `results.json`、`cleanup.json`、`screenshots/`、`resumed/` 配下のログ。通常の`target/`、依存関係、frontend distは再利用用に保持する。merge、deploy、live再起動、TODO完了は行っていない。
