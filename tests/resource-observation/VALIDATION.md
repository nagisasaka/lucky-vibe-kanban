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
