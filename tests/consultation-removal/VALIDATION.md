# 相談コピー導線削除の検証記録

2026-10-23 UTC。Workspace `22507bed-b6cd-4165-aa11-61037ee5b8a8`。

## リビジョンと範囲

- 初期main／実装base: `aa59ab5855ae90b0fe3cf65a2017ff60be52df61`。
- 製品変更／frontend試験対象: `cabf350916ca309e50fa5c66b60f42e36108a247`。
- この検証commitは試験と報告のみ。上記SHA以降、packages・crates・docsの差分なし。
- backend: `/var/tmp/lvk-instances/dev/recovery/local65-aa59ab58-20261023/bin/server` の隔離コピー。
  `/api/info`の埋込revisionは初期main、kind=development、dirty=falseと実行時に照合。
- 後続の独立レビュー／Integrationは上位AIが担当。本Workspaceではmain更新・merge・deployを行わない。
  検証中にmainは外部で `291992a606a9ac386e3ee6f56e7d587bf91eb33e` へ進んだが、本試験はそこへの統合結果ではない。

## 実行結果

| コマンド | 結果 |
| --- | --- |
| `pnpm install --frozen-lockfile` | 成功。lockfile変更なし |
| `pnpm run web-core:check` | 成功 |
| `pnpm run local-web:check` | 成功 |
| `pnpm --dir packages/local-web exec eslint ../web-core/src/features/workspace-chat/ui/SessionChatBoxContainer.tsx --config .eslintrc.cjs --no-ignore --report-unused-disable-directives --max-warnings 0` | 成功。既存設定で変更箇所を検査 |
| `pnpm --dir packages/web-core exec vitest run src/shared/components/web-push/pushReturn.test.ts src/features/agent-runtime/model/canonicalAgentActions.test.ts src/features/agent-runtime/model/canonicalAgentConversation.test.ts` | 3ファイル33件成功 |
| `LVK_TEST_BACKEND=/var/tmp/lvk-instances/dev/recovery/local65-aa59ab58-20261023/bin/server node tests/consultation-removal/browser.mjs` | Chromiumで13確認項目成功。画像の取得条件を追加して最終再確認 |
| `git diff --check` | 成功 |
| `git diff cabf350916ca309e50fa5c66b60f42e36108a247 --exit-code -- packages crates docs` | 成功。検証中の製品差分なし |

前回の `node --test tests/web-push/*.test.mjs` は4件成功。対象ソースが変わっていないため今回は再実行していない。
frontendの型検査・lint・Vitestも成功後に重複実行していない。

## ブラウザで確認したこと

- fresh DBに実APIでProject・Issue・Workspace・Sessionを作成。canonical Runの状態／イベントのみSQLite fixtureで挿入。
- 質問待ち: 質問本文と回答欄、入力後のSendが有効。
- 承認待ち: desktopのApprove／Stop、mobileのApprove／Denyが有効。mobileのPending approval表示を維持。
- 完了: 完了メッセージと通知対象Runのsucceeded表示。Copy as Markdownをクリックして実クリップボード内容を照合。
- 以上3状態を1440×1000 darkと390×844 lightで確認。lightはDOMのtheme class切替。
- Project配下のURLで古い完了Runを指定すると、最新Runが待機中でも通知対象は古いRunのsucceededを表示。
- 通知パラメータなしのWorkspace画面でも相談導線なし。
- 相談ボタン、ChatGPTリンク、相談文textareaは0件。旧 `/web-push/context/` リクエストも0件。
- browser pageerror、HTTP 5xxは0件。UI上の回答・承認送信は行っていない。

差分レビューではRustは専用context routeとhandlerの削除のみ。`Path`等のimportは他handlerで引き続き使用。
通知worker・通知の対象Run表示・共通clipboard・質問回答・MCPは変更なし。
`git diff --exit-code aa59ab5855ae90b0fe3cf65a2017ff60be52df61 -- crates/server/src/remote_mcp crates/server/src/routes/sessions/questions.rs crates/db/migrations packages/public packages/web-core/src/shared/lib/clipboard.ts` でも差分なし。
削除専用テストは機能とともに削除し、維持する通知・canonical状態の既存テストは変更していない。

## 証拠と試験環境の後片付け

リポジトリの `.build/consultation-removal/` に保存:

- `results.json`: frontend／backend SHA、13確認結果、表示テキスト、エラー／旧APIリクエスト0件、終了・port閉鎖の検証。
- `awaiting_input-{desktop-dark,mobile-light}.png`、`awaiting_approval-{desktop-dark,mobile-light}.png`、`succeeded-{desktop-dark,mobile-light}.png`。
- `web-core-check.log`、`local-web-check.log`、`changed-lint.log`、`targeted-vitest.log`、`browser.log`、`server.*.log`、`vite.*.log`。
- `attempt1-results.json`～`attempt4-results.json`: 試験スクリプト調整時の結果も保持。
  最初はViteログ生成のrace、次は存在しないSubmit Answerラベル、次はdesktopにDenyがあるという誤った試験前提で失敗。
  実コードの入力後Send／mobile Denyに合わせて修正。4回目は13項目成功し、画像目視でmobile履歴のロード待ち追加が必要と判明。
  assertion削除で成功扱いにせず、実際のUIを待って操作可能性を確認した。

resource coordination snapshotはresources/holders/queueが空。登録共有資源は使用せず、試験専用instanceと未使用loopback portで実行。
各試行finallyでbrowserと自分のbackend/Viteを終了、port閉鎖を確認、DB・binaryコピー・fixtureを削除。
既存backend binaryは読取のみ、既存サーバー・他Workspaceは操作していない。

## 未検証・残る限界

- **変更後Rustのコンパイルおよび専用API削除の実動作は未検証**。旧backendは削除APIを含む。
  LOCAL-84のRustビルド中につき、ユーザー指示でcargoコマンドは実行しない。
  追加target checkの要否は後続の独立レビューで判断する。
- MCPの実取得／回答、providerへの回答／承認送信、非同期質問送信、実Web Push配信、OS通知、実機モバイル、Firefox/WebKit、製品buildは未実施。
  維持の根拠は変更境界のレビューと既存状態判定／表示／通知復帰の33件、前回Service Worker4件、今回Chromium fixtureであり、これらの未実施試験を成功とは扱わない。
- ViteのBrowserslistデータ古さ／Tailwind設定警告、未導入Claude検出警告はログに残る。
  executor選択欄のLoading表示はfixture環境のままで、実モデル起動可能性を検証していない。
- canonical OpenWikiは読取専用のため旧相談記述を編集していない。今回のmanifestから後続の正規保守で整合させる。
