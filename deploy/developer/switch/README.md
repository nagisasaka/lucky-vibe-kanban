# 開発版の成果物切替と自己再起動

開発者専用です。安定版image更新・停止済みserverの復旧とは別の入口です。
外側で対象選択、build、migration互換性試験、新規作業停止を済ませます。
engineはSQL収集・migration審査・buildを行わず、migrationは新serverのstartupが実行します。
自動DB rollbackはありません。live環境への導入・切替には別途承認が必要です。

## 配置と正本

```text
INSTANCE/
  launch.json                 # 唯一の永続起動設定、0600
  current.json                # 原子的に置換するrelease選択（秘密なし）
  server.pid
  switch/                     # operatorが配置する固定kit、sourceにリンクしない
    launcher.py
    start.sh
    start_binary.py
    start_support.py
  releases/RELEASE_ID/         # 通常current + previous、切替中のみcandidate追加
    server
    agent-process-host
    vibe-kanban-mcp
  data/                       # DB、認証、必要設定・添付等
  runtime/                    # Workspace、Agentのcwdは各Workspace
  switch-operations/UUID/      # 永続request/snapshot/result/log、最新backup
```

`launch.json`は `{"port":4020,"environment":{"PATH":"/usr/local/bin:/usr/bin:/bin", ...}}`
という形です。operatorが既存の必要な起動環境（認証・暗号鍵・HOME等の**参照**を含む）を
明示して保持します。HOMEや認証ディレクトリをbundleへ丸ごとコピーしません。
launcherはinstance、loopback、port、絶対agenthost、bundle優先PATHを設定し、instanceをcwdにします。
起動設定へ秘密を含む場合もあります。0600で保持しログ・報告へ出さないでください。
毎回run.pyを書き換えません。停止中でも外部terminalから次で起動できます。

```bash
python3 /absolute/INSTANCE/switch/launcher.py /absolute/INSTANCE
```

`current.json`は `{"release":"/absolute/INSTANCE/releases/ID","manifest":BUNDLE_MANIFEST}`。
releaseはcopy先だけを参照し、元source、frontend、target出力へのfallbackはありません。
Agent host明示pathが欠落すると起動は失敗し、別のhostへfallbackしません。
OS runtime libraries、Python3、Git、選択provider CLI等は運用環境の互換性固定済み依存です。
provider CLIやユーザー認証をbundleへ無差別に収集しません。

## buildとbundle作成（切替engineの外側）

```bash
pnpm install --frozen-lockfile
pnpm --dir packages/local-web run build
CARGO_BUILD_JOBS=2 cargo build -p server -p local-deployment -p mcp \
  --bin server --bin agent-process-host --bin vibe-kanban-mcp \
  --features server/developer-runtime
python3 deploy/developer/switch/bundle.py \
  --server /absolute/target/debug/server \
  --agent-host /absolute/target/debug/agent-process-host \
  --mcp /absolute/target/debug/vibe-kanban-mcp \
  --output /absolute/new-bundle
```

`developer-runtime`はdebug profileでもfrontendとutilsのSoundAssets/ScriptAssetsの
RustEmbed `debug-embed`を有効にします。通常の開発時外部frontend参照と区別します。
`runtime-payload`はDB/listener/認証の初期化前に終了する検査コマンドです。
serverは実際のasset bytesが埋込みであることとhash一覧を返します。
kitは3実行ファイルと`bundle.json`だけを受け入れ、symlink、欠落、hash/権限不一致を拒否します。
manifestの`format:1`、`files`（3実行ファイルのSHA256）、`assets`（frontend/sounds/scriptsのSHA256）を
コピー済みpayloadと照合します。manifest宣言だけをembeddedの証明にはしません。
起動後はindex.htmlとJS/CSSをHTTPでhash照合します。画像等も埋込みprobeで検査しますが、
既存の予約済みMCP routeと衝突する/mcp/*画像URL等の配信は切替healthの対象にしません。
この検査は選択した実行ファイルを実行するため、外部で検証済みの信頼するbundleだけを渡してください。

## CLIとMCP

```bash
/absolute/INSTANCE/switch/start.sh \
  --bundle /absolute/new-bundle --dry-run
/absolute/INSTANCE/switch/start.sh \
  --bundle /absolute/new-bundle --request-id UUID --no-new-work
/absolute/INSTANCE/switch/start.sh \
  --status --request-id UUID
```

同期CLIとMCPは同じengineです。CLIのinstanceは配置先switch/の親から解決し、
隔離fixture等で必要な場合だけ`--instance`で明示します。CLIの`--async`は受付だけを返します。
`--dry-run`は停止・copy・candidate実行をせず、現時点のhash/idle/容量見積りを確認します。
実切替では稼働中にcandidateをrelease領域へcopyし、hash・権限・全runtime起動・embedded payloadを検査します。
その後はコピー先だけを参照します。切替lock内でidentity、listener、hash、idleを再照合して停止し、
整合backup→current原子切替→固定launcher起動→health/identity/DB検査→永続receiptへ進みます。

```json
{
  "region":"configured-development-instance",
  "request_id":"8bf3990b-9ec8-4ec9-9027-fdf035bb58d2",
  "purpose":"外部互換性試験済みbundleへの開発版切替",
  "bundle":"/absolute/new-bundle",
  "no_new_work":true
}
```

専用`self_restart`は稼働server自身のPID/start/boot/cwd、解決済instance/data、起動port、
currentと起動設定hashを非公開pipeで渡します。operation snapshotは監査記録であり恒久設定ではありません。
停止直前にworkerがlistener/identity/hashを独立に再検証します。
`LVK_SELF_RESTART_ENGINE/CONFIG`は不要です。engineは`INSTANCE/switch/start_binary.py`固定です。
対象instanceと完全一致する既存runtime grant、Project、scope、principal ownershipを維持します。
汎用`run_process`の自己再起動禁止は変わりません。権限を本kitから作成・拡大しません。

受付processの終了はpidfdで待ち、新session・独立I/Oのworkerが切替lockを保持します。
固定sleepや呼出元の生存には依存しません。新serverへlock FDは渡しません。
復帰後は`get_self_restart_operation`へ同じregion/request_idを指定します。
`get_operation`は受付監査であり完了証明ではありません。
同一ID・同一入力は既存結果、入力違いID再利用・別ID並行要求は拒否します。
現在のgrant/Project/read scopeと元principal所有を結果取得時にも確認します。
CLIでMCP所有operationを見る場合だけ、保護されたrequestのownerを`--owner`で指定します。

### 旧契約からの移行

旧CLIの`--config/--binary/--agent-host/--source/--frontend/--frontend-mode/--manifest/--sha/--retain-worktree`、
旧MCPの対応fieldは新契約では受理しません。`--instance/--bundle`（MCPはbundle）へ明示移行します。
旧`switch.json`を自動解釈せず、旧run.pyをAST書換えしません。
既存operation結果の所有・状態照会は同じ永続形式です。旧要求への新入力での再実行はしません。

初回legacy移行だけは**外部terminalのoperator作業**です。本変更は自動的にlive移行しません。
1. 新bundleを隔離検証し、固定kitと単一launch.jsonの内容をレビューします。旧起動設定・依存worktreeを保持します。
2. 新規作業を止め、既存の承認済み停止手順で旧PID/start/boot/listenerを確認して停止します。
3. SQLite backup APIでWAL込みのDBと必要data・旧起動設定を整合backupし検証します。唯一の既存backupを先に消しません。
4. instanceのrelease領域に検証済みbundleをcopyし、初期currentをそのmanifestとcopy先で作成します。
   固定kitと0600のlaunch.jsonへ必要環境を移します。旧run.py/switch.jsonは復旧資料として保持し、起動の正本には使いません。
5. 外部terminalから固定launcherで起動し、health/UI/API/Agenthost/Agent内MCPとWorkspace cwdを確認します。
6. build元を隠した隔離試験で独立性を証明した後、旧worktree保全例外をoperatorが解消します。
   稼働参照やcleanup ownershipを確認せず旧worktreeを削除・archiveしてはいけません。
以後はbundle指定だけで切替でき、scriptの編集は不要です。移行途中は旧serverへ新MCP契約を期待しません。

## 保持と失敗復旧

停止後backupの対象はdata/、launch/current、固定kitです。runtime/worktrees、target、node_modules、
共有cache、Git全体、HOMEは対象外です。data symlinkは拒否します。
SQLite backup APIでWALのcommit済データを単独DBへ取り込み、integrity/FK/manifest/fsyncを確認します。
通常releaseはcurrent+previousの2世代、candidate追加時だけ3世代です。
最新backupは1セット、新backup検証と切替成功までは旧backupを残し一時2セット分の空き容量を見積もります。
成功した切替だけが古い大容量release/backupを整理します。小さなreceipt/manifest/logは別に保持します。

`switch-operations/UUID`のrequest/selection/launch-selection/result、worker/launch PID、保護log、
settings/data/data-manifest/backup-completeを確認します。complete markerなしのbackupは完成扱いしません。
失敗時はartifact/DB/logを保持し、未解決のfailed/interrupted/未完了operationがある間は新切替を拒否します。
同じIDの再送は自動retryしません。worker消失はlock観測からinterruptedとなります。
operatorはprocess/listener、migration後DB、backupの実状態を確認し、復旧方針を決めてください。
自動SIGKILL・DB rollback・旧binaryへの復帰はしません。新serverが診断用に残る場合もあります。
復旧後、不要な失敗releaseを所有者が整理し、失敗operationを管理領域外の保護された復旧記録へ移すことで
新切替のfenceを解除できます。記録を成功へ書き換えず、診断資料を失わないでください。

resource snapshotのholders/queueは停止前に拒否します。未使用resource登録だけでは拒否しません。
この独立engineは対象instance/DBが登録資源でないことが前提です。登録対象なら既存resource operation bundleを使います。
`--no-new-work`は外部調整の確認で、他の資源使用やscheduler停止を代行する強制flagではありません。

## 検証

```bash
python3 -m unittest discover -s deploy/developer/switch -p test_switch.py -v
CARGO_BUILD_JOBS=2 cargo test -p server --features developer-runtime --lib remote_mcp::environment -- --test-threads=2
```

C/SQLite fixtureはlive DB・実認証・実モデルを使いません。production MCP routerの隔離testは
呼出元processをengineで停止して、再接続先から同じreceiptを取得します。
実bundle検証は`test_real_server.py --bundle /absolute/bundle --source /absolute/repository`。
sourceは試験用の過去schema構築にだけ使い、起動設定へ入りません。
Python SQLiteが3.42未満なら試験用環境にpysqlite3-binaryが必要です。
health/asset hash成功と実Auth0・browser UI操作・実課金Agentの検証は区別してください。

Git Skillは依頼された原文に固定同期しています。その旧「configured engine/configuration」という記述は
ここでいう固定kit/launch.jsonに対応します。新MCP fieldへSkill原文を改稿してはいません。
リリース直前のlive plugin版再照合は別工程です。
