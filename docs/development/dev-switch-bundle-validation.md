# 開発版bundle切替・自己再起動の検証記録（2026-10-24）

LOCAL-102、Workspace `beda1e84-1565-4ec3-8552-77939ec1f6da`。
基準は `737252a5ae00208400ec0f5bd2243ad423017753`。実装対象は本記録と同じcommitの差分。
稼働版 `0227e740…` と区別する。独立レビューは上位側の別fresh contextで実施する。
本作業は製品コードと隔離fixtureのみで、main merge、live停止/再起動/切替、live起動設定・DB・成果物変更、
旧74a5 Workspace変更、Wiki更新、plugin更新は実施していない。

## 変更と契約

- 固定launcher、単一`launch.json`、原子的`current.json`へ移行。通常入力はbundle。
  source/frontend/frontend_mode/retain_worktreeと重複switch.json、engine/config環境変数を廃止。
- 稼働serverがPID/start/boot、解決済instance/data、実bind済listener、current、実際の起動設定hashを渡す。
  workerが停止直前に独立検証し、変更された起動設定・対象PID・hash・listenerを拒否する。
- debug `developer-runtime` featureでfrontend/SoundAssets/ScriptAssetsを埋込む。
  コピー済み実行ファイルの起動probeと実asset hashを検査し、manifest宣言だけでは受理しない。
  server/agent-process-host/vibe-kanban-mcpをreleaseへcopy、運用instance cwdと絶対host/PATHで起動する。
- 既存operation/request ownership/lock/pidfdを再利用。受付終了はpidfdで待ち、zombieを稼働子と誤認しない。
  procfs走査中の消失競合も処理する。汎用run_processの制約・scope/region/principal確認は維持。
- 停止後にWAL込みbackupを検証、server startupがmigrationを実行。成功時だけrelease2世代・backup1セットへ整理。
  candidate/backup作成中は一時3世代/2セット。失敗は保持し、新要求をfenceする。自動DB rollbackなし。

[MCP/CLI、初回legacy移行、失敗時確認手順](../../deploy/developer/switch/README.md)を正本とする。
初回は外部terminalのoperatorが旧runtimeと復旧資料を保全して固定kitへ移行する。今回は実環境移行していない。
以後はscriptを編集せずbundle指定で切り替える。旧CLI/MCP fieldは暗黙変換せず明示的な契約移行を要求する。

## 実行証拠

生ログはこのWorkspaceの `.task-evidence/` に保持。秘密を含み得る任意server logを報告へ転載していない。

| コマンド・対象 | 結果・証拠 |
| --- | --- |
| `pnpm install --frozen-lockfile` | 成功、`pnpm-install.log` |
| `pnpm --dir packages/local-web run build` | tsc/Vite成功、`frontend-build.log` |
| `CARGO_BUILD_JOBS=2 cargo build -p server -p local-deployment -p mcp --bin server --bin agent-process-host --bin vibe-kanban-mcp --features server/developer-runtime` | debug生成成功。初回11m19s、最終整合確認29.38s。`build.log`, `final-build.log` |
| `python3 -m unittest discover -s deploy/developer/switch -p test_switch.py -v` | 26成功、`python-tests.log` |
| `PYTHONPATH=deploy/developer/switch python3 -m unittest -v test_switch.SwitchTest.test_installed_cli_resolves_instance_without_duplicate_input test_switch.SwitchTest.test_two_artifacts_and_server_owned_migration test_switch.SwitchTest.test_async_duplicate_and_caller_exit` | 最終CLI既定値とdirectory fsync変更後3成功（うち新規1）、`python-final-tests.log` |
| `CARGO_BUILD_JOBS=2 cargo test -p server --features developer-runtime --lib remote_mcp::environment -- --test-threads=2` | 25成功、自己再起動fixtureの起動設定hash渡し漏れで1失敗、補助3ignored。`rust-runtime-tests.log` |
| `CARGO_BUILD_JOBS=2 cargo test -p server --features developer-runtime --lib remote_mcp::environment_switch -- --test-threads=2` | fixture修正後2成功、補助host1ignored（親testがsubprocessとして起動）。`rust-switch-retest.log` |
| `target/debug/deps/server-8368c6b9dc31cfe6 routes::workspaces::archive_processes --test-threads=1` | source使用判定の既存境界2成功、`archive-tests.log` |
| `python deploy/developer/switch/test_real_server.py --bundle BUNDLE --source .` | 実debug bundleでmigrationあり/なし、source不可視runtime、UI index/JS/CSS/API、host bootstrap、同梱stdio MCPとWorkspace cwd成功。`real-bundle-tests.log` |
| 同じ実binaryからdebug symbolだけ除いた試験用bundleで追加検証 | 実server停止後のasync worker完走・同receipt再取得も成功、`real-async-tests.log` |
| 対象PythonのBlack、変更Rustのrustfmt、`git diff --check` | 成功。`format-check.log`, `rust-format-check.log` |

Rustコマンドは `RUSTC_WRAPPER=/usr/local/bin/sccache`、canonical共有cacheの`SCCACHE_DIR`を指定。
既存sccache serverは別保存先を報告したため停止・再設定せず記録した。統計は他clientと共有であり本作業単独の値ではない。
`test_real_server.py`用にtask専用venvへpysqlite3-binaryを導入し、実ユーザーDBを使わず過去空schemaを作った。
SQL履歴構築は試験だけの責務で、切替engineには入れていない。

実bundleのSHA-256（最終build後も全3ファイル一致）:

```text
server             6e56acbc5e3bfe356623f295b3a4efa7b121a598caec65f5c08568b08ced9d96
agent-process-host 84e6adc50b6cc0aa9fefef47b49d4bfafcaf93bef5d58df37226b93152ea5443
vibe-kanban-mcp    030ee2d80959c946978b93d170749b69fefe4caba223db493bdb346681a08e1a
```

build metadataは基準SHA、development、dirty=true。checkout HEADだけからbinary由来を断定していない。
`tested-bundle-manifest.json` と `tested-async-bundle-manifest.json` に全実行ファイル・asset hashを保存した。
異なる2候補、copy後の元build不可視、停止前不良で旧server継続、PID/設定不一致、
呼出元終了、worker中断、duplicate/入力違いID/並行拒否、owner/grant/scope、
backup/health失敗、世代保持、WAL、migration有無を差分リスクに沿って検証した。

## 検証境界と観測

- source不可視は子runtimeへのLandlockで実施。mount namespaceは環境に許可されていなかった。
  engineまでLandlockで隔離するとprocfs identity確認が拒否されるため、engineは対象確認可能なままにした。
  親Workspaceをrename/chmodせず、子がsourceのCargo.tomlを読めないことを確認した。
- production MCP routerの認証fixtureによる呼出元停止・再接続と、実server binaryを停止するasync worker試験を分けた。
  実serverに実Auth0/ChatGPTをつないだ一体のMCP E2Eではない。
- 実host bootstrapとWorkspace cwdの同梱MCP初期化を検証した。実provider経由のAgent実行・課金model・
  HOME認証・browser UI操作・実Auth0は未実施。OS/Python/Git/provider CLI等は運用配置の互換性依存として残る。
- 既存`/mcp/*`画像URLの一部は予約routeにより404だった。切替healthはindex/JS/CSSのHTTP hashを確認し、
  他assetはembedded probeで検査する。既存画像route修正へ範囲を広げていない。
- initial fixtureのvenv実行path、Landlock domain、設定hash渡し漏れ、procfs消失競合の失敗は修正して再検証した。
  本記録にない全体check/lint/backend:test、正式Integration、Docker/release image、live移行は未実施。

## 容量と同期

初期空き約40GiB。debug target・test link・bundle複製で一時約6.6GiBまで減少した。
追加観測を受け、大きな生成を重ねず、終了・非稼働を確認した自作fixtureとbundleだけを整理した。
追加async試験は新規buildをせず、symbol除去コピー約547MiBを使った。通常のworkspace targetは後続レビュー用に保持し、
他Workspace/live/recovery/sharedcacheの資材は削除していない。終了時の容量は最終応答に記載する。

Git Skillは付録原文と完全一致:
`26988 bytes / SHA256 3a6db747cddaec3f45af4f37a7043a218a29c4f8a15993600db7bf66794eaad8`。
plugin側は未変更。旧Skillのengine/config表現と新固定kit/launch.jsonの運用差はREADMEに分離して記載した。
リリース直前のlive plugin版再照合は後工程に残す。
