---
type: concept
title: 実行環境へのアクセス
description: 外部 HTTP MCP の Linux runtime grant、管理設定、ファイル・プロセス観測と直接実行の境界、取消・再起動・不確実な結果を説明する。
tags: [runtime, mcp, authorization, processes, lifecycle]
sources:
  - id: openwiki-source-bcdf7692387d928a66457e55
    resource: repo://crates/db/migrations/20261029010000_runtime_access_redesign.sql
  - id: openwiki-source-8ab39f439cdb152ec09eb06c
    resource: repo://crates/server/src/remote_mcp/environment_files.rs
  - id: openwiki-source-076036ed7bdac82d628d39d1
    resource: repo://crates/server/src/remote_mcp/environment_process.rs
  - id: openwiki-source-6cb471b32e19ce921b78d86e
    resource: repo://crates/server/src/remote_mcp/environment_settings.rs
  - id: openwiki-source-2260918d1f9b90281f9755f9
    resource: repo://crates/server/src/remote_mcp/environment_switch.rs
  - id: openwiki-source-0801a7e5bbcc064c12effc80
    resource: repo://crates/server/src/remote_mcp/environment.rs
  - id: openwiki-source-9e15762bf7a432b7605f8466
    resource: repo://crates/server/src/remote_mcp/issue_write.rs
  - id: openwiki-source-6adda10f98b079272f82a4cc
    resource: repo://crates/server/src/remote_mcp/operations.rs
  - id: openwiki-source-605a3efdc11efae80ada92b1
    resource: repo://crates/utils/src/mcp_error.rs
  - id: openwiki-source-417b2b5ecb26c89c1b06df88
    resource: repo://deploy/developer/switch/README.md
  - id: openwiki-source-9e0cfce0846bb98d2dc47de0
    resource: repo://deploy/developer/switch/start_binary.py
  - id: openwiki-source-688a3a970a72f7fa2b5dbc1c
    resource: repo://packages/web-core/src/shared/dialogs/settings/settings/RuntimeAccessSettingsSection.tsx
generated: { by: "codex", at: "2026-11-04T20:06:10.920Z" }
verified:
  - by: openwiki/0.7.2
    at: 2026-11-07T09:17:18.907Z
---

# 実行環境へのアクセス

Runtime Accessは認証された外部MCP clientがLVKのOS user権限でファイルを扱い、既存プログラムを直接起動・観測・停止する機能である。AgentRunを作らず、OS sandboxでもない。内容・出力は参照データであり承認ではない。[runtime API の契約](../../crates/server/src/remote_mcp/environment.rs#L119-L183)・[process所有](../../crates/server/src/remote_mcp/environment_process.rs)

## Grant と権限の重なり

現行はregion/grant方式を廃止し、`files_enabled`と`execution_enabled`の独立した設定を既定ONにする。direct file rootsは既定 `/`、1〜64個の絶対directoryで、相対path・`..`・空を拒否し、保存時にcanonicalizeする。file rootsは直接file APIだけに適用し、executionのcwdやprogramのアクセス先を閉じ込めない。executable allowlist、region ID、Project別root設定、legacy JSON fallbackはない。[設定型とtool定義](../../crates/server/src/remote_mcp/environment.rs)

全toolは `lvk:read` と `lvk:runtime:read` を要求する。直接変更は追加 `lvk:write`、起動・停止・self_restartは追加 `lvk:runtime:execute`。能力参照以外は明示project_idと利用可能なProjectを照合する。Projectは認可・audit文脈でありfilesystem tenant境界ではない。共有デバイス・DB・配置は別の [資源調停](shared-resource-coordination.md)と明示許可に従う。

## 管理設定の保存・適用・取消

browser管理者のSettings → MCP連携で設定する。`GET/PUT /api/runtime-access-settings`はweb/basic/localのadmin専用で、保存時に非local actorのactive adminとexpected revisionを再確認し、SQLite設定とauditを同じBEGIN IMMEDIATEで確定する。MCPに設定writerを公開しない。保存でprocess registryを再生成しない。[設定API](../../crates/server/src/remote_mcp/environment_settings.rs)

新形式の保存rowが正本で、未保存時は既定設定を返す。破損row/DB障害は失敗させ、 permissiveなdefaultへ戻さない。redesign migrationはpre-release設定を一度削除する。古いregion/grantを自動変換する経路ではない。[前方migration](../../crates/db/migrations/20261029010000_runtime_access_redesign.sql)・[読取](../../crates/server/src/remote_mcp/environment_settings.rs)

各要求と副作用直前に設定を照合する。OFFは後続操作を拒否するが既存processをkillしない。execution OFFでは観測/output/stopも拒否され、自然終了や既存timeoutは別に進む。file OFFでも許可されたprogramのOS書込を禁止するものではない。root消失時でもfile機能をOFFにする失効操作は可能である。

## ファイルの境界と逐次読取

`read_runtime_path`はlist/read、`change_runtime_path`はwrite/deleteを分離する。Workspaceの閲覧は可能だが、直接変更はidle/archivedも含むmanaged Workspace自身と子を拒否し、削除はその祖先も拒否する。Workspace変更は担当agentへ渡す。[境界](../../crates/server/src/remote_mcp/environment.rs)・[file実装](../../crates/server/src/remote_mcp/environment_files.rs)

root祖先からfd相対にsymlinkを辿らず、proc/sysや特殊fileを拒否する。hard link・bind mount・任意OS writerの完全隔離ではない。読取は既定16KiB、最大512KiB。inode/metadata/256byte anchorのcursorでrotation/truncation等を検査し、読取中変更なら次cursorを返さない。非UTF-8・NUL・UTF-8境界片はbase64になる。immutable snapshotや全rewrite検知ではない。[read/cursor](../../crates/server/src/remote_mcp/environment_files.rs)

writeは既存parent内に最大512KiB UTF-8の通常fileを作成/原子的置換する。既存permissionを保ち、新規はprivate permissionとなるがowner/ACL/xattrの保持保証はない。任意expected snapshot比較はbest-effortでkernel CASではない。append/patch/mkdir/moveはない。recursive deleteは明示指定し、失敗時にも一部削除済みになり得る。置換後の観測喪失はuncertainとなる。再試行前に実状態を確認する。

## プロセス観測と一 PID の制御

`observe_process`はsame UIDのlist/inspectと所有managed outputを統合する。argv/environmentは出さず、boot/PID/start ticksのidentityを使う。実行fileのexec変更そのものは拒否理由ではない。観測/stopはfile rootsや現在cwdに制限されず、managed processはprincipal/Project帰属を要求する。[観測](../../crates/server/src/remote_mcp/environment_process.rs)

`run_process`は絶対executable/cwdを要求し、起動時のpinned cwdがWorkspace自身/子であれば拒否する。Workspace祖先や別siblingは許可するが、起動後のcwd監視は行わない。command語の検閲やexecutable配置制限はなく、programはcwd/file roots外にもアクセス可能である。既存Workspaceをprogram経由で直接編集する許可にはならない。

子の環境はclearし、`PATH=/usr/local/bin:/usr/bin:/bin` と `LANG=C.UTF-8` だけを渡す。serverのHOME、provider資格情報、Cargo/NVM/virtualenv設定を継承する前提で実行しない。必要な実行accountの設定は明示した絶対pathや信頼できるwrapperで準備する。有限寿命は1〜3600秒、serviceは1〜86400秒。timeoutはTERM後5秒で必要ならKILLする。pidfdを使いexactな一PIDを対象とし、PID1・LVK自身・祖先を保護する。子孫やprocess groupの停止保証ではない。stop receiptは終了証明でなく、再観測が必要である。

## 出力・receipt・再起動

outputはstreamごと1MiB、最大128記録の揮発bufferで終了記録をevictする。offset喪失とcapture drain期限を区別し、要求は既定16KiB・最大64KiB。起動receiptはstartup受付であってhealthやtest passではない。起動・stop・file変更は既存Operationsを使い、応答喪失時は同じrequest IDと入力を再送/読取る。uncertainは新IDでの盲目的再実行を許可しない。file内容/command argsをauditへ直接保存せず入力hashとtargetを残す。[dispatch](../../crates/server/src/remote_mcp/environment.rs)

server再起動後はoutputと寿命monitorを復元しない。耐久launch receiptのprincipal/Project帰属は残し、旧processを無所有として再割当しない。結果unknownと受付済みを区別する。runtime receiptのget_operationは追加runtime:readを要する履歴参照である。[所有回復](../../crates/server/src/remote_mcp/environment.rs)・[receipt読取](../../crates/server/src/remote_mcp/issue_write.rs)

## 開発専用 self_restart

`self_restart` / `get_self_restart_operation`はproject_idとexecution許可、installed instanceの固定switch/start_binary.pyとlaunch.json、no_new_work、絶対bundleを使う。一般run_processで代用しない。実loopback listener・PID/start/boot・instance/data/current・launch設定hashをpipeで渡し、workerが停止前に独立照合する。管理PTYの終了、busy再確認、backup/health/receiptは既存switch kitが所有する。受付とcutover成功を分け、再接続後は同request ID・同principal/Projectで結果を読む。[bridge](../../crates/server/src/remote_mcp/environment_switch.rs)・[切替手順](../../deploy/developer/switch/README.md)

## 検証範囲

runtime HTTP fixturesは隔離SQLite/filesystemとmock IdPでscope/Project、設定、Workspace/path、receiptと応答上限を検査する。Chromium設定fixtureは製品componentとAPI clientを使うがisolated HTTP応答で、live設定DBではない。実Auth0/ChatGPT・稼働devのrestartや共有DB検証に読み替えない。[fixture](../../crates/server/src/remote_mcp/environment_tests.rs)・[設定試験](../../crates/server/src/remote_mcp/environment_settings_tests.rs)

## 失敗診断と部分作用

副作用前の入力・権限・設定拒否は確定した失敗として分類し、起動・停止・ファイル変更後に結果を確定できない場合は `uncertain` / `outcome: unknown` を維持する。失敗receiptのpath・cwd・command・入力本文などは秘匿投影され、ID・revision・部分作用の事実を残す。診断自由文から未実行を推定せず、元requestと対象状態を確認する。[実行境界](../../crates/server/src/remote_mcp/environment.rs)・[投影](../../crates/utils/src/mcp_error.rs)
