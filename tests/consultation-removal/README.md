# 相談コピー導線の削除：隔離ブラウザ検証

変更frontendをViteで読み込み、明示指定の既存backendを一時ディレクトリに
コピーして起動する。Rustをビルドせず、有料モデルを呼ばない。DB、Git fixture、
Workspace、Session、canonical Run記録は試験専用。既存サーバーは操作しない。

```sh
pnpm install --frozen-lockfile
LVK_TEST_BACKEND=/path/to/authorised/aa59ab58/server node tests/consultation-removal/browser.mjs
```

backendの埋込revisionは初期main
`aa59ab5855ae90b0fe3cf65a2017ff60be52df61` と照合する。
backendの削除対象APIは旧binaryに残るため、本試験はAPI削除の実動作を検証しない。
共有資源を使う場合は実行前にresource coordinationの登録・利用契約を確認する。

対象は質問待ち、承認待ち、完了、通知が古いRunを指すProject配下画面、通知なしの
Workspace。desktop/darkとmobile/lightで相談ボタン・リンク・textareaの不在と
既存の質問・承認・Run表示を確認する。モバイルthemeはCSS classで切り替える。
完了メッセージのCopy as Markdownは実クリップボードで照合する。
質問回答・承認の送信は行わず、操作可能なUIまでを確認する。

`.build/consultation-removal/` にJSON、画面画像、backend/Viteログを保存する。
finallyで試験用browserと子プロセスを終了し、listenerが閉じたことを確認して、
隔離DB・binaryコピー・fixtureリポジトリを削除する。

製品変更は `cabf350916ca309e50fa5c66b60f42e36108a247`。
詳細な実施結果・未検証範囲は同ディレクトリの `VALIDATION.md` を参照する。
