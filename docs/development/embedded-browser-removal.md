# LOCAL-247 内蔵ブラウザの廃止と検証

## 採用仕様と境界

ユーザーは2026-10-27に内蔵iframeとclick-to-componentの廃止を採用し、実装を許可した。監査とClaude.aiレビューはLOCAL-247の既存コメントに保持されている。実装開始時のIssue本文revisionは `2026-10-27 14:29:07.290`、HEADとローカルmainは `a3247dadd5b4385e07a7848fd0009aa07d8ffa71`。この変更は独立worktreeでの実装・隔離検証であり、main統合・push・配布・稼働サーバー変更を含まない。

- DevServer操作を既存のLogs画面に集約した。script編集、開始・停止、process状態、repo別ログ選択、非ゼロ終了コード、検出URLのコピーと別タブ表示を残す。開始時に開くパネルはLOGS。
- iframe、画面サイズ・ナビゲーション、click-to-componentの新規選択・送信、Eruda/bippy/独自postMessage、専用Rust proxy、両起動経路の追加listener、preview専用認証grant/cookie/route/configを除去した。専用proxyの終了が本体終了を引き起こす結合もなくなる。
- 管理DevServerのUI/stdio/remote共通manager、汎用認証・session・CSRF、独立CSV公開、Chromium/Playwright/CDP、ファイル/Markdown/diff/画像プレビューは維持した。サイドバーhover表示やViteのbuilt-asset previewは別機能。

## 意図的な互換処理

適用済みmigrationには変更を加えない。`access_preview_*`テーブルも残すが、認証付き公開機能は再開しない。旧 `PREVIEW_SETTINGS` Scratchの型とdecode、過去チャットの `vk-component` / `component-info` rendererを残す。旧PREVIEWパネルの保存値はLOGSに復元し、廃止画面へ戻さない。新規クリック選択のproducer・store・送信導線は残さない。

`LVK_PREVIEW_DOMAIN`、`VK_PREVIEW_DOMAIN`、`PREVIEW_PROXY_PORT`、`VK_PREVIEW_DISABLE_NAV_SCRIPT` は警告して無視する。古い設定の値が不正、旧portが使用中、またはport値が0でも、それ自体では起動を拒否せず追加listenerも作らない。Composeの旧domain入力は廃止警告のためだけに渡す。

`LVK_PREVIEW_ROUTES` のhostname/port検証、予約port集合、TLSの全hostname検証、HTTP/WS/HMR、アプリ側のAuthorization/Cookieを維持する。歴史的なproxy用予約portも今回緩和しない。旧wildcardからCSVへの自動変換は行わず、認証付き経路を無認証で公開しない。CSVのアプリは従来どおりアプリ自身が認証を担う。

外部ブラウザ・WSL・SSH tunnel・公開URLの手順と、意図的に失うレビューUXは [Browser Testing](../browser-testing.mdx) に記載した。検出URLのhostを閲覧元やlocalhostへ自動変換しない。アプリの到達性・readinessはprocess runningから推測しない。URL表示とboundedなログ解析以外の公開管理層や自動tunnelは追加しない。

## 検証

表は修正後の最終結果であり、初回のfixture期待値・未使用変数等による失敗を成功に数えていない。実行証拠はworktreeの `.build/local247/` に保存する。画像は実際のbuilt CSSを使用したcontrols fixtureのPC/モバイル・明暗テーマと、隔離standaloneの初期表示。fixtureは管理hookの観測をmockし、実manager/stdio/remoteの検証とは区別する。

| 検証                                                                                                        | 結果・確認範囲                                                                                                                                                                                                     |
| ----------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `pnpm run local-web:check` / `web-core:check` / `ui:check`                                                  | 成功                                                                                                                                                                                                               |
| `pnpm run local-web:lint` / `ui:lint`、未使用i18n key検査                                                   | 成功                                                                                                                                                                                                               |
| `pnpm --dir packages/local-web run build`                                                                   | 成功。既存のbundleサイズ/Browserslist警告あり                                                                                                                                                                      |
| `pnpm run generate-types`、生成binaryの `--check`                                                           | 成功。API infoから旧proxy設定を除去し、歴史Scratch型を維持                                                                                                                                                         |
| `node scripts/dev-cargo.mjs check -p server -p mcp`                                                         | 成功。関連consumerを含む                                                                                                                                                                                           |
| `CARGO_BUILD_JOBS=4 node scripts/dev-cargo.mjs test -p server -p mcp -p server-access -p db -p utils --lib` | 601件成功、13件は隔離専用としてignore。内訳 DB142 / MCP9 / server389 / server-access21 / utils40。子process helperの出力を重複加算しない                                                                           |
| `LVK_TEST_NGINX=/usr/sbin/nginx LVK_REQUIRE_NGINX_TEST=1 node --test deploy/server/*.test.mjs`              | 39件成功、Compose実行1件のみ環境不在でskip。native nginx 7件を含む                                                                                                                                                 |
| `python3 deploy/server/acme.test.py`                                                                        | 5件成功。実証明書発行は行わない                                                                                                                                                                                    |
| `node --test tests/embedded-browser-removal/controls.test.mjs tests/mobile-preview/*.test.mjs`              | 4件成功。外部URL/clipboard、repo選択、旧component表示、旧panel値、失敗表示、Chromium CDP、無関係なbuilt previewを確認                                                                                              |
| `python3 tests/embedded-browser-removal/lifecycle.py`                                                       | 成功。新規専用instanceの起動停止2回、所有socketがmainだけ、旧設定の無害化、旧API不在、port/info JSON、旧ScratchのAPI取得/list、migration checksum/session保持、実built frontendとCDPを確認。instanceは終了後に削除 |

`CARGO_BUILD_JOBS=4 LVK_DEV_SERVER_REAL_AGENT=0 node tests/server/run-managed-dev-server.mjs` は成功。実stdio catalog/start/status/stop、remote HTTPのscope/target/receipt/concurrent-start、UIと共通の管理状態、2回のcold start、process cleanupのsettlementを確認し、fixtureを削除した。`CARGO_BUILD_JOBS=4 node scripts/dev-cargo.mjs clippy -p server -p mcp -p server-access -p db -p utils --all-targets -- -D warnings` も成功した。初回の `items_after_test_module` 指摘は関数の定義位置だけを修正し再検証した。通常ignoreされた残りの別用途E2Eや、実有料agent、実Auth0、実DNS/ACME、実VPS/WSL/SSH接続、実アプリのHMR再接続、Docker image/Compose実行は本検証の成功に含めない。稼働DBを複製・参照しての試験ではなく、新規DBに旧形式を投入して再起動した互換fixtureである。


## 独立レビュー追補: SSEを保持した通常停止

`2d0f2044fe7b76fe08be305fa3da38970cfecb20` の停止処理には回帰があった。実有効な `/api/events`（末尾slashの `/api/events/` は404）で200・`text/event-stream`・本文を確認し、接続を閉じずSIGTERMを送ると、mainは12秒後も終了せず、`kill_all_running_processes` に到達しなかった。`startup::ServerHandle::serve` でも同じ条件で終了tokenをcancelすると12秒超過した。これは無期限停止を理論だけで断定したものではなく、12秒の監視上限まで停止しない実測である。従来の `lifecycle.py` はブラウザ/HTTP接続を先に閉じていたため、この条件を網羅していなかった。

両経路のHTTP drainに5秒の上限を設けた。猶予を超えると警告してserve待機taskをabortし、既存の実行プロセスcleanupへ進む。cleanup自体を途中で打ち切る処理や、新たなproxy・設定・接続管理基盤は追加していない。残存HTTP接続の最終切断はruntime終了時に行われる。ライブラリ呼出し側がruntimeを維持する場合、`serve()`復帰が全接続の強制切断を保証するわけではない。

Unix隔離回帰は次の手順で実行する。通常のCargo共有build-dirを使い、別targetや稼働instanceを指定しない。

```bash
CARGO_BUILD_JOBS=4 node scripts/dev-cargo.mjs test -p server --test shutdown --no-run
# 上の出力にある Executable tests/shutdown.rs の絶対パスを指定する。
python3 tests/embedded-browser-removal/shutdown.py --mode both --library-test-binary /path/from/cargo/shutdown-HASH
# 接続を先に閉じる対照ケース。
python3 tests/embedded-browser-removal/shutdown.py --mode both --library-test-binary /path/from/cargo/shutdown-HASH --close-sse-before-stop
```

`shutdown.py` は毎回専用一時instance/DBを作り、mainのSIGTERMと、SIGTERMを終了tokenへ変換するライブラリfixtureの両方を検査する。cleanup到達とexit code 0、ライブラリでは`serve()`復帰も確認する。空の新規DB（実行プロセス0件）での試験であり、実行中agentの全停止ケースをこの試験だけで保証しない。修正前の再現確認に限り `--expect-hang` を指定できる。監視上限超過時も自分の子processだけをkill/waitしてから一時instanceを削除する。


追補の実測（2026-10-27 UTC、SSEのSIGTERM送信から監視結果まで）:

| 条件 | main | ライブラリ `serve()` | cleanup |
| --- | --- | --- | --- |
| 修正前・SSE保持 | 15:28:30開始、12秒超過 | 15:28:58開始、12秒超過 | 両方未到達、自分の子processを強制終了 |
| 修正後・SSE保持 | 15:32:31開始、5.025秒、exit 0 | 15:32:52開始、5.029秒、exit 0 | 両方到達・完了 |
| 修正後・SSEを先にclose | 15:33:13開始、0.010秒、exit 0 | 15:33:29開始、0.015秒、exit 0 | 両方完了、猶予超過warningなし |

追補時の `python3 tests/embedded-browser-removal/lifecycle.py` も成功し、旧Scratch/migration/session保持、単一listener、通常起動停止2回、同梱Chromium/CDPを再確認した。新テストのJSONL（時刻・cleanupログ・exit code）と実binaryのSHA-256はworktreeの `.build/local247/shutdown-*.json*` に保持する。修正前後とも新規一時DBだけを使い、稼働dev/stable/previewや実DBには操作していない。

追補の `CARGO_BUILD_JOBS=4 node scripts/dev-cargo.mjs clippy -p server --all-targets -- -D warnings` は成功。別の `check` はClippyとtest buildが関連targetのコンパイルを行うため重複実行しない。フロントエンド/API型/DB schema/公開設定には追補差分がなく、既存の全体テストを一律には再実行していない。

追補の `CARGO_BUILD_JOBS=4 LVK_DEV_SERVER_REAL_AGENT=0 node tests/server/run-managed-dev-server.mjs` は1件成功（10.70秒）。実stdio/remote MCP、共通状態、2回のcold start、計2件の管理processとcleanup settlementを確認し、専用instanceを削除した。有料実agentとSSEの同時停止は未実施。

試験準備中の失敗も区別する。初回fixture buildは、ソースに存在する `write_port_file` の未解決importで失敗した。該当ソースのmtimeだけを刷新して再buildすると同じソース内容で成功したため、共有中間成果物の鮮度不整合が疑われるが根本原因は未調査。キャッシュ削除や設定変更は行っていない。初回SSE harnessは末尾slashの404を使ってstartup待ち上限に達したため、有効経路に訂正してから修正前後を実測した。これらを正常停止の成功試験として数えていない。
