# 開発版の成果物切替

`start.sh` は、外部でビルド・互換性試験を済ませた成果物へ、稼働中の開発instanceを切り替えます。Git操作、ビルド、試験証拠の審査、migration SQLの収集・解釈・実行、履歴修正は行いません。migrationは新serverの通常の起動機構が実行します。事前のコード・E2E・DB互換性確認、作業停止の調整、切替判断は呼出し側の責任です。

このディレクトリは置換用kitです。リポジトリへ追加しても稼働環境へはインストールされません。既存の開発版入口・stable・コンテナー・updater・公開ネットワーク・認証権限は変更しません。最終インストールと実行はoperatorが行います。

## インスタンス設定と導入

Linux、Python 3.11以降、pidfd対応kernel、同一OSユーザーを前提とします。対象は明示したinstanceのloopback listenerです。初回起動・supervisor管理・停止済みinstanceの復旧には既存のoperator手順を使ってください。

1. 既存の`start.sh`、`start_binary.py`、`start_support.py`、途中配置された`start_migrations.py`と試験記録を、owner限定の回復ディレクトリへ保存します。原本は削除しません。
2. このkitの`start.sh`、`start_binary.py`、`start_support.py`を、worktree cleanup対象外の保持ディレクトリへコピーします。旧`start_migrations.py`は新engineから参照されません。
3. instance直下へowner限定の`switch.json`を配置します。例の値は対象環境へ置き換えます。

```json
{"instance":"/absolute/instances/development","port":4020}
```

既存の`run.py`を起動設定の正本として保持します。対応する形式は、`source = Path(...)`、`env.update({...})`内の`VIBE_KANBAN_AGENT_PROCESS_HOST`、`os.execve(binary, argv, env)`が各一つの既存形式です。`env`の既存設定を維持し、切替対象の参照だけをASTで更新します。対応しない形式は停止前に拒否します。設定ファイルを候補成果物として実行して検査することはありません。

`DISABLE_WORKTREE_CLEANUP='1'`を既存の`env.update`と稼働process環境で確認します。引用符の種類には依存しません。instanceの`builds/`に保持したsource snapshotを推奨します。それ以外のsourceは`--retain-worktree`が必要です。instanceのcleanup管理下にあるworktreeはDBの登録も確認します。元のsource/frontend/runtime依存は、稼働・復旧に必要な間、外部のcleanupや再ビルドでも削除・上書きしないでください。全worktreeのコピーは不要です。

## 同期CLI

```bash
/path/to/installed/switch/start.sh \
  --config /absolute/instances/development/switch.json \
  --binary /absolute/build-A/server \
  --agent-host /absolute/build-A/agent-process-host \
  --source /absolute/retained-source-A \
  --frontend /absolute/retained-source-A/packages/local-web/dist \
  --retain-worktree --no-new-work \
  --request-id 8bf3990b-9ec8-4ec9-9027-fdf035bb58d2
```

別の成果物はパスと新しいrequest UUIDを指定します。スクリプト編集、SHA一致、特定Issue/Runへの例外は不要です。同期実行も非同期実行も同じ`execute()`を使います。TTYの有無を権限の代用にせず、明示入力・対象identity・排他・idle確認を必須にします。稼働serverの子として同期実行すると子孫検査で停止前に拒否されます。自己再起動には専用MCP経路を使います。

同じ引数に`--dry-run`を加えると、停止・backup・起動をせず、選択物・現process・backup範囲と見込み容量を表示します。現在の物理使用状況の確認も含みます。観測だけなので`--no-new-work`は不要です。実切替には引き続き事前調整と同flagが必須です。結果や引数に秘密値を入れないでください。

`--source`省略時は従来の`target/debug/server`配置を推定し、hostはbinaryと同じディレクトリ、frontendはsourceの`packages/local-web/dist`を既定にします。異なる配置では全パスを明示してください。

## 成果物のidentity

実binary/agent hostのSHA-256、frontend全ファイルのSHA-256 tree、起動後の`/proc/PID/exe`、cwd、listener、配信された`index.html`、`/api/info`の取得可能なbuild情報を記録・照合します。checkout HEADからbinaryの出所は推定しません。

既定の`--frontend-mode external`はdebug binary内のfrontend絶対パスを検査します。この文字列検査だけで同一buildを証明するものではなく、外部のbuild/互換性試験が前提です。frontend内蔵binaryは`--frontend-mode embedded --manifest /path/build-manifest.json`を使います。manifestは次の形式で、外部のbuild担当が実成果物に結び付けて作成します。`instance.mjs`の従来の`build.json`はhashを持たないため、そのままではこのmanifestの代用になりません。

```json
{
  "artifacts": [
    {"path":"/absolute/build/server","sha256":"..."},
    {"path":"/absolute/build/agent-process-host","sha256":"..."}
  ],
  "frontend": {
    "path":"/absolute/source/packages/local-web/dist",
    "tree_sha256":"...",
    "files":{"index.html":"...","assets/example.js":"..."}
  }
}
```

Tree hashは相対パスの昇順で`SHA256 + "  " + relative_path + "\n"`を連結したUTF-8のSHA-256です。`source_sha`は任意で、指定すると起動後build revisionも照合します。`--sha`はmanifestのrevisionに対する任意の追加制約です。内蔵assetsとmanifestの対応自体をengineが再ビルドして証明することはありません。

新旧binary/hostとfrontendの復旧用コピーをoperationに保存します。新binary/hostはこの保存先から起動するため、元のCargo出力を再ビルドしても稼働binaryは上書きされません。debug frontendの絶対参照先とsource/runtime依存は元の保持先を使い続けます。

## 非同期とMCP

CLIに`--async`を加えると、永続`operation_id`と`result_path`を返します。独立workerが切替全体を担当し、新session・独立I/Oで起動して排他FDを引き継ぎます。呼出し元の終了やMCP切断でworkerをcleanupしません。新serverへは排他FDを渡しません。

```bash
/path/to/installed/switch/start.sh \
  --config /absolute/instances/development/switch.json \
  --status --request-id 8bf3990b-9ec8-4ec9-9027-fdf035bb58d2
```

MCPを有効にするには、operatorがserver起動環境に`LVK_SELF_RESTART_ENGINE`（保持した`start_binary.py`の絶対パス）、`LVK_SELF_RESTART_CONFIG`（instance直下の`switch.json`）を設定し、対象instanceと完全一致する既存runtime grantを用意します。既存のruntime設定・Project許可・scopeを再利用し、ここから権限は作成しません。**最初の導入は、専用toolを含むserverへのoperatorによる同期切替が必要です。** 旧serverの`run_process`で制約を迂回して自己再起動しないでください。

`self_restart`の引数例:

```json
{
  "region":"configured-development-instance",
  "request_id":"8bf3990b-9ec8-4ec9-9027-fdf035bb58d2",
  "purpose":"外部互換性試験済み成果物への開発版切替",
  "binary":"/absolute/build/server",
  "agent_host":"/absolute/build/agent-process-host",
  "source":"/absolute/retained-source",
  "frontend":"/absolute/retained-source/packages/local-web/dist",
  "retain_worktree":true,
  "no_new_work":true
}
```

復帰後は`get_self_restart_operation`へ同じ`region`と`request_id`を渡します。現在のgrant/Project/read scopeと、元のprincipal・grantの所有を再確認します。MCPの`get_operation`は**受付監査**であり、切替成功ではありません。旧processのreceiptが`uncertain`でも、専用照会はengineの永続結果を参照できます。MCP所有の結果をCLIで見る場合は、保護された`request.json`のownerを`--owner`に指定します。ownerは秘密値ではありませんが、結果を公開しないでください。

同一ID・同一入力の再送は既存operationへ対応します。入力違いは拒否し、別IDの並行要求も排他で拒否します。workerが消失し排他が解放された未完了operationは`interrupted`として記録し、自動再実行しません。受付用一時ディレクトリの原子的な公開前には切替を開始しません。

## 競合・backup・失敗

既存resource coordinationのsnapshotを確認し、現在のholdersまたはqueueがある場合は明示拒否します。未使用resourceの登録だけでは拒否しません。登録は使用中を意味せず、この観測も資源の取得・使用許可ではありません。呼出し側は各資源のusage契約と既存調停を守り、切替中に新規使用を始めないよう調整します。通常のmanaged resource operationからdetachする迂回はしません。この独立engineは対象instance／DB自体が登録資源ではないことが導入・呼出しの前提です。対象が登録済みなら空き状態でも本経路で切り替えず、既存のoperation bundle契約を用います。engineは自由記述のresource説明から対象identityを推測しません。使用中資源の所有を再起動へ引き継ぐhandoffは今回の範囲外です。

AI／人間は、agent/workflow/Integrationの業務上の作業可否、配信待ち・論理予約・schedule、再起動による中断の扱いを事前確認し、新規作業を止める調整を行います。`--no-new-work`はその調整の確認であり、単なる強制実行flagや汎用scheduler停止機能ではありません。engineは全履歴のterminal判定や過去stale記録の修復を要求しません。特定Run ID例外も設けません。代わりに、現在のserver子孫、対象instance環境を持つ他process、dataを開いている他processを拒否し、pidfdによる停止直前と停止後にも物理使用を再確認します。DBのrunning表示だけで現processの存在を断定せず、表示がterminalでも実在するprocessを見逃しません。MCPのscope・Project・grant・所有者認可は変更しません。

停止には、起動時刻・boot・exe・cwd・instance・portを照合したpidfdを使います。TERMで停止を確認できなければ次へ進まず、自動SIGKILLしません。停止後のbackupは`data/`と起動設定・helperだけです。`db.v2.sqlite`はSQLite backup APIでWALの確定済み内容を含む単独DBとして保存し、integrity/FKとmanifestを確認します。worktree、Git repository、target、node_modules、共有cache全体は含めません。任意のdata symlinkは拒否します。

結果は`instance/switch-operations/UUID/`に保存されます。

- `result.json`: status、最後のstage、safeな失敗理由、health/identity/DB確認の結果。
- `request.json`、`selection.json`、`launch-selection.json`: 要求と実際の成果物identity。
- `settings/`、`data/`、`data-manifest.json`、`backup-complete.json`: 検証済みbackup。complete markerのないbackupを復旧用完成品と扱わないでください。
- `artifacts/`: 保持した新旧実行ファイルとfrontend。
- `worker.json`、`launch.json`、`worker.log`、`server.log`: worker/起動の確認情報とowner限定ログ。

`accepted`/`running`は成功ではありません。`failed`/`interrupted`ではstage、process/listenerの実状態、backup marker、保護ログを確認してください。起動後に失敗したserverは診断のため残る場合があります。自動DB rollback、旧binaryへの自動復帰、失敗データ削除は行いません。migration後のDBに旧binaryを向けられるとは仮定しないでください。旧成果物への復旧にも対応するDB・設定と外部判断が必要です。

engineは認証・暗号鍵を結果やstdoutへ展開せず、旧process環境をメモリ内で新processへ引き継ぎます。任意server自身のログまで秘密非出力を保証するものではないため、`server.log`も公開せず扱ってください。health成功はAuth0/UI/MCPの機能確認と別です。

## 隔離検証

```bash
python3 -m unittest discover -s deploy/developer/switch -p test_switch.py -v
CARGO_BUILD_JOBS=2 cargo test -p server --lib remote_mcp::environment -- --test-threads=2
```

Python試験にはC compilerとSQLite runtime libraryが必要です。fixtureだけを一時領域で構築し、2組の成果物、server起動時migration、WAL、PID違い、競合、backup/起動/health失敗、呼出し元/worker終了、重複要求を検証します。MCP試験はproduction HTTPルーターと実engineを使い、engineが呼出し元MCP fixtureを停止してから別成果物を起動し、再接続したMCP hostから同じ結果を取得します。mock IdPを使うため実Auth0の証明ではありません。

既存LVKバイナリの通常migration機構も別途隔離検証できます。

```bash
python3 deploy/developer/switch/test_real_server.py \
  --binary /absolute/tested/server --agent-host /absolute/tested/agent-process-host \
  --source /absolute/retained-source --frontend /absolute/retained-source/packages/local-web/dist
```

この試験だけが、外部test setupとして空の過去schemaを構築します。PythonのSQLiteが3.42未満なら試験用venvに`pysqlite3-binary`を用意します。engineにはこの依存・migration処理を持ち込みません。試験はlive DBをコピーせず、終了時にfixtureのprocessと一時データを片付けます。
