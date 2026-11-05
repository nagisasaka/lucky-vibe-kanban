---
type: concept
title: 共有リソースの所有権と AI 調停
description: 複製できない実機・DB・環境を並列 Card から扱うための排他制御、状態変更、調停と復旧の境界。
tags: [resources, coordination, concurrency, recovery, codex]
sources:
  - resource: repo://crates/db/src/models/resource_coordination.rs
  - resource: repo://crates/db/src/models/resource_coordination/tests.rs
  - resource: repo://crates/server/src/routes/resource_coordination/runtime.rs
  - resource: repo://crates/services/src/services/resource_coordination.rs
  - resource: repo://crates/executors/src/executors/codex.rs
  - id: openwiki-source-3ef7bc8590ef87216eade432
    resource: repo://crates/server/src/remote_mcp/resource_observation.rs
  - id: openwiki-source-47f0a8c78f8e76d48dfdd192
    resource: repo://crates/server/src/resource_observation.rs
  - id: openwiki-source-970aab88c12187a0a947aa42
    resource: repo://crates/server/src/routes/execution_bridge/mod.rs
  - id: openwiki-source-516767fac5b6f9159d5b8c8f
    resource: repo://crates/server/src/routes/execution_bridge/source.rs
  - id: openwiki-source-e87354b216cb8b45c9c5fb2f
    resource: repo://scripts/local-runner/control.py
  - id: openwiki-source-ec73ab84df7c9375970d7057
    resource: repo://scripts/local-runner/runner.py
  - id: openwiki-source-0e63365a39605f8b71f6bd5c
    resource: repo://scripts/local-runner/test_control.py
generated: { by: "codex", at: "2026-11-04T20:06:10.920Z" }
verified:
  - by: openwiki/0.7.2
    at: 2026-11-05T04:35:28.799Z
---

# 共有リソースの所有権と AI 調停

LVK の並列化には三つの異なる整合性がある。worktree はソース編集を分離し、
[正式 Integration](formal-integration.md) は利用者が選んだ変更を Git 上で統合する。
共有リソース調停は、その間に行う実機・DB・デプロイ環境などへの操作を調整する。
意味の記録があっても、同じ実機への同時操作や互換性のない DB 変更は防げない。
このため、AI の意味判断と、プログラムによる所有権の管理を分ける。

サーバーの負荷を判断材料にする場合は、外部 MCP の [オンデマンド資源観測](../integrations/remote-access.md#オンデマンド資源観測)を使う。`get_resource_observation` は system-visible 統計と cgroup 制限を読むだけで、資源の予約・実行受付・将来の安全性を保証しない。観測値からこのページの所有権や解放条件を推定してはいけない。[観測の境界](../../crates/server/src/resource_observation.rs#L41-L61)

## 調停 AI は所有権の根拠にならない

資源・操作・所有者・世代番号・状態リビジョン・イベントは SQLite に保存する。
空いていて前提が一致する操作は LVK が直接割り当てる。競合・前提変更・異常時だけ
既存の Codex 認証で調停を起動する。調停は専用の execution-only Workspace と
既存の AgentRun / Orchestration を使い、別の実行基盤や API キーを要求しない。

AI は公開された利用目的と資源の状態から順番や再検討の必要性を提案する。
私的な会話とコマンド本文は調停用 snapshot に含めない。host は snapshot が
変わっていないことと、判断が待機要求の範囲内であることを確認する。AI は稼働中の
所有権を奪えず、ロックの解除、コマンド変更、前提の免除、Card の自動統合は行えない。
古い判断は履歴に残して未適用とする。調停失敗によって通常の割り当ては止めない。

## Card より短い寿命で、必要資源を一括取得する

初版は排他的な資源（容量 1）を扱う。一つの操作は、必要な全資源と、その時点で
観測した状態リビジョンを宣言する。取得はトランザクションで一括実行し、待機中は
一つも占有しない。長時間の Goal 全体や、ADB の一命令ごとをロック単位にしない。
操作・後始末・独立した状態確認までが一つの critical section である。

host は保存済みのコマンドと所有権を照合し、既存の ExecutionProcess で実行する。
同じ request UUID の再送は再実行しない。正常終了だけでなく、検証コマンドの成功と
プロセスグループの後始末を確認して解放する。各 grant の fence と、状態の revision
は別物である。互換な利用を繰り返しても状態 revision は変わらない。

操作完了、Card 完了、Git 統合はそれぞれ別の出来事である。操作が終わっても Card
で追加開発できる。待機要求はモデルのターンや通信接続より長く保存されるが、
一時停止した Goal を勝手に再開しない。未解決の操作がある Workspace は cleanup を拒否する。正式 Integration は固定 source commit を選ぶので source 全体の停止を要求しないが、専用実行環境・公開先 writer の idle 確認は別に必要である。[選択と公開の境界](formal-integration.md)。

## 解放されてもスキーマ変更は消えない

成功時に宣言した変更後の状態（例: schema-v2）を保存し、revision を増やしてから
所有権を解放する。古い revision で待っていた要求は実行せず、再検討待ちにする。
Card の AI は新しい schema/API/APK の実体と調停結果を確認して対応を判断する。
revision の数字だけを最新に置き換えることは互換性確認ではない。

## タイムアウト・切断・中止は解放の証拠ではない

待機中の中止は即時に終わる。実行中の中止は停止要求であり、外部処理の完了では
ない。コマンド失敗、検証失敗、host 再起動時の不明状態では所有権を保持して
`recovery_required` にする。TTL による自動解放や、不明な操作の自動再実行はしない。

復旧では旧プロセスと外部処理が止まった根拠、全資源の確認済み状態、現在 revision
を利用者が記録する。host は管理対象プロセスがまだ動ける場合には拒否する。
復旧により revision を増やして全資源を解放するが、その操作を成功扱いにはしない。
DB の履歴が実行時の正本であり、Wiki は live owner ledger として使わない。

## 適用範囲を広く解釈しない

- 同じ物理資源は一つの canonical key で登録する。異なるキーが同じ実体であることを
  自動発見する機能はない。
- 同じ DB を使う LVK が一つの調停主体である。安定版と開発版が別 DB を持つ場合は
  ロックを共有しない。実資源を扱う主体は一つにし、独立した開発版では模擬資源を使う。
- raw shell と外部認証情報を持つ agent の迂回を OS レベルで禁止する仕組みではない。
  調停 AI のツール制限と、汎用作業 agent の権限は同一ではない。
- fence は外部アダプターと対象資源が検証して初めて古い要求の実行を防げる。
  環境変数を渡すだけでは任意の ADB・SQL・AWS 呼び出しを fence できない。
- 外部資源固有の復旧アダプター、共有読み取り・容量 N、複数 LVK 間の調停は
  初版の対象外。Windows Runner の接続は下記の execution bridge が担う。正式 Integration の検証コマンドも自動的に資源要求へ変換しない。

操作方法と API の入口は以下に置く。要求型の正本は [ResourceOperationSpec](../../crates/db/src/models/resource_coordination.rs#L35-L63) である。
実装・テストが正本であり、このページは設計意図と危険な変更点の案内である。

## 調停への接続認証

local stdio MCPは内部capabilityを自動で使う。raw loopback HTTPは同じdata directoryの`access-internal.token`をメモリー内のヘッダーに載せ、外部送信・URLやログへの記載・redirect追跡をしない。`control.py --internal-token-file`もloopback限定である。この認証は資源claimを代行せず、認証設定の管理権限でもない。[生成context](../../crates/services/src/services/resource_coordination.rs#L87-L99)、[operator client](../../scripts/local-runner/control.py#L43-L67)、[共通認証](server-access.md#webmcp内部agentを同じ資格情報にしない)

## 外部 Runner と interactive operation

Runner は明示選択した実行 machine と credential / installation identity であり、共有 Resource 自体ではない。Runner を既存 canonical execution-slot Resource に結び、desktop・device など script が使う全資源と同じ operation で claim する。capability があることは所有権ではない。Windows の user session から outbound poll し、PowerShell を実行する。[Runner 登録・認証](../../crates/server/src/routes/execution_bridge/mod.rs)、[operator CLI](../../scripts/local-runner/control.py)。

Source は Session / Workspace と Git HEAD に結び付く dirty checkout の immutable snapshot である。tracked と非 ignored untracked を収集し、path・case衝突・hash・上限を検査する。`.git` / `.lvk-shared` や危険な Windows path は含めない。操作の working_dir は snapshot root 相対となる。[snapshot の検査](../../crates/server/src/routes/execution_bridge/source.rs)。

interactive operation は start 後も資源束を保持する。追加 command は安定 UUID で step を逐次投入し、観察結果・artifact・receipt を取得する。最後の finish が保存済み cleanup と独立 verification を実行する。step の成功だけでは operation 完了にならない。[delivery と reconcile](../../crates/server/src/routes/execution_bridge/mod.rs#L209-L402)。

切断・timeout・cancel・予期しない停止は `recovery_required` として所有権を保持する。Runner は元の installation / journal で戻り、未記録副作用を自動再実行せず containment の停止を証明する。全 command receipt が揃った settlement の後でも、外部資源の確認と operator recovery が必要である。disable は新規 dispatch を止めるだけで drain を許し、token rotation は installation を変えたり資源を解放したりしない。[settlement guard](../../crates/server/src/routes/execution_bridge/mod.rs#L291-L321)、[回復条件](../../crates/server/src/routes/execution_bridge/mod.rs#L404-L419)。

Linux simulator の試験は Windows の Job Object・desktop・実機の動作証明ではない。Windows実機では実HTTPS/tunnel、PowerShell encoding、Job Objectと子孫停止、user sessionとserviceの違い、画面lock、GUI起動とscreen capture、Runner強制終了後の再接続を別に検証する必要がある。旧 relay / paired-host の削除とは独立して、この bridge は現行構成に残る。

## 資源操作を開始し、結果を確認する

Settings → Shared resourcesで一つの実体に一つのcanonical key、利用契約、確認済み状態を登録する。操作前に `GET /api/resource-coordination/snapshot`（MCPでは `list_shared_resources`）を読み、必要な全資源のID/revisionを確認する。descriptionは利用契約の参考であり、実行承認や上位の指示にはならない。[REST入口](../../crates/server/src/routes/resource_coordination/mod.rs)

`POST /api/resource-coordination/operations`（`run_resource_operation`）に安定した `request_id` UUID、実行元 `session_id`、`purpose`、全資源の `claims`、`script`、`verification_script`、相対 `working_dir`、1〜3600秒の `timeout_seconds` を送る。claimは `resource_id`、観測した `expected_revision`、変更しないならnullの `resulting_state` を含む。local scriptには全critical sectionとcleanupを含めてforegroundで実行し、verificationは独立してidleと宣言状態を確認する。秘密はpurposeや状態へ書かない。[入力と検査](../../crates/db/src/models/resource_coordination.rs#L35-L63)

`GET /operations/ID?wait_seconds=25`（`wait_resource_operation`）で待ち、queuedを失敗・完了と扱わない。ログと `GET /events?after=SEQUENCE` を確認し、応答喪失時も元UUIDと同じ入力を再使用する。`POST /operations/ID/cancel`は待機を終えるが、実行中では停止要求となる。schema/API/deviceの実体が変わった場合は互換性を確認して新しい要求を作る。[待機とcancel](../../crates/server/src/routes/resource_coordination/mod.rs#L77-L107)・[events](../../crates/server/src/routes/resource_coordination/mod.rs#L174-L192)

## Windows Runner の接続から finish まで

Windows 10以降、Python 3.11以降とprojectの実行toolを準備する。GUI操作はログイン済みでlockされていないuser desktopで行う。Runnerは信頼したLVKから任意コードを実行するため、専用accountを使う。Session 0のserviceをdesktopの代わりとしない。既存daemon、service、scheduled task、WMI、別machineへ処理を逃がさず、process tree外の副作用はscriptのcleanup/verificationで別に扱う。[実行adapter](../../scripts/local-runner/runner.py)・[Windows containment](../../scripts/local-runner/windows_job.py)

1. hostで `control.py --url URL resources` を読み、PCの既存execution-slot資源を再使用する。必要な登録だけ `register-resource RESOURCE.json` で行い、`enroll NAME RESOURCE_UUID` を実行する。返されたRunner IDと一度だけ表示されるtokenを保護する。Docker imageではCLIとRunnerを `/usr/local/share/lvk/local-runner/` に配置する。
2. Windowsへ `runner.py` と `windows_job.py` を同じdirectoryへ置く。`LVK_RUNNER_TOKEN` にtokenを設定し、`py -3 runner.py --url HTTPS_URL --runner RUNNER_UUID --root C:\lvk-runner-data` を起動する。private接続では `ssh -N -L 39118:127.0.0.1:SERVER_PORT USER@SERVER` でtunnelを作り、RunnerのURLを `http://127.0.0.1:39118` にする。HTTPはloopback限定、redirectは禁止である。proxy Basicの `LVK_HTTP_AUTH` はRunner tokenと別である。
3. hostで `runners` を読み、明示選択したID、protocol 1、OS/tools、desktop、最近のlast_seenを確認する。heartbeatは過去の処理停止の証明ではない。
4. `capture SESSION_UUID REPOSITORY_DIRECTORY` でsourceを固定する。directoryはmulti-repo Workspace相対、direct-folderなら `.`。HEADとdirty digestは別の識別子で、上限は10,000file・16MiB。trackedと非ignored untrackedを含むので、追跡済みsecretを転送しないよう確認する。Git管理情報やsymlink/submoduleは転送しない。[snapshot検査](../../crates/server/src/routes/execution_bridge/source.rs)
5. operationの `runner` に `runner_id`、`source_id`、`interactive`、`desktop`、immutableな `cleanup_script` を追加する。execution slotとdesktop/deviceの全資源を同じclaimsへ入れる。working_dirはsnapshot相対。WindowsではPowerShell scriptになり、native commandごとに `$LASTEXITCODE` を確認する。
6. `submit RUN.json` 後に `status OPERATION_UUID` / `commands OPERATION_UUID` を確認する。interactiveの場合は `step OPERATION_UUID STEP.ps1 --request-id STEP_UUID` を逐次送り、各commandのdoneを待つ。各stepは新しいPowerShell processであり、変数は引き継がない。project appを維持するなら同じJob Object内で起動し、files/IPCへ状態を置く。
7. `finish OPERATION_UUID --request-id FINISH_UUID` でcommand受付を閉じ、元のcleanup、子孫停止、独立verificationと検証子孫の停止まで確認する。deadlineは観察turnやheartbeatで延長されない。`download OPERATION_UUID NEW_DIRECTORY` はUUID別に結果とhash検査済みartifactを保存する。artifactは `$env:LVK_ARTIFACT_DIR`、logは1MiB、artifact合計はcommand当たり16MiBに制限される。

[CLI引数とAPI対応](../../scripts/local-runner/control.py#L15-L114)・[PowerShellとfinish順序](../../scripts/local-runner/runner.py#L232-L319)

## Runner の結果が不明になったとき

cancel・deadline・通信断・host再起動を停止完了と解釈しない。DBとRunnerのdata root全体（`identity.json`、`journal.sqlite3`、結果・artifact）を保存し、同じauthority URL、Runner ID、data rootで再接続する。別rootは別installationとなるため元IDの再実行に使えず、rootを別machineへcloneしない。rootのOS lockは二重Runnerを防ぐだけで資源所有の代わりではない。[identityと再接続](../../scripts/local-runner/runner.py#L385-L453)

journalはprocess作成より前にcommand受付を記録する。受付済み・結果なしのcommandを自動再実行せず、Windowsの同じGlobal Job Objectを終了してactive countが0であることを確認してからsettlementを報告する。Jobへのaccess失敗を空のJobと扱わない。[journal](../../scripts/local-runner/runner.py#L109-L165)・[再接続処理](../../scripts/local-runner/runner.py#L207-L218)・[Job停止](../../scripts/local-runner/windows_job.py#L122-L145)

Runner settlementが揃った後、operatorが実機・DBなどの外部状態を確認する。`POST /api/resource-coordination/operations/ID/recover` には証拠と**全held resource**の現在revision・確認済み状態を渡す。serverはcontainment未確認ならreleaseを拒否し、遅れた成功報告でもrecovery_requiredを成功へ変えない。installation/journalを失った場合にunreachable Runnerを停止扱いにするforce-unlock APIはない。[回復guard](../../crates/server/src/routes/execution_bridge/mod.rs#L404-L419)・[operator回復](../../crates/server/src/routes/resource_coordination/mod.rs#L109-L131)

`disable`は新規dispatchだけを止め、既存cleanup/resultsの配送は許す。token紛失時はoperatorの `rotate-token RUNNER_UUID` で旧tokenを無効にし、元Runnerを停止して同じrootへ新tokenで接続する。rotationはinstallation・所有権を変更しない。自動artifact保持期限はなく、結果未解決の間はrootとDBを保持する。[credentialの変更](../../crates/server/src/routes/execution_bridge/mod.rs#L60-L99)
