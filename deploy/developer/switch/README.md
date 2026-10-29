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
    terminal_drain.py
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
CARGO_BUILD_JOBS=2 node scripts/dev-cargo.mjs --purpose dev build -p server -p local-deployment -p mcp \
  --bin server --bin agent-process-host --bin vibe-kanban-mcp \
  --features server/developer-runtime
python3 deploy/developer/switch/bundle.py \
  --server /absolute/checkout/target/release/server \
  --agent-host /absolute/checkout/target/release/agent-process-host \
  --mcp /absolute/checkout/target/release/vibe-kanban-mcp \
  --output /absolute/new-bundle
```

常用devはrelease最適化＋line-tables-only、strip=none / split-debuginfo=offで行情報を実行ファイルに保持します。bundleはdebug sidecarを必要としません。previewは専用instance入口から未最適化devを使います。

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
  "project_id":"11111111-2222-4333-8444-555555555555",
  "request_id":"8bf3990b-9ec8-4ec9-9027-fdf035bb58d2",
  "purpose":"外部互換性試験済みbundleへの開発版切替",
  "bundle":"/absolute/new-bundle",
  "no_new_work":true
}
```

専用`self_restart`は稼働server自身のPID/start/boot/cwd、解決済instance/data、起動port、
currentと起動設定hashを非公開pipeで渡します。operation snapshotは監査記録であり恒久設定ではありません。
停止直前にworkerがlistener/identity/hashを独立に再検証します。
terminal drainは1回だけ実施し、その後の初回preflightが子孫・instance利用者・共有resource占有でbusyの場合だけ、30秒待って1回再確認します。
identity/hash/config/権限等の拒否、停止直前の新しいbusy、SIGTERM以降の失敗は再試行しません。待機と理由は永続receiptのstageとbusy-recheck.jsonへ記録します。
stdioの`self_restart`も同じ認証付き専用MCP経路へ委任し、永続受付を返します。受付後は担当Codex Runを終了し、再接続後に同じproject_id/request_idの`get_self_restart_operation`で確認してください。受付は切替成功を意味しません。
`LVK_SELF_RESTART_ENGINE/CONFIG`は不要です。engineは`INSTANCE/switch/start_binary.py`固定です。
プログラム実行設定のON、Project認可、runtime read/execute scope、principal ownershipを確認します。
region/grant形式は廃止され、読書rootは自己再起動の許可条件には使いません。
対象は実行中の開発instanceとその固定kit/configurationに限定します。
汎用`run_process`の自己再起動禁止は変わりません。権限を本kitから作成・拡大しません。

実切替の開始時に、対象serverが所有する内蔵PTYとそのcommandを一度だけ終了し、
その後に従来のidle/identity/listener/data-FD/resource guardを実行します。
`--no-new-work` の外部調整には、対象DEVへのTerminal接続停止（再接続するUIを閉じる等）も含めます。
終了後にPTYが再生成された場合は既存child guardで安全拒否します。成功するまで繰り返しkillしません。
Rust変更・新しい受付lock・追加protocol・一般UI変更は不要で、既存binaryでも初回から利用できます。
通常利用者の安定版image更新にこの運用は追加しません。

終了対象は、対象serverの直接の子、同UID、session/group leader、serverが保持するPTY masterと
対応するcontrolling TTY・stdin、`VIBE_KANBAN_TERMINAL=1` と同一 `LVK_INSTANCE_DIR` が
揃う内蔵PTYと、その境界内の子孫だけです。cwdやshell名で判定しません。
PID/startと親・session・group・TTY・UIDをpidfd取得後も照合し、別sessionへのdetachや所属変化は拒否します。
commandへTERMを送り終了確認後、通常TERMを無視する対話shellへHUPを送ります。
管理PTY内の実行中commandも終了対象です。Terminalをguardから除外しません。
旧serverはshellを明示的にwaitしないためzombieが残る場合があります。pidfdでexitを確認し、
実行・FDを持たないzombieは既存guardの既存規則に従います。全子孫のwait回収を追加保証するものではありません。

TERM・logoutのtimeoutではKILLへ進まず、残ったprocessと失敗記録を診断用に保持します。
`terminal-drain.json` にはsignal前に確認したprocessの所属・start等を保存します。
失敗fenceとDB/log保全は従来どおりです。`--dry-run` はPTY停止を行わず、
PTYがある場合は従来のchild guardで非破壊に拒否します。

初回も、外部調整と対象同定後に新しい固定kitを配置し、外部terminalから新kitを使うか、
旧serverの既存 `self_restart` から同じ固定kitを呼び出せます。新binaryの先行起動は不要です。
過去のfailed operationは以下の復旧手順で記録を保全してfenceを解消してから、新しいrequest IDを使います。
稼働binaryのSHAと固定kit各scriptのSHAは別々に記録します。kitの更新をbinary更新成功と扱いません。

受付processの終了はpidfdで待ち、新session・独立I/Oのworkerが切替lockを保持します。
固定sleepや呼出元の生存には依存しません。新serverへlock FDは渡しません。
復帰後は`get_self_restart_operation`へ同じproject_id/request_idを指定します。
`get_operation`は受付監査であり完了証明ではありません。
同一ID・同一入力は既存結果、入力違いID再利用・別ID並行要求は拒否します。
現在の実行設定/Project/read scopeと元principal所有を結果取得時にも確認します。
CLIでMCP所有operationを見る場合だけ、保護されたrequestのownerを`--owner`で指定します。

### 初回導入とstdio委任

bundleは3binaryとmanifestだけで、固定`switch/` kitは自動更新しません。
30秒再確認とstdio直接受付を使うには、レビュー・main統合・新bundleの隔離検証後に、
別途承認されたoperator作業で、対象instanceと進行中operationがないことを確認してください。
現kitを復旧用に保持し、同じrevisionの`launcher.py`、`start_binary.py`、`start_support.py`、
`terminal_drain.py`、`start.sh`を整合して配置します。実行中workerのkitは置換せず、
source symlinkで代用しません。launch/current/dataや既存認証情報を上書き・コピーする手順ではありません。

261のstdioにはself_restartがありません。新しいstdio binaryを明示するMCP起動設定で
Codex側stdio processを起動し直す必要がありますが、LVK serverの先行再起動は不要です。
instanceは既存の`LVK_INSTANCE_DIR`、接続先は`MCP_HOST`/`MCP_PORT`（既存port discoveryも可）を使い、
設定済みloopback/portとkitのlaunch設定が一致する必要があります。tool引数でinstance/engine/ownerは変更できません。
既存stdio同様、これは同一OSユーザーの信頼するローカル実行です。OAuth principalやBearer scopeを
持つと偽装せず、OAuth専用/mcpを迂回するHTTP adapterも追加しません。
stdioは共通engineの正式CLIへ`--async`で委任し、永続受付だけを返します。
engineは既存runtime設定の`execution_enabled`、projectの存在、Workspace contextがある場合は
そのProjectリンクを既存DBから読取確認します。contextにprojectがあればstdio側でも一致を確認します。
global stdioでcontextがない場合は、既存global discovery同様に明示projectを使います。
operator CLIの既存権限モデルは変更せず、remoteは従来のOAuth scope・project許可と設定確認を維持します。
stdio結果のownerはOS UIDとprojectから決まるinstance内namespaceです。remoteのissuer/subject由来ownerとは
区別し、別channelのreceiptを自分の成功として読んだり、そのrequest IDを流用したりしません。

初回も、旧261serverを動かしたまま新kitと新stdioを正規導入した後、Codexへ明示承認済みbundleの
`self_restart`を依頼できます。上位AIの新remote MCPによる再起動を前提にしません。
受付後は担当Codex Runを終了して子孫/writerを収束させ、復帰後に同じproject/request IDの
`get_self_restart_operation`を使います。stdio processだけを終了しても担当Runやhostが残ればbusyは解消しません。
30秒後もbusyなら未切替で停止します。設定失効、unknown結果、権限拒否を成功と扱わず、自動再送もしません。
Codexの既存MCP設定では`mcp_servers.vibe_kanban.command`を、保持した検証済み新bundleの
`vibe-kanban-mcp`絶対pathにします。旧serverが設定したPATHは旧releaseを先頭にするので、
単にcommandをbasenameのままにして新bundleを置くだけでは切り替わりません。
既存の同一UID・instance/loopback/portを使い、認証情報やHOMEを別場所へコピーしません。
新しい通常Codex Runでstdioを再生成し、まずtools/listでself_restartと結果取得toolの露出を確認します。
新stdioのMCPクライアントへの配置・設定再読込/stdio process再生成と、LVK serverの再起動は別操作です。
この導入自体にも別途承認が必要で、source変更やbundle buildだけで稼働環境へ反映されたとは扱いません。

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
python3 -m unittest discover -s deploy/developer/switch -p test_terminal_drain.py -v
python3 -m unittest discover -s deploy/developer/switch -p test_busy_recheck.py -v
CARGO_BUILD_JOBS=4 node scripts/dev-cargo.mjs test -p server --features developer-runtime --lib remote_mcp::environment
LVK_TEST_MCP_BINARY=/absolute/checkout/target/debug/vibe-kanban-mcp python3 -m unittest discover -s deploy/developer/switch -p test_stdio_restart.py -v
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
