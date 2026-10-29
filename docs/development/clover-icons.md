# 四つ葉アイコンの正本と検証

LOCAL-235 の正本は `packages/public/lvk-clover.svg`（A、背景なし）と
`packages/public/lvk-app-icon.svg`（B、深緑角丸背景付き）。ユーザー提供原文の
SHA-256を生成スクリプトで照合し、両ファイルをそのまま保持する。
画面内ロゴ、アプリテーマ、manifestの名前・起動URL・scope・色などは変更しない。

2026-10-29のユーザー承認により、白背景を主版とする。
`lvk-app-icon-white.svg` はBの背景色 `#18251E` だけを `#FFFFFF` にした派生版。
四葉の形状・隙間・色 `#89F265`・配置・0.78の倍率は維持する。
faviconはlight/dark共通でこの白版SVGを参照する。
通常manifestの192/512px PNGと既存の `favicon.png`（32px）も白版から生成する。
Apple touch（180px）とmaskable（512px）は背景だけをキャンバス全面へ延長し、
四葉を動かさず、端末の切り抜きと不透明背景に対応する。

深緑バリエーションは正本Bと、`packages/public/icon-variants/dark-green/` の
`favicon.png`、`apple-touch-icon.png`、`lvk-icon-192.png`、`lvk-icon-512.png`、
`lvk-icon-maskable-512.png` に保持する。同じスクリプトで両バリエーションを再生成できる。

緑と白のコントラスト比は約1.40:1で、深緑版より低い。明るいタブ背景では白い外周も
背景へなじむ。16/32pxで四葉が欠けないことを検査するが、人間の識別しやすさや実機の
表示品質は別途確認する。承認済みの四葉色や形状を独断で変えない。

## 生成と検査

依存関係を `pnpm install --frozen-lockfile` で用意し、既存Chromiumを指定する。

```bash
LVK_CHROMIUM_PATH=/path/to/chrome node scripts/generate-clover-icons.mjs
LVK_CHROMIUM_PATH=/path/to/chrome node scripts/generate-clover-icons.mjs --check
pnpm --dir packages/local-web run build
LVK_CHROMIUM_PATH=/path/to/chrome node tests/server/run-clover-icons.mjs --static
```

`--check` は正本SHAの照合に加え、白版SVGと両バリエーションのPNGを再生成し、
保存済みファイルとバイト単位で比較する。生成環境のChromium版も揃えて確認する。

素材・参照だけの変更では `--static` を使い、専用のVite previewがビルド済みWebを
localhostの動的ポートで配信する。API proxyは無効にし、DBや他のserverに接続しない。
Rust配信経路も変更した場合は、必要なserverをこのcheckoutでビルドし、`--static`なしで
実行する。後者は固有の一時 `LVK_INSTANCE_DIR` と空DBを使い、live設定・資格情報を
継承しない。既存のpreview2や他Workspaceのプロセスは使用しない。

検査は正本のSHA、派生SVGの背景以外の完全一致、HTTP・MIME・配信バイト、Chromiumの
manifest解析、light/darkの16/32px描画、PNG寸法・白背景・背景を除いたB/白派生の四葉画素一致、
maskableの半径40%安全域と丸形表示を対象にする。保存画像の四葉ピクセルも検査する。
終了時に自分の配信プロセスを停止し、一時状態を削除する。レビュー画像と数値だけを
Git管理外の `.build/clover-white-review/` に保持する。
headless検証では、実機のホーム画面追加・OSの切り抜き・タブバーの美観は確認できない。

## キャッシュ更新

faviconは新しい白版SVGのURL、Apple touchとmanifest内のPNG参照は `v=white-1` を
使用して深緑主版のキャッシュと区別する。Web Pushのアイコン参照も同じ白版へ揃える。
インストール済みホーム画面はOS側に画像が残る場合がある。再読み込みで変わらない場合は
サイトキャッシュを確認し、既存ショートカットの削除・再追加を試す。
実機でのキャッシュ更新動作は別途確認する。
