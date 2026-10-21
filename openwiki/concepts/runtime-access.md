---
type: concept
title: 実行環境へのアクセス
description: 外部 HTTP MCP の Linux runtime grant、管理設定、ファイル・プロセス観測と直接実行の境界、取消・再起動・不確実な結果を説明する。
tags: [runtime, mcp, authorization, processes, lifecycle]
verified:
  - by: openwiki/0.5.1
    at: 2026-10-21T05:59:59.398Z
sources:
  - id: openwiki-source-b4f827f199bc6b188f7969cf
    resource: repo://crates/db/migrations/20261020200000_runtime_access_settings.sql
  - id: openwiki-source-8ab39f439cdb152ec09eb06c
    resource: repo://crates/server/src/remote_mcp/environment_files.rs
  - id: openwiki-source-076036ed7bdac82d628d39d1
    resource: repo://crates/server/src/remote_mcp/environment_process.rs
  - id: openwiki-source-6dda51a64e0e3f9725fd2c6e
    resource: repo://crates/server/src/remote_mcp/environment_restart_tests.rs
  - id: openwiki-source-1bf95d488567f09e0c725dee
    resource: repo://crates/server/src/remote_mcp/environment_settings_tests.rs
  - id: openwiki-source-6cb471b32e19ce921b78d86e
    resource: repo://crates/server/src/remote_mcp/environment_settings.rs
  - id: openwiki-source-be65ae3f61eba813d7981f61
    resource: repo://crates/server/src/remote_mcp/environment_tests.rs
  - id: openwiki-source-0801a7e5bbcc064c12effc80
    resource: repo://crates/server/src/remote_mcp/environment.rs
  - id: openwiki-source-9e15762bf7a432b7605f8466
    resource: repo://crates/server/src/remote_mcp/issue_write.rs
  - id: openwiki-source-71f837a8aa36443ad9688945
    resource: repo://crates/server/src/remote_mcp/mod.rs
  - id: openwiki-source-6adda10f98b079272f82a4cc
    resource: repo://crates/server/src/remote_mcp/operations.rs
  - id: openwiki-source-9f247c0205880588f54bff7e
    resource: repo://docs/development/runtime-access-review.md
  - id: openwiki-source-8f6f080ee7500026352930b6
    resource: repo://docs/self-hosting/runtime-access.mdx
  - id: openwiki-source-688a3a970a72f7fa2b5dbc1c
    resource: repo://packages/web-core/src/shared/dialogs/settings/settings/RuntimeAccessSettingsSection.tsx
generated: { by: "codex", at: "2026-10-21T05:59:59.398Z" }
---

# 実行環境へのアクセス

Runtime Access は、管理者が明示許可した Linux 上のアプリケーション領域で、外部会話からログを読み、既存プログラムを起動・停止する機能である。[外部 MCP](../integrations/remote-access.md) の認証・操作 receipt を再利用し、coding agent の Session / AgentRun を作らない。設定・ソース・依存・スクリプトを変更する仕事は従来の [Issue → Workspace → 統合](../workflows/task-to-integration.md)へ渡す。[公開契約](../../docs/self-hosting/runtime-access.mdx#choose-the-appropriate-workflow)・[実装の所有境界](../../crates/server/src/remote_mcp/environment_process.rs#L1-L34)

## Grant と権限の重なり

region は管理者 grant の不透明な ID であり、Project、絶対 root、正確な subject の集合、実行可能ファイルの集合、`allow_uncoordinated_execution` を結びつける。表示名は ID の代わりにはならない。root と executable は設定時に canonicalize され、root は存在する具体的なディレクトリーで `/` は拒否される。最大64 grant、各100 subject・64 executable、設定本文64 KiBという上限がある。executable が空でもファイル参照はできるが、プロセス観測・実行は許可されない。[grant 検証](../../crates/server/src/remote_mcp/environment.rs#L13-L131)・[保存上限](../../crates/server/src/remote_mcp/environment_settings.rs#L166-L201)

既定では無効である。[サーバー認証](server-access.md)の active member と `lvk:read` に加え、観測には `lvk:runtime:read`、起動・停止にはさらに `lvk:runtime:execute` が必要となる。scope だけでは足りず、現在の grant の subject と Project も照合する。既存コード参照の権限を runtime へ拡張したものではない。[HTTP scope](../../crates/server/src/remote_mcp/mod.rs#L167-L240)・[grant と dispatch](../../crates/server/src/remote_mcp/environment.rs#L329-L429)

`+r +x -w` は公開する操作と役割の方針であり、OS の書込 sandbox ではない。実行したプログラムは LVK の OS ユーザー権限で副作用を起こせる。file-write / patch / delete / Git mutation 専用 tool はなく、inline program や編集コマンドを送らないという利用契約があるが、実装はコマンド語の blacklist ではない。ログ・出力も承認ではなく、秘密を含み得る参照データである。[契約と限界](../../docs/self-hosting/runtime-access.mdx#choose-the-appropriate-workflow)・[実際の spawn](../../crates/server/src/remote_mcp/environment_process.rs#L300-L324)

## 管理設定の保存・適用・取消

Settings → MCP連携 → 実行環境へのアクセスは、[coding provider の MCP 設定](../integrations/agent-providers.md#coding-agent--外部-mcp-server)とは別である。`GET/PUT /api/runtime-access-settings` は browser 管理者用で、web/basic/local channel の admin だけを許す。外側の認証・CSRFに加えて、保存 transaction 内で非local actorの active admin を再確認する。内部 agent token や MCP bearer に設定拡張権限を与えず、設定 writer は MCP catalog に存在しない。[管理 API](../../crates/server/src/remote_mcp/environment_settings.rs#L62-L81)・[保存](../../crates/server/src/remote_mcp/environment_settings.rs#L191-L245)・[UI 接続](../../packages/web-core/src/shared/dialogs/settings/settings/settingsRegistry.tsx#L95-L143)

保存先は singleton `runtime_access_settings`。保存は `BEGIN IMMEDIATE` 内で expected revision を照合し、増分 revision・設定本文・audit を同時に確定する。競合は409で再読込を要求する。保存済み grant を変えずに機能を無効にする場合は、root や Project が消失していても失効可能とする。再有効化や grant 変更は再検証する。[schema](../../crates/db/migrations/20261020200000_runtime_access_settings.sql)・[保存規則](../../crates/server/src/remote_mcp/environment_settings.rs#L166-L239)

最初の保存前だけ `LVK_RUNTIME_ACCESS_CONFIG` の legacy JSON が fallback になる。保存済み無効設定も含め、DB row があればそれだけが正本で、legacy と merge しない。各 MCP 呼出しで設定を読み直し、受付済みの起動・停止も実行直前に grant 全体の一致を再確認する。変更されていれば失敗させ、意図した新しい要求を必要とする。router が共有する registry は保存で作り直さないため出力バッファを失わない。[設定読取](../../crates/server/src/remote_mcp/environment_settings.rs#L83-L162)・[共有 registry](../../crates/server/src/remote_mcp/mod.rs#L55-L93)・[再照合](../../crates/server/src/remote_mcp/environment.rs#L466-L486)

legacy の読取不能・JSON不正・有効 grant 不正だけは、画面で `invalid_legacy` と型付き理由を示す。閲覧ではDBへ書かず、明示した「無効・空の設定を保存して復旧」が revision 0 から最初の保存を行う。runtime の読取はその間失敗する。保存済みJSON破損やDB障害は503であり、無効な初期値へ黙って戻さない。[分類](../../crates/server/src/remote_mcp/environment_settings.rs#L24-L36)・[読取と復旧 view](../../crates/server/src/remote_mcp/environment_settings.rs#L83-L162)・[UI の明示操作](../../packages/web-core/src/shared/dialogs/settings/settings/RuntimeAccessSettingsSection.tsx#L44-L115)

region・subject・機能の取消しは後続の停止や出力取得も拒否するが、既に動く子を kill せず、所有権も移さない。記録された製品判断は自然終了待ちで、必須のOS cleanup、管理者 takeover UI、自動再許可は追加していない。既存 timeout は維持されるが、長期 daemon は自然終了しない可能性がある。[取消の実装テスト](../../crates/server/src/remote_mcp/environment_settings_tests.rs#L170-L224)・[決定の記録](../../docs/self-hosting/runtime-access.mdx#enable-an-explicit-grant)

## Workspace と共有資源の境界

起動・停止は明示的な `allow_uncoordinated_execution` が必要である。全 Workspace の非NULL `container_ref` のうち canonicalize できるパスと root が一致・祖先・子孫関係なら拒否する。Project、archived、AgentRun の稼働状態で絞らず、cwd だけが重ならなくても root が重なれば拒否する。読取にはこの実行専用判定を適用しない。存在しない／正規化できない Workspace パスは判定から外れる。[判定](../../crates/server/src/remote_mcp/environment.rs#L276-L298)・[idle/archived と読取の回帰](../../crates/server/src/remote_mcp/environment_tests.rs#L431-L465)

これは [共有資源の原子的 claim と実行](shared-resource-coordination.md)ではない。受付と実行前の再検査は予約を取得せず、後からの Workspace 登録を排他的に防ぐものでもない。新しい scheduler や競合する Workspace lease を作らないために保守的な制限を置いている。旧レビューの「判断待ち」は履歴で、現行の初版は制限維持が承認済みである。汎用 Workspace 実行への拡張案は未実装である。[判定の意図](../../crates/server/src/remote_mcp/environment.rs#L276-L298)・[更新された判断と過去の選択肢](../../docs/development/runtime-access-review.md#1-workspace-重複拒否挙動は再現アクセス方針は判断待ち)

## ファイルとログを観測する

`get_runtime_access` で自分の region と限界を確認し、`list_runtime_files` / `read_runtime_file` で相対 path を読む。Git tracked だけに限定しない。root の祖先も含め fd 相対の `openat` / `O_NOFOLLOW` で辿り、symlink、traversal、特殊ファイルを拒否する。最終 inode は型確認後に固定した fd 経由で開く。procfs/sysfs はファイルシステム型でも拒否する。通常の bind mount / hard link は管理者が許可した範囲に入り、別の隔離境界を作らない。[file 境界](../../crates/server/src/remote_mcp/environment_files.rs#L1-L142)

一覧は名前順で既定100・最大500件、最大100,000 entryの走査上限を持つ。続きは `next_offset` と `snapshot` を組で送り、directory変更なら再一覧する。symlinkや特殊 entry は拒否種別として一覧に現れるが読めない。読取は既定16 KiB・最大64 KiBの bytes、offset・tail・cursor は排他的である。非UTF-8、NUL、UTF-8境界断片はbase64となる。[一覧](../../crates/server/src/remote_mcp/environment_files.rs#L184-L241)・[読取](../../crates/server/src/remote_mcp/environment_files.rs#L254-L337)

cursor はpath、device/inode、metadata、offset、直前最大256 bytesのhashを持つ。rotation・truncation・検出した書換えは読み直しを要求する。`changed_during_read` が真なら再利用cursorを返さない。これは全ファイルのimmutable snapshotではなく、anchorより前の書換えと追記の組合せ等は検出できない。観測時刻・identity・変更状態を保ってログを引用する。[anchor と検査](../../crates/server/src/remote_mcp/environment_files.rs#L243-L337)

## 起動・観測・停止の寿命

`run_process` は絶対 executable とargv、root内の相対cwd、purpose、`request_id`、寿命を受け取る。有限実行は最大3,600秒、serviceは最大86,400秒。argvは最大128件・合計32 KiBで、receiptはargument値を保存せず入力hashと件数を保持する。子のstdinはnull、環境をclearして固定PATHとLANGだけを設定する。HOMEやtoolchain環境が要るなら、通常のcoding経路で準備した既存wrapperが必要になる。[受付検証](../../crates/server/src/remote_mcp/environment.rs#L424-L466)・[spawn](../../crates/server/src/remote_mcp/environment_process.rs#L300-L324)

同じ `request_id` の回復では元の入力を保持する。既存 [operation receipt](../integrations/remote-access.md#参照結果と操作-receipt) のprincipalはissuer/subjectで、Project・operation・入力hashの不一致を拒否する。起動の `succeeded` はstartup observationであり、コマンドの成功、service health、build commit、完了を証明しない。`get_operation` で runtime receipt を読むには runtime read scope も要るが、現在のregion grant確認とは異なる履歴読取である。[receipt identity](../../crates/server/src/remote_mcp/issue_write.rs#L80-L86)・[履歴読取](../../crates/server/src/remote_mcp/issue_write.rs#L156-L180)・[再送](../../crates/server/src/remote_mcp/operations.rs#L208-L245)・[起動応答](../../crates/server/src/remote_mcp/environment_process.rs#L428-L430)

`list_processes` / `inspect_process` は同じUID、現在のcwdがroot内、現在のexecutableがallowlist内という条件を使い、argv/envを返さない。identityはboot ID・PID・start ticksである。launcherが`exec`で別binaryへ変わると、遷移先も許可されていなければ一覧・inspect・stopが拒否される。管理run IDは現在の権限を広げない。既存プロセスのstdoutは取得できず、設定済みログを読む。[観測とscope](../../crates/server/src/remote_mcp/environment_process.rs#L69-L181)・[exec 遷移テスト](../../crates/server/src/remote_mcp/environment_process.rs#L668-L744)

`read_process_output` は起動応答の `run_id` を `process_id` として受け、所有者とrootを照合する。各streamは末尾1 MiB、registryは最大128記録。容量不足時は終了済み記録をevictし、空きがなければ起動を拒否する。byte offsetには `dropped_before` / `cursor_lost` / `cursor_ahead` があり、UTF-8-lossy表示と不正UTF-8 flagを返す。状態・exit codeとcapture完了は別で、子孫がpipeを保持すると500 msのdrain期限でcaptureを打ち切る。[buffer](../../crates/server/src/remote_mcp/environment_process.rs#L19-L66)・[容量](../../crates/server/src/remote_mcp/environment_process.rs#L279-L299)・[drain と終了](../../crates/server/src/remote_mcp/environment_process.rs#L355-L441)・[出力認可](../../crates/server/src/remote_mcp/environment_process.rs#L432-L458)

停止はpidfdを開いてidentityを再検証し、正確な一PIDだけへSIGTERM、明示 `force:true` ならSIGKILLを送る。LVK自身・祖先・PID 1以下は拒否し、process group・子孫は停止しない。通常の寿命切れもTERM→5秒待機→KILL→5秒待機で、終了を確認できなければunknownとなる。停止受付の `stop_requested:true, stopped:false` は終了証拠ではない。管理出力のterminal state等で確認し、観測不能だけを終了と断定しない。[pidfd](../../crates/server/src/remote_mcp/environment_process.rs#L119-L217)・[timeout](../../crates/server/src/remote_mcp/environment_process.rs#L355-L386)・[stop](../../crates/server/src/remote_mcp/environment_process.rs#L459-L496)

## 再起動と不明な結果

起動・停止receiptは永続化するが、出力、exit outcome、寿命monitorは復元しない。旧process instanceのaccepted/running receiptは読取時にuncertainとなり、自動再実行しない。既に成功したstartup receiptも現在のhealthへ更新されない。再起動後は再観測して判断する。[receipt](../../crates/server/src/remote_mcp/operations.rs#L208-L245)・[揮発registry](../../crates/server/src/remote_mcp/environment_process.rs#L19-L34)・[再起動テスト](../../crates/server/src/remote_mcp/environment_restart_tests.rs#L108-L242)

生き残った管理対象を「所有者不明の外部プロセス」として扱い直さないため、stopは永続launch receiptをidentity/run_idで検索し、issuer/subject owner、Project、grant IDを照合する。停止時にはさらに現在のgrant、executable/cwd、identityを確認する。detached子孫や失われたmonitorを補うsupervisorはない。[durable owner](../../crates/server/src/remote_mcp/environment.rs#L298-L327)・[停止への接続](../../crates/server/src/remote_mcp/environment.rs#L519-L529)

## 変更時の検証と残る範囲

変更時はHTTP認可・receiptだけでなく、管理API/CSRF、保存優先順位、ファイル境界、exec遷移、子生存中のhost再起動を一緒に確認する。[HTTP契約](../../crates/server/src/remote_mcp/environment_tests.rs)・[設定と失効・legacy復旧](../../crates/server/src/remote_mcp/environment_settings_tests.rs)・[別OSプロセスのrestart fixture](../../crates/server/src/remote_mcp/environment_restart_tests.rs)。UIテストはstatic renderとAPI mockで、実browser操作の証拠ではない。[UI](../../packages/web-core/src/shared/dialogs/settings/settings/RuntimeAccessSettingsSection.test.tsx)・[API](../../packages/web-core/src/shared/lib/runtimeAccessApi.test.ts)

runtime設定migrationは `20261020200000` で、別機能の `20261020190000` との番号衝突を避けている。freshと両側からのupgrade fixtureはあるが、旧番号のruntime migrationを適用済みの実DBの移行手順は対象外で、履歴を書き換える根拠にならない。[migration試験](../../crates/server/src/remote_mcp/environment_settings_tests.rs#L434-L497)・[変更理由と限界](../../docs/development/runtime-access-review.md#b171f645-に対する追加修正2026-10-20)

今回のSyncは統合済みsourceとテスト内容の静的照合で、製品テストを再実行していない。凍結入力の正式Integrationは明示的に検証省略であり、開発時成功報告と区別する。restart fixtureはmock認証のHTTP hostで、full serverのshutdown hook、本番Auth0/ChatGPT、実PID番号再利用、spawn→receipt確定間の各点のcrash、実Cargo/nvm/venv環境を保証しない。[検証限界の記録](../../docs/development/runtime-access-review.md#3-子が生存中のホスト終了再起動限定した実プロセス検証で成功)
