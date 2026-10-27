# UI4 integration and preview2 handoff

Workspace: `25e0971a-1bff-4564-b0b8-bba84bb4eb07` / branch `vk/25e0-ui4-235-237-240`。
共通基点main: `a3247dadd5b4385e07a7848fd0009aa07d8ffa71`。
mainへのmerge、push、本体deploy、旧preview停止・設定上書きは行っていない。

## 合成と保持した仕様

開始時に各commitの存在・変更内容と共通基点を照合し、以下の順でcherry-pick。競合なし。

| Issue | 入力SHA | この検証branch上のSHA |
| --- | --- | --- |
| LOCAL-235 | f71014765d20b2f41d742e1c10e1d86ee0f54de3 | b8df88106cefc04b1bd5130de1f2d3d514e8cf70 |
| LOCAL-237 | 59c1f3cbc0b766217b1018d718d649f862b84f93 | 82a90381c4eb488c6ecacf29a609d9b08b586058 |
| LOCAL-240 | 5b7fcd86a7cb466ec6aa8b7b2ff94f847645d62d | a61b4297d85428fec20b1bc19aa1d2c41cb9d031 |
| LOCAL-241 | 9a275b6efb82a7f889b171d20e757bc76acb70ef | b82224b99f702cfe4aa7a0d3e2ebf2b32b53f4b4 |

`KanbanGrid`のtouch guard import/call/capture spreadとsortable transform/transition/dropAnimationを両保持。タグtouch処理、f64 JSON入力、既存順位競合保護、保存・同期直列化は各元commitのまま。追加の製品仕様変更はない。LOCAL-247等は取り込んでいない。

## 検証コマンド

repository rootで実行。`PLAYWRIGHT_BROWSERS_PATH=/opt/lvk-browsers`、`LVK_TEST_SERVER="$PWD/target/debug/server"`を各backend付きブラウザー試験へ指定。

- `pnpm install --frozen-lockfile`
- `pnpm --dir packages/local-web run build`
- `pnpm run web-core:check` / `pnpm run local-web:check` / `pnpm run ui:check`
- `pnpm run local-web:lint` / `pnpm run ui:lint`
- `pnpm --dir packages/local-web exec eslint --config .eslintrc.cjs ../web-core/src/features/kanban/ui/KanbanGrid.tsx ../web-core/src/features/kanban/ui/useTouchClickGuard.ts ../web-core/src/features/kanban/ui/KanbanContainer.tsx ../web-core/src/features/kanban/ui/kanbanDropSync.ts --no-ignore --max-warnings 0`
- `node --test tests/kanban/order.test.mjs tests/kanban/drop-sync.test.mjs`：15件PASS。
- `node tests/kanban/touch-guard.mjs` / `node tests/kanban/issue-classification.mjs`
- `node tests/kanban/readability.mjs`：crisp/bouncy別実行。
- `node tests/kanban/touch.mjs`：trusted mobile touch/tag scroll・release後tap・cancel・長押し・mouse/keyboard/focusと実API保存。
- `VITE_KANBAN_MOTION=crisp node tests/kanban/animation.mjs` と `bouncy`：1列360px/2列600px、不揃い高さ、displacement/drop animation/easing・focus・実API。これらの専用試験のreload付きケースは連続正常系の合格には算入しない。
- `node tests/server/run-clover-icons.mjs`：実serverのmanifest/icon、Chromium取得と画像hash確認。
- `LVK_UI4_CONTINUOUS=1 LVK_DND_REGRESSION=1 VITE_KANBAN_MOTION=crisp node tests/kanban/isolated.mjs` と `bouncy`：最終連続試験。結果・時刻は `.build/ui4/continuous-final-results.json`。

上記の最終検証はすべてPASS（型3種、lint3種、unit15、Rust1+3、各Chromium試験）。最終連続試験はbouncy: 2026-10-28 01:20:05.540–01:21:43.123 JST（97.583秒）、crisp: 01:21:43.124–01:23:18.799 JST（95.675秒）。animation専用はcrisp: 01:14:46.691–01:15:51.203（64.512秒）、bouncy: 01:15:51.205–01:16:55.287（64.082秒）。いずれも起動・待機込み実時間。

最終連続試験は同一pageでnative touch scroll→summary tap開閉→長押し同列小数順位保存→同じカードをkeyboardで先頭へ戻す→保存900ms/確認1100ms遅延→連続keyboard→mouse列間移動→移動先keyboard並替→mouse再移動までreloadなし。各段階でDOMと実APIの順序/statusを照合。以降、実activity競合、保存500/同期503注入、15秒同期timeout/明示再同期、非manual sortを検証。Running表示だけはsummary fixtureで、provider実行の証拠ではない。

一時SQLite・loopback backend/Viteを使い、最終harnessは環境allowlistと無動作codex/claude discovery fixtureを使用。実DBを試験に使用せず、AgentRun・モデル実行を起動しない。

## binary identityとRust検証

このcheckoutの通常targetへ `node scripts/dev-cargo.mjs build -p server --bin server -j 4` を実行。最終buildでは次を明示した。

```sh
LVK_BUILD_REVISION=b82224b99f702cfe4aa7a0d3e2ebf2b32b53f4b4
LVK_BUILD_DIRTY=true
LVK_BUILD_KIND=development
```

実行binary: `target/debug/server`。
SHA-256: `831f814a53acc88a0bf35b086552b9152b8ba43ab689409f0305421fedf864ff`。
`/api/info`も上記full revision / development / dirty=trueを返した。dirty差分は検証harness・preview準備で、compiled Rust/Cargo入力はこのrevisionから差分なし。`backend-identity.mjs`はfull revision、Rust/Cargo差分なし、binary hashを各専用試験で検査・記録する。後続の検証commitとbackend build revisionは区別する。混在artifactをPASSとはしない。

同じbuild identityで実行：

```sh
node scripts/dev-cargo.mjs test -p server --lib -j 4 bulk_issue_decimal_ranks_deserialize_from_http_json
node scripts/dev-cargo.mjs test -p server --lib -j 4 issue_activity_
```

小数入力1件・既存activity/競合3件PASS。合成後の`.5` HTTP probeも200。Rust全体check/clippy/test、production配布build、他browser、実スマホは未実施。

初回frontend buildは依存未導入の`tsc: not found`で失敗し、frozen install後にPASS。最初のRust buildとfrontend buildが並行した際のplaceholder警告は、実frontend build完了後の最終server再buildで解消。共有cache削除・sccache停止・他Workspace artifact書換えはしていない。開始時available memory約14.45GB、8 CPU、最終Rustはjobs4。ピーク前後もメモリ余裕を観測した。

## 観測結果・失敗を区別する

- 両motionの1列・不揃い高さでは移動中の非drag sibling重なり1組、2列では0組。スクリーンショットも確認。既存`rectSortingStrategy`のまま。最終触感/easingの選択は人間へ留保する。
- 最初のbouncy連続試験は、固定期待順位2.5に対し保存3.5で失敗した（HTTP200）。animated collisionの隣接target差を記録し、最終試験は小数中点・近傍移動・DOM/API全順序一致を確認する。失敗traceを `.build/ui4/continuous-bouncy-fixed-target-failed/` に保持。
- readabilityの初回はesbuild iifeの`import.meta.env`未置換でrender失敗。fixtureのdefineを補正。bouncyの取消試験はkey受付前のEscapeでghost残留を検出し、100ms key duration・aria-pressed確認を使って再検証。製品コードは変更していない。
- preview smokeの初回は公開Hostなしで403を受け、423期待が失敗。次にTIME_WAITをlistenerと誤判定したためSO_REUSEADDR付きbindへ修正。通常`.build/...`はSPA fallbackの200なので、最終の保護検証は`/@fs/絶対path`が403であることを確認。これらの途中失敗は最終成功へ書き換えずログ保持。
- 今回のログでSQLite busy再発は観測していない。backendのtransaction・再試行仕様は変更していない。

## preview2を上位AIが管理起動する手順

既存nginx実設定 `/tmp/lvk-server/nginx.conf` の公開経路は `https://preview2.luckyvibekanban.com` → `127.0.0.1:18444`。旧所有Workspaceは `2f33322d-1002-4f67-88ea-6cf4890c0c92`、process `a3b37dc1-b70b-4014-a35d-979e35a6e253` はkilled・cleanup settled。開始時18444にlistenerなし。旧Workspaceのstart.sh/run.pyを読取確認し、DB・Basic・session・tags・設定は無変更。

共有Repository設定は既に `exec bash .lvk-preview/start.sh`。このWorkspaceの同じlocal入口から `scripts/ui4-preview.py` をforeground実行する。start.shは既存Git excludeに従うlocal fileで、設定を共有Repositoryへ書き戻していない。

- 管理対象Workspace: `25e0971a-1bff-4564-b0b8-bba84bb4eb07`
- Project: `2e066b19-db27-4b8a-943e-ea097aa601c3`
- frontend/Vite: loopback18444、backend: loopback18446、公開/basicを強制。
- data: このcheckoutの `.build/ui4-preview/instance/data/`。設定は `.build/ui4-preview/config.json`。
- fixture: synthetic Issues30、inert Workspaces2、tags3、AgentRuns0。project IDは `.build/ui4-preview/fixture.json`。
- motion: 暫定crisp。bouncyは管理起動環境の`VITE_KANBAN_MOTION=bouncy`で選択。人間の最終選択は未決。
- source frontendはHMR。backend hash/Rust入力が変われば起動を拒否し、意図的な再build・再検証・config更新が必要。
- 起動時port占有を拒否し、他processを停止しない。終了時は自分のbackend/Viteをterminate→waitし、必要時だけ自分のchildをkillする。

上位AIは起動直前に共有resource snapshot・portと管理statusを再確認してから、既存 `start_dev_server(project_id, workspace_id, request_id)` / UI管理入口を使う。現在のsource checkoutを保持する。終了後の存続と管理stopは上位AIが確認する。このrunから恒久serverは起動しない。

新規DBの認証は未設定のため**locked**。旧preview資格情報をコピーしていない。起動後、管理者が同じLVK_INSTANCE_DIRで公式 `server setup-token` を使い、既存手順でBasicを設定する必要がある。tokenは今回発行していない。ログイン済み人間による調整はこの設定後であり、その受入は未確認。認証無効化・proxy/DNS/firewall/tunnel変更は行わない。

一時smokeで2026-10-28 01:17:19–01:17:23 JST（4秒）、外部root200/API423、内部公開Host付きAPI423、直接保護path403を確認し、foreground wrapper/backend/Vite終了exit0。`.build/ui4/preview-smoke.json`参照。これは管理DevServerのrun終端後存続を実証したものではない。

## 証跡とcleanup

`.build/ui4/`にcommand logs、suite時刻・結果、最終連続crisp/bouncyのresults/revision/hash/trace/screenshots。`.build/kanban-animation-{crisp,bouncy}/`に専用animation traceとgeometry、`.build/touch-validation/`にtrusted touch event・screenshots・cleanup、`.build/clover-review/`にicon取得結果。`.build/ui4-preview/`は要求された人間preview用に保持する。通常targetとnode_modulesも保持する。

各testのfinallyでowned Chromium/Vite/backend終了・使い捨てinstance削除。preview用に保持したのは最初の成功連続試験のfixtureのみ。最終のlistener/process確認は完了時報告へ記録する。共有resource snapshotは登録・holder・queueとも空で、登録済み共有device/DB/deploymentの利用はない。

全体作業とbuildの厳密な開始終了時刻は未記録。計測したE2EのJST時刻と経過は `.build/ui4/suite-results.json` と `.build/ui4/continuous-final-results.json` から参照できる。待機を含む実時間であり、active CPU時間ではない。
