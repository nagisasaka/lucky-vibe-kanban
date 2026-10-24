# CSV preview routes 検証記録

- Workspace: `57135236-91be-486e-b6eb-fb20e91fe594`
- 開始: 2026-10-24T10:56:48Z / 終了: 2026-10-24T11:03:28Z（UTC、合計6分40秒）
- testedSHA（基点HEAD）: `c60836658f2d2fdb11b1726b8f7e3e17a7b6a0c4`
- 試験対象はこのHEADに未コミット変更を加えたWorkspace。HEAD単独を合格扱いしない。
- 変更ファイル内容manifestのSHA-256: `707ba848b556e814d5ef241bb667ed8ae8d58b489eaa0f89846025d8b68c64fa`（本記録自身を除く、下記順序で各ファイルSHA-256＋空白2個＋パス＋改行を連結しSHA-256）。

## 追跡確認と工程

最初のコマンドで `git ls-files deploy/server` を実行。指定対象の `.env.example`, `compose.yaml`, `compose.acme.yaml`, `server.mjs`, `acme.py`, `nginx.conf.template` および既存JS/Pythonテストの追跡を確認。開始時の `git status --short` は空。

| 工程 | 時間（UTC） | 所要 |
| --- | --- | --- |
| 指示・追跡・既存実装調査、実装、局所試験 | 10:56:48–10:59:26 | 2分38秒 |
| 試験追加・整形・配布系一式検証 | 10:59:26–11:00:01 | 35秒 |
| fresh context独立レビュー、長いDNS名の失敗再現・修正 | 11:00頃–11:01頃 | 約1分 |
| 修正後の配布系検証、parser期待値補強・再検証 | 11:01頃–11:02:06 | 約1分 |
| 記録・一時物整理・最終差分確認 | 11:02:06–11:03:28 | 82秒 |

## 変更ファイル

- `deploy/server/.env.example`
- `deploy/server/acme.py`
- `deploy/server/acme.test.py`
- `deploy/server/compose.acme.yaml`
- `deploy/server/compose.yaml`
- `deploy/server/nginx.conf.template`
- `deploy/server/nginx.test.mjs`
- `deploy/server/preview-routes.cases.json`
- `deploy/server/server.mjs`
- `deploy/server/server.test.mjs`
- `docs/self-hosting/server-container.mdx`
- `docs/development/preview-routes-validation.md`（本記録）

## 試験結果

- **passed**: `LVK_TEST_COMPOSE=/bin/false LVK_TEST_NGINX=/usr/sbin/nginx LVK_REQUIRE_NGINX_TEST=1 node --test deploy/server/*.test.mjs` — 修正後40件中39 passed、1 skipped、0 failed（12.37秒）。Python単体5件は既存Node wrapperから実行。
- **passed**: 最後のfixture期待値追加後 `node --test deploy/server/server.test.mjs deploy/server/acme.test.mjs` — 18 passed、0 failed（1.74秒）。JS/Pythonが同じCSVを同じhost/port配列へ変換することを確認。
- **passed**: 隔離nginx構文検査・TLS/HTTP・WebSocket upgradeとsubprotocol・Authorization/Cookie/Host/Origin・HTTPS forwarded情報・Set-Cookie・カスタム認証ヘッダ保持。ループバック模擬upstreamとローカル自己署名fixtureのみ。新ルートの匿名HTTP到達とLVK側Basic保護の併存、253文字DNS名の実転送、各route SAN欠落の拒否を確認。
- **passed**: 空/未設定・legacy単独、同時設定拒否、5件以上、dev:4020、重複host/LVK_HOST衝突、予約ポート・範囲境界・scheme/path/IP/wildcard/注入/改行、253/254文字境界、同一upstream別名、外部証明書時101件、nginx生成。ACME 25 SAN境界・ホスト追加/置換/削除の引数、shortlived/IP、既存外部cert保持・atomic export・reload回帰、Compose env構造も確認。
- **passed**: `git diff --check`。
- **skipped**: Compose CLIによる展開試験1件。`LVK_TEST_COMPOSE=/bin/false`で明示無効化し、Docker操作を行わずYAML構造試験で両サービスのenv配線を検証。
- **skipped**: 無関係なRust全ビルド・UI E2E、Docker image/container試験。変更範囲とユーザー指定による。
- **unverified**: 実ACME発行・更新、実DNS、実VPS、実アプリ、稼働nginx reload、実配布image。Certbotは引数生成とローカルcert fixtureのみで検証し、本番/ステージングACMEへ接続していない。

初回の単体起動は `yaml` 未導入で失敗。Workspace内の一時ディレクトリーへ固定版yaml 2.8.2と整形用Prettierを用意して再実行し成功。既存依存/lockfileは変更していない。一時依存・ログ・編集scriptは完了時に削除。重いビルドは実行せず、ディスク残量は41GiB。

## 独立レビューと判断

別エージェント `independent_review` を会話履歴なし（fresh context）で起動し、要件・差分・リポジトリ指示から独立レビュー。

- P2: 有効な長いDNS名でnginxの既定 `server_names_hash_bucket_size 64` が不足。253文字のnative fixtureで `nginx -t` の失敗を再現した。
- 修正: templateに512を指定し、253文字のnginx/TLS/HTTP実行と254文字拒否を追加。修正後の試験は成功、レビュー担当も修正を再確認。
- 任意提案: Python側validケースも受理だけでなく解析後のhost/portを比較する。共有fixtureに期待値を追加し両言語で比較、関連試験成功。
- 未決・残存指摘: なし。上記unverifiedの実環境確認は別途ホスト担当の適用時作業。

既存のshortlived/IP運用を維持するため、ACMEのみLVK_HOSTを含む25 SANで拒否し、CSV件数自体は制限しない。[Let's Encrypt profile](https://letsencrypt.org/docs/profiles/#shortlived) と [注文制限](https://letsencrypt.org/docs/rate-limits/#new-orders-per-account) を2026-10-24に確認。複数証明書管理は導入せず、外部証明書を選択肢として文書化。

mainマージ、push、Wiki更新、デプロイ、DNS/firewall変更、秘密.env探索、稼働設定・コンテナー操作、実証明書発行は行っていない。
