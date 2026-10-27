# 四つ葉アイコンの正本と検証

LOCAL-235 の確定素材は `packages/public/lvk-clover.svg`（A、背景なし）と
`packages/public/lvk-app-icon.svg`（B、深緑角丸背景付き）。原文コメントから復元し、
生成スクリプトに記載した SHA-256 と照合する。正本の形状・隙間・色を変更しない。
既存の画面内ロゴやテーマ設定の変更はこの作業に含めない。

favicon は light/dark 共通で B を使う。背景付きなので明るいタブでも四葉が背景に
埋もれにくい。通常の manifest アイコンは B を 192/512px に描画する。
Apple touch（180px）と maskable（512px）は B の背景だけをキャンバス全面へ延長し、
四葉のパス・配置・0.78 の倍率は維持する。端末による丸形等の切り抜きに備え、
四葉の全ピクセルが中心から半径40%のセーフゾーンに収まることを確認する。
既存 `favicon.png` も B の32px版へ更新する。

依存関係を `pnpm install --frozen-lockfile` で用意した上で、既存 Chromium を指定する。

```bash
LVK_CHROMIUM_PATH=/path/to/chrome node scripts/generate-clover-icons.mjs
pnpm --dir packages/local-web run build
CARGO_BUILD_JOBS=4 node scripts/dev-cargo.mjs build -p server --bin server
LVK_CHROMIUM_PATH=/path/to/chrome node tests/server/run-clover-icons.mjs
```

E2E はこの checkout の `target/debug/server` を起動し、固有の一時
`LVK_INSTANCE_DIR` と空のDBを使用する。live 設定・資格情報は継承しない。
HTML/manifest/画像のHTTP・MIME・配信内容、Chromiumのmanifest解析、light/darkでの
16/32px描画、PNGサイズ、不透明背景、maskableの安全域を確認する。
終了時に自分のserverを停止して一時DBを削除し、レビュー用のスクリーンショットと
数値結果だけを `.build/clover-review/` に保持する。headless の検証なのでブラウザの
タブバーそのものの美観、実機でのホーム画面追加・OS側の表示は別途確認する。

favicon/manifest内の新しい画像URLとApple touchのバージョン付き参照により、旧画像の
キャッシュと区別する。既存のインストール済みホーム画面はOS側に画像が残る場合がある。
再読み込みで変わらない場合はサイトデータのキャッシュを確認し、ホーム画面の既存
ショートカットを削除して再追加する。今回、実機キャッシュの更新動作は未検証。
