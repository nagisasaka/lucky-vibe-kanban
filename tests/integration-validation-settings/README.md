# 正式Integration検証設定の隔離試験

登録リポジトリの設定画面から `.lvk/integration-validation.json` を編集する。
startup/cleanupはRepoのDB設定だが、この設定は既存executorが読むGitファイルを正本とし、
DBへ複製しない。UIは登録checkoutのpath・branch・HEADを表示する。

保存は未commitファイルの更新のみ。利用者が通常のGit手順で目的の対象branchへcommitした後、
新規Integrationが受付target commitから全planを凍結する。現在checkoutしていないbranchを
UIから切り替えたり、直接commit/push/mergeしたりしない。既存run、一式検証、skipの意味は維持する。
commandは順番どおり、各要素を複数行のBashとして編集できる。JSON欄から既存不正JSONを修復できる。

保存時は既存Git publication guard（Git common directory単位）を使い、読込時の
branch/HEAD/ファイル存在/内容のSHA-256 revisionを比較する。同じUI・別名Repoからの保存は
直列化し、古いrevisionは409。1 MiBまでのUTF-8通常ファイルを対象とし、symlinkを拒否、
同一ディレクトリの一時ファイルから原子的に置換して読戻す。既存ファイルのpermissionを保持する。
外部editorや任意のGit操作はこのロックに参加しないため、比較直後の外部更新を完全排他する
契約ではない。保存中の外部編集・branch切替を避ける旨をUIに明記する。

## 実行コマンド

リポジトリルートで実行する。ブラウザ試験はloopback port 0の専用fixtureサーバーだけに接続し、
実環境のDB・設定・実行器を呼ばない。既存Chromiumを使用し、必要なら `LVK_E2E_CHROMIUM` を指定する。

```sh
pnpm install --frozen-lockfile --prefer-offline
pnpm run web-core:check
pnpm run local-web:check
pnpm --dir packages/web-core exec vitest run src/shared/lib/integrationValidationSettings.test.ts src/shared/lib/machineClient.test.ts
node tests/integration-validation-settings/browser.mjs
pnpm --dir packages/local-web exec eslint -c .eslintrc.cjs ../web-core/src/shared/lib/integrationValidationSettings.ts ../web-core/src/shared/lib/machineClient.ts ../web-core/src/shared/dialogs/settings/settings/IntegrationValidationSettings.tsx ../web-core/src/shared/dialogs/settings/settings/ReposSettingsSection.tsx --report-unused-disable-directives --max-warnings 0
node scripts/check-unused-i18n-keys.mjs
git diff --check
```

Rust実行枠が空いた後、通常のworkspace targetと既存sccacheを利用して順次実行する。
事前に共有資源snapshot・cargo/rustc・空き容量を確認し、AGENTS.mdのキャッシュ設定に従う。

```sh
CARGO_BUILD_JOBS=2 cargo check -p server
CARGO_BUILD_JOBS=2 cargo test -p server --lib routes::integration_validation::tests
CARGO_BUILD_JOBS=2 cargo test -p server --lib mechanical_validation_is_frozen_from_target_and_skip_never_reads_or_runs_commands
```

## 2026-10-24 開発検証記録（JST）

対象はbase `a4aab2d7e754e536781fc7f724f002bcf1925e6c` からの本変更。
最終対象commit SHAは実装完了報告に記載する（本記録自体もそのcommitに含む）。

- 08:06:25 開始。08:07:38まで既存repo設定・executor契約を調査（1分13秒）。
- 08:07:38–08:12:15 設計・API/UI・隔離fixture実装と依存準備（4分37秒）。
- 08:12:15–08:16:30 軽量検証・fixture修正・画面確認（4分15秒）。以後は最終記録・commit。完了時刻は完了報告に記載。
- web-core/local-web型チェック成功。Vitest 16件成功。変更4ファイルのESLint、翻訳キー検査、変更ファイルPrettier/rustfmt、diff whitespace検査成功。
- ブラウザmock: 読取、複数行保持、空コマンド拒否、追加・削除、保存後再読込、保存失敗、古いrevisionの拒否、読込失敗時の下書き保持、欠落ファイル作成、不正JSON修復、英語desktop/日本語mobile・light/dark・横overflowを確認。
- darkテーマの画面確認で旧Buttonの文字色を修正し、既存SettingsのPrimaryButtonへ統一。修正後のbrowser・型・scoped lintを再確認。
- fixture初回はstatus selectorがdirty表示にも一致し失敗、追加試験時は元の内容へ戻した後のdisabled保存ボタンを押そうとして失敗した。fixtureを修正し再実行。製品不具合の成功への読み替えではない。
- 最初のPrettierは依存未導入、ESLintは設定パス未指定で起動できなかった。依存導入と明示configで上記成功を確認。
- Rust check/test/linkは**未実行**。LOCAL128がRust枠を使用中との指示、および別cargo/rustc稼働を確認し、追加buildを開始しなかった。新規Rust fixture 7件と既存plan/skip回帰1件は統括の逐次実行待ち。rustfmt成功は型検査・実行成功を意味しない。
- 空き容量: 開始122 GiB → 08:10頃119 GiB → 08:13頃115 GiB → 08:16時点112 GiB。終了前の瞬間観測ではcargo/rustcなし。ただしLOCAL128のRust枠解放通知はなく、新規buildを開始しなかった。共有snapshotには登録資源・holder・queueなし。cleanや他workspace成果物削除なし。

未検証: 新APIの実HTTP/認証との接続、Rustのコンパイル・fixture実行、実executorによるcommand実行、実DB、正式Integration E2E、production build。browserは製品componentとmock APIによる検証であり、実サーバー保存を証明しない。
Rust fixtureは一時GitでJSON互換、非実行・index/HEAD不変、競合、symlink/サイズ/失敗保護を確認する設計。
既存plan回帰は保存→通常commit→targetからの読取と、commit前は反映されないこと、凍結後の作業木変更の無影響とskipを確認する設計。

スクリーンショットは `test-results/integration-validation-settings/` に保存しGit対象外。
一時bundle/CSSは終了時に削除し、browser/fixtureサーバーを停止する。通常のnode_modulesを再利用用に保持する。
main merge・デプロイ・再起動・実認証設定変更は実施していない。fresh独立レビューは統括側の別依頼を待つ。
