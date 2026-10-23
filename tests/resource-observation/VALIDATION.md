# LOCAL-37 追加実装の検証記録（2026-10-23）

作業ブランチ `vk/5416-mcp` の clean 状態を確認し、開始時点の main
`707bb0c3` を fast-forward で取り込んで実装した。main への変更、deploy、
restart、Wiki 更新、TODO 完了操作は行っていない。自己確認の記録であり、
独立レビューは上位AI側の別工程。

## 変更と共通処理の再利用

- `crates/server/src/resource_observation.rs` と同ディレクトリの `linux.rs`:
  既存MCPの観測器を共通モジュールに移動。`remote_mcp/resource_observation.rs`
  と `routes/resource_observation.rs` が同じ `observation()` を呼ぶ。
  同じ2worker上限・2秒deadline・約500ms CPUサンプルを使う。
- `resource_observation/disk.rs`: LVK data / worktree base のディレクトリhandleを
  `fstatvfs` で観測。filesystem ID、使用率、空き容量を対象別に返す。
  合算・親filesystemへの推測・未作成ディレクトリの作成は行わない。
- `crates/utils/src/{assets,instance,path}.rs` と
  `crates/worktree-manager/src/worktree_manager.rs`: 起動時初期化を維持しつつ、
  観測専用のread-only保存先解決を追加。
- `crates/server/src/{lib.rs,routes/mod.rs}`: 共通モジュールと既存認証配下の
  `GET /api/resource-observation` を接続。
- `packages/ui/src/components/{Navbar,ResourceSummary}.tsx`、
  `packages/ui/src/lib/resourceObservation.ts`、
  `packages/web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx`、
  `packages/web-core/src/shared/hooks/useResourceObservation.ts`:
  CPU/RAM/DISKを右側アイコン群の左に表示。70/80%で緑/橙/赤を切り替え、
  unknown・失敗・45秒以上のstaleは灰色の「—」。可視ページのみ15秒周期取得。
  DISKはD/Wを分け、空き容量と対象をtooltipに表示。LOAD/SWAPは表示しない。
- `docs/development/chatgpt-mcp.md` と `tests/resource-observation/`:
  API・表示契約、model/Chromium試験、実行手順と本記録を追加。

## 実行結果

Rustは `CARGO_BUILD_JOBS=1`、test threads=1、既存sccache、既存targetを使用。
重い処理は直列。途中の資源保留指示に従い新規起動を止め、14:59の再許可後に
必要な検証のみ再開した。初回のRust応答型エラーは修正済み。

| コマンド | 結果 |
| --- | --- |
| `pnpm install --frozen-lockfile` | 成功 |
| `pnpm run ui:check` | 成功 |
| `pnpm run web-core:check` | 成功 |
| `pnpm run local-web:check` | 成功 |
| `pnpm run ui:lint` | 成功 |
| `pnpm run local-web:lint` | 成功 |
| `node --test tests/resource-observation/model.test.mjs` | 4件成功 |
| `node tests/resource-observation/browser.mjs` | 12ケース成功、page errorなし |
| `cargo test -p server resource_observation --lib -- --test-threads=1` | 最終read-only修正後18件成功、明示smoke 1件は別実行 |
| `cargo test -p utils instance::tests --lib -- --test-threads=1` | 2件成功。既存初期化と未作成instanceの非作成を確認 |
| `cargo test -p server resource_observation::tests::live_smoke --lib -- --ignored --exact --test-threads=1 --nocapture` | 1件成功、負荷追加なしの小規模read |
| `cargo clippy -p server --all-targets -- -D warnings` | 成功 |
| 変更Rustファイルの `rustfmt --check` / `git diff --check` | 成功 |

MCPのread scope、未認証/権限不足、任意path引数拒否、Webセッション必須と
write method拒否、部分失敗、timeout時の取得済みDISK保持を確認。
CPU delta/duration、cgroup unlimited/quota/cpuset、DISKの予約領域を含む計算、
別filesystem・欠損、70/80境界、可視/非可視と取得の共有をfixtureで検証した。

## Chromium証跡とsmoke

リポジトリの `test-results/resource-observation/` に保持（Git対象外）。
実Navbar・表示component・取得hookを隔離loopback fixtureで動かし、外部通信を遮断。
試験用ブラウザー、HTTPサーバー、一時CSSは終了・削除済み。

- `results.json`: 12ケースの結果・幅・取得回数・cleanup。
- `normal.png`, `warning-70.png`, `high-80-dark.png`: 正常/警告/高負荷とテーマ。
- `below-70.png`, `below-80.png`: 境界直前。
- `failed.png`, `stale.png`, `missing.png`: 失敗・期限切れ・欠損。
- `narrow-320.png`, `narrow-project-320.png`: 320px幅の横あふれなし。
- `smoke-summary.json`: 実行ログから記録した小規模readの要約。

smokeの観測時刻は `2026-10-23T15:05:20Z`、CPU sampling durationは
500.29519ms、completionはcomplete。両保存先のfilesystem IDは64771で同一だった。
これは観測時点の値であり、実行安全性や容量予約を保証しない。

## 未検証・制約

- Chromiumはcomponent/API fixture試験。稼働中LVKを変更したfull-app E2E、
  実Auth0/ChatGPT接続、非Linux環境は未検証。
- 実測環境の2保存先は同じfilesystem。異なる実mountは作らずfixtureで検証。
- DISKは設定されたdata/worktree baseのみ。任意DirectFolderやその配下の別mountを
  網羅しない。未作成/不可読の保存先はunknown。
- CPU/RAMはsystem-visibleであり、物理hostやcgroup内の割当量を示すとは主張しない。
  LVK自体が停止・フリーズすれば観測APIも応答できない。
- full workspace/Tauri/正式Integrationは今回の対象外。独立レビューは未実施。
- 通常の `target/`（約26GB）と `node_modules/`（約1GB）は継続作業・再利用用に保持。
  Chromium/smoke証跡は約0.3MB。無関係なprocessのkillやcleanupは行っていない。

## 独立レビュー後の実アプリ受入追加（同日）

上記は初回commit `77ebfec3` までの記録。独立レビューから、Navbar直描画の
fixtureでは実right icons・40px user slot・実router接続を証明できないとの指摘を
受け、`full-app.mjs` を追加した。今回はそのcommitからの追加変更であり、mainを
取り込み直していない。独立レビューは上位AI側で実施済みで、本節は実装者の
追加試験記録。

実NavbarContainerのproject/320pxでは389pxに広がり、Settingsとuser slotが
画面外に出ることを再現した。モバイルだけ、観測値とアイコン群をそれぞれ
まとまりとして折り返す。アイコンや機能を減らさず、desktop構造は維持した。
`Navbar.tsx` が今回唯一の製品コード変更。既存の共通collector/API/hookは再利用し、
専用サンプラーや認証の迂回は追加していない。

テストは実server binary・built local-web・一時DBを使う。loopback TLS proxyと
ローカルBasic認証でSecure cookieを維持し、実ログイン画面からログインする。
Auth0、本番DB、既存serverには接続しない。Integrationはパネル表示のみで実行しない。
空の隔離workspaceにはrepositoryを登録していないため、Git/file系APIに通常の
「repositoryなし」エラーが出る。これは資源APIの取得成功とは区別する。

追加試験結果:

| コマンド/範囲 | 結果 |
| --- | --- |
| `pnpm --dir packages/local-web run build` | 成功。既存distはplaceholderだったため実bundleを生成し、Navbar修正後も再生成。tscを含む |
| `CARGO_BUILD_JOBS=1 cargo build -p server --bin server` | 成功。古いbinaryを共通観測実装入りに更新。sccache/既存target利用。製品Rustの追加変更なし |
| `pnpm run ui:check` / `pnpm run web-core:check` / `pnpm run ui:lint` | すべて成功 |
| `node tests/resource-observation/full-app.mjs` | 最終16ケース成功、uncaught page errorなし、同時資源リクエスト最大1 |
| `GOMAXPROCS=1 UV_THREADPOOL_SIZE=1 node tests/resource-observation/browser.mjs` | Navbar修正後のcomponent回帰12ケース成功、cleanup成功 |
| `git diff --check` | 成功 |

実routerでは未認証GET 401、実Basicフォームログイン後GET 200と`no-store`、
有効CSRFを付けたPOST 405を確認。通常観測はChromium自身が実APIへ送信し、
観測値の置換は100%/503/遅延/staleケースだけ。実測DISKの両targetが隔離保存先の
`stat().dev`と一致し、filesystem IDと空きGiBがtooltipに届くことを確認した。

12の画面ケースはproject root / issue subroute / workspace ×
320 / 767 / 768 / 1280px。すべてdocument/nav横幅、表示buttonの左右座標、
tooltip左右座標がviewport内。モバイル実user slotは40px、projectには実Integration・
Command Bar・Guide・Settingsも含む。4幅すべてでIntegrationパネルも開いた。
追加4ケースは全値100%の赤表示、503、6秒遅延（client timeout 5秒）、45秒stale。
44秒前の観測時刻を返し、実hookの時計更新で数値から灰色へ変わることを確認した。

非表示は`document.visibilityState`を上書きして`visibilitychange`を発火し、
実時間17秒で取得が止まり、再表示で1回再開することを確認。OSによる実タブの
background化の検証ではない。実SPAのworkspace→project→workspace遷移では
fresh queryの重複取得なし。境界70/80、別filesystem、欠損は既存fixtureを再利用。

証跡は `test-results/resource-observation/full-app/`:

- `results.json`: 実router認証、16ケースの座標・tooltip、保存先照合、polling、cleanup。
- `*-header.png` / 同名`.png`: tooltipなしの実アイコン群 / tooltip表示。
- `integration-{320,767,768,1280}.png`: 実Integrationパネル。
- `project-100-percent-320*.png`, `api-{503,delayed,stale-45-seconds}*.png`。
- `before-fix-project-320.png`, `before-fix-results.json`: 修正前389pxの再現。
- `server.log`: 隔離serverのみ。外部通信遮断と未登録repositoryに由来する
  console/networkエラーは結果に保持し、uncaught page errorとは分けている。

試験harnessの応答照合・DISK子要素の色判定を修正し、TLS proxyのkeep-aliveと
route対象を絞ってasset取得の不安定性を解消した後の最終結果。最終実行では
asset取得の予期しない失敗なし。中断したharnessの私有scratchは、該当portの
非listenを確認して削除した。最終cleanupも全項目trueで、私有instance残留なし。

追加範囲でも実Auth0、別実mount、非Linux、agent実行、正式Integration、配布済み
サーバーは未検証。通常target約29GB/node_modules約982MBは再利用用に保持し、
証跡約2MBを保存。重いbuildは直列・job1、他作業のkill/cleanupはしていない。
