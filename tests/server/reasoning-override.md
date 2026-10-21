# LOCAL-27: 手動推論深度の保持

対象TODO: `7079eefc-eb46-4061-b16b-8cc3543cd0d1`

## 再現と修正

変更前は discovery が空、または手動値 `max` を含まない場合、
`resolveReasoningOverrideState` が表示値を `null` とし、保存値も `null` に修復していた。
また、手動変更を差分だけでScratchへ保存すると、再表示時に継承したモデルIDとの
一致判定で推論深度が落ちるケースがあった。

変更後は一覧を表示用メタデータとして扱い、明示値を消さない。
既存Scratchに実効選択を保存し、古い部分的ドラフトはモデルを継承して復元する。
同じモデルの再選択では深度を維持し、別モデルを選ぶ場合は深度を明示的に解除する。
`null` は設定追従（画面の `Follow CLI config`）として保持する。
既存backendはNoneの推論深度をJSONから省略するため、overrideを含むScratchで
キーがない場合は設定追従として復元する。executor/variantだけのScratchはpreset選択
としてfallbackを許可する（下記P2修正）。保存形式は変更しない。
CLI/TOMLの読込方針、モデルの対応能力、backendの実行設定解決は変更していない。

## 検証方法

重いコマンドはすべて逐次実行する。backend buildはこのWorkspace固有の`target/`と
既存sccacheを使い、コマンド単位で`CARGO_BUILD_JOBS=2`とする。

```sh
pnpm --dir packages/local-web run build
# 既存のsccache設定を使い、このWorkspaceから実行
CARGO_BUILD_JOBS=2 cargo build --bin server --bin agent-process-host
node tests/server/run-reasoning-override.mjs
```

runnerは専用の`.build/reasoning-instance-*`を作成し、`LVK_INSTANCE_DIR`として設定する。
アプリとpreview proxyは空きloopbackポートへbindする。Playwrightは1 workerで実行し、
終了後は自分のserverを停止・待機して専用DB/data/runtimeを削除する。
`tests/server/playwright.config.ts`のglobal並列設定は変更しない。

実backend範囲: 自Workspaceでビルドしたfrontend/backend、Workspace・Session作成、
設定API、Scratch保存・WebSocket復元・送信後の削除、実アプリの描画と再読込。

mock範囲: provider discovery、最新launch設定API、follow-upの応答。
実モデルへの推論要求や実providerによるTOML解決を検証したものではない。
送信中・応答後の検証は合成した応答に対するUIと送信payloadの一致を確認する。

hook単独のChromium試験は実Reactと`useExecutorConfig`を使用するが、preset取得と
保存先はfixtureである。これを実backend試験と数えない。

## 受入操作

- 古い部分的ドラフトのHighを復元し、Maxへ変更してモデルIDとともにScratchへ保存。
- 同じモデルを再選択してもMaxを維持。
- discoveryを空にしてもMaxを表示し、メニューに保持中の選択として表示。
- Maxを含まない一覧へ更新後も、別Workspaceとの往復・再読込でMaxを維持。
- 送信payloadの`reasoning_id`が`max`であり、送信待ち・合成応答後・再読込でもMaxを維持。
- Follow CLI configへ明示変更すると`null`を保存・送信し、再表示でも設定追従を維持。

## 前回の実行結果（2026-10-20、HEAD 527f9c36まで）

| コマンド | 結果 |
| --- | --- |
| `pnpm --dir packages/web-core exec vitest run src/shared/lib/modelSelector.test.ts src/shared/lib/executorOverrides.test.ts --maxWorkers=1` | 23件通過 |
| `node --test tests/chat/executor-config.test.mjs` | 1件通過。controlled/uncontrolledの両経路、null省略、再mount、別scope、モデル変更を検証 |
| `pnpm --dir packages/local-web run build` | 最終ソースで通過（tsc + Vite） |
| `CARGO_BUILD_JOBS=2 cargo build --bin server --bin agent-process-host` | 既存sccache wrapper/cacheを指定して通過、9分52秒 |
| `node tests/server/run-reasoning-override.mjs` | 最終実行2件通過、1件skip（loopback HTTPのためHTTPS ingress検査は対象外）、14.5秒 |
| `pnpm run web-core:check` | 修正後通過 |
| `pnpm run ui:check` | 通過 |
| `pnpm run local-web:lint` | 通過 |
| `pnpm run ui:lint` | 通過 |
| 変更web-coreファイルへのESLint（下記） | 通過 |
| `git diff --check` | 通過 |

```sh
pnpm --dir packages/local-web exec eslint \
  ../web-core/src/shared/components/ModelSelectorContainer.tsx \
  ../web-core/src/shared/hooks/useExecutorConfig.ts \
  ../web-core/src/shared/lib/modelSelector.ts \
  ../web-core/src/shared/lib/executorOverrides.ts \
  --config .eslintrc.cjs --no-ignore --max-warnings 0
```

受入操作の全項目が最終E2Eで期待通りになった。Workspace切替はサイドバーの
ボタンで行い、ページ再読込も別途確認した。手動Maxのラベル・保持中の選択チェックと
Follow CLI configの表示は、Desktop Chromiumのlightテーマで画像も確認した。
保存値がnullのとき実backend応答ではキーが省略されることを踏まえ、再読込後の
表示と次の送信payloadのnullまで確認した。

最終instanceは空きloopback `127.0.0.1:33873`、
`.build/reasoning-instance-hA124w`。終了後、serverとinstance識別子を継承した
子プロセスの不在を確認して専用データを削除した。残存instanceディレクトリは0。
再開後にOOMは発生していない。mainへのmerge、deploy、live再起動、TODO完了操作は行っていない。

検出した失敗も成功扱いにしない。最初のE2Eは準備APIのCSRF欠落で失敗し、
通常のstatus取得とCSRFヘッダー付与に修正した。次のE2Eはbackendのnull省略を検出して
失敗したため、Scratchの復元処理を修正した。その途中のTS2322も修正後に型検査を
再実行した。中断前のbuild/tsc/lint/E2Eログは今回の成功の根拠に含めない。

未検証: 実モデル応答、providerによるTOML解決、HTTPS認証、dark/mobile表示。
fixtureはリポジトリ未接続のWorkspaceで、Git status/diffの検証は対象外。
既存のbuild警告（Browserslist、Tailwind content、chunk size）は残る。

ローカル証拠は`.build/local27/`、serverログは`.build/reasoning-override-server.log`、
画面画像は`test-results/server/reasoning-override-manual--81251-load-and-workspace-switches/`に保持。
通常の`target/`（約12GB）とfrontend出力・依存は再利用のため保持する。
中断時checkpointの`.task-7079/`（約52KB）は中断履歴として保持し、改変していない。

## P2レビュー修正（2026-10-20）

`setVariant` / `setExecutor` はidentityだけを保存するため、そのScratchのreasoning欠落を
明示クリアと扱うのは誤りだった。`resolveExecutorOverride` はidentityだけのドラフトを
通常のfallbackへ通し、overrideを含む保存済み選択についてはNoneの復元を維持する。
CLI/model policy、保存形式、設定のscopeは変更していない。

fixtureの固定preset応答をvariant別に修正した。Codex実装の`get_preset_options`に沿って
permission_policy/execution_modeを返し、High presetはmodel未指定とした。
controlled/uncontrolled各経路で以下を観測した。

- High preset選択時の保存値は正確に `{executor: "CODEX", variant: "HIGH"}`。
- 保存後・remount後ともに実効reasoningはhigh。
- 手動ultraの保存・remountでultraを保持。
- Follow Settingsの保存でnullが省略されてもremount後はnull（highに戻らない）。
- preset再選択・remountでhighへ復帰。

今回の検証は実React/hook + Playwright/Chromium、preset取得とScratch保存はmock。
backendのNone省略をfixtureで模倣している。上記の前回実backend E2E結果は今回の
修正ソースの実行結果ではない。最新指示のtargeted tests優先に従い、全面buildと
実アプリE2Eは再実施していない。実モデルへの要求も行っていない。

| コマンド | 今回の結果 |
| --- | --- |
| `pnpm --dir packages/web-core exec vitest run src/shared/lib/modelSelector.test.ts src/shared/lib/executorOverrides.test.ts --maxWorkers=1` | 24件通過 |
| `node --test tests/chat/executor-config.test.mjs` | 1件通過、内部でcontrolled/uncontrolled両方を実行 |

既存形式で全overrideが省略されidentityだけになった過去データから、当時の操作意図を
復元することはできない。その形は既存setVariant/setExecutorの契約に従いpreset選択
として扱う。モデル未指定でも通常のCodex presetにはpermission_policyとexecution_modeが
含まれるため、その実効選択を保存する明示クリアは区別できる。

追加静的検査: `pnpm run web-core:check`、
`pnpm --dir packages/local-web exec eslint ../web-core/src/shared/lib/executorOverrides.ts --config .eslintrc.cjs --no-ignore --max-warnings 0`、
`git diff --check` はすべて通過。heavy commandは逐次実行。
Chromiumはfinallyでcloseし、試験終了後の自Workspace cwdに紐づく
Chromium/esbuild/serverプロセスは0件。今回試験サーバーや専用DBは作成していない。
