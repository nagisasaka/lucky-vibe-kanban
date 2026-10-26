# 公式CLIから利用枠とusageを読む

MCP `read_provider_usage` は、Codex / Claude Codeの公式CLI control接続から必要時に読み取る。既存のWorkspace token累積値には書き込まない。UI、価格表、定期poll、監視サービスは追加しない。

## 明示するcontextと権限

```json
{"provider":"codex","account_context":"server_default","include_usage":true}
```

`provider` は `codex` または `claude_code`。`server_default` はLVKサーバーの当該provider DEFAULT launcherと既存認証であり、呼出元WorkspaceやProjectの環境・設定ではない。曖昧な省略や任意のnative IDは受け付けない。サーバー管理accountの情報なのでadministratorまたは認証済みLVK internal agentと `lvk:read` を要求する。新しいloginやgrantは作らず、認証refreshは公式CLIに任せる。passive接続ではlauncherの実行ファイルpathだけを許し、base commandに付いた引数やadditional_params、routerはunsupportedとする。設定済みprompt/resume/shell引数を情報取得で誤実行しないためである。stdioとHTTP MCPは同じサーバー処理を使う。

既存Sessionのusageを要求する場合だけ `project_id` / `workspace_id` / `session_id` を全て指定し、`include_usage=true` にする。サーバーは所属、providerと保存されたruntime profileがDEFAULTと一致することを確認してnative IDを解決する。外部MCPでは接続に許可されたProjectも確認する。provider側の現accountでnative IDが読めない場合は未取得となる。profile名の一致は、認証後のaccountが過去と同一である保証ではない。

## 接続と終了

Codexは既存AppServerClientとowned discovery transportを再利用し、一時的なLVK管理ディレクトリでapp-server → initialize / initialized → account RPC → 終了回収と進む。実行用callbackは使わない。新schemaはrust-v0.164.0の公開型に沿う小さなserde projectionで、compile-time protocol依存はrust-v0.144.1のまま。`supportsLunaReserve` は送らず、`excludeResetCreditDetails=true` を送る。古いCLI等の失敗時には、別schema・ログ・モデル実行への自動fallbackはしない。

Claudeは既存ProtocolPeerにrequest_id相関を追加し、initializeのcontrol responseを待ってから `get_usage` / `skip_behaviors=true` を送る。CLIにはpromptを渡さず、hook無効、空のMCP構成、tools無効、非永続sessionを指定する。Project/local設定を読み込ませず、user認証設定を使用する。`--bare` はOAuthを無効にするため使わない。get_usageはAgent SDK v0.3.301の実験的wire contractに基づく。古いCLIがskip_behaviorsを無視して走査する危険を避けるため、同じlauncherの公開 `--version` を5秒以内で確認し、検証対象の2.1.308だけに送信する。CLI解決は2秒以内のfilesystem lookupだけを使い、未検出時にlogin shellやstartup fileを実行してPATHを更新しない。未知版はget_usageを送らずunsupportedとし、互換検証後に対象を追加する。返却behaviorsがnullであることも検査し、未確認の返却形を成功扱いしない。version probeにもowned process回収を使う。実CLIの非対応・取得失敗は通常の未取得結果として扱う。

両providerとも初期化/readは全体15秒、account/control RPCは5秒。Claudeはその前のversion probeに最大5秒を使い、取得の上限は合計20秒。終了・回収には別途短い猶予がある。呼出切断は自分で起動した接続だけを終了する。各providerの同時接続は1、起動頻度は最大5秒に1回。重複は待機や再試行を増やさず理由付きunavailableを返す。短命接続の受信は各2 MiBまで、usageのbucket/group件数も有界とする。回収失敗時はそのproviderの新規read受付を閉じ、成功・idleとは報告しない。常駐の取得プロセスはない。

生存中の実行接続はprocess-hostの別プロセスに属し、安全にaccount readを中継する既存interfaceがないため、この段階では再利用しない。実行中runのsteer/cancel/controlは操作しない。Claudeの短命接続が返すsession usageは `information_connection_session_only` として返し、既存Workspaceのusageに転用しない。既存Claude Sessionのusageを要求するとunsupportedを返す。対象Sessionのresumeやtranscript走査で補わない。

## 結果の意味

quotaとusageは別の結果であり、一方の通常RPC失敗だけで他方を捨てない。未報告/nullは0にも無制限にも変換しない。取得失敗時はstaleを捏造せずunavailable。自動retryやtimerはない。`observedAt` は今回の観測時刻であり、providerの計上時刻・即時反映保証ではない。

Codexの `rateLimitsByLimitId`、primary/secondaryの `usedPercent`、`windowDurationMins`、Unix秒の `resetsAt` を保持する。5時間/週間とは決め打ちしない。remainingを表示するなら同じwindowの使用率から `100 - usedPercent` と導出した値だと明示する。Claudeのsnapshot `utilization` は0–100で、通知eventの0–1とは別契約。

Codex usageのaccount summaryとnative threadのbreakdownを区別し、input/cache/output、model/effort/speed、推定credits/USDのmicros（100万分の1）を保持する。inputにcacheが含まれるか、outputにreasoningが含まれるかを変えず、親子threadやaccount quotaを合算しない。Claudeの `total_cost_usd` やproviderのestimated USDはsubscriptionの実請求額ではない。生レスポンス・認証情報・account ID・prompt・behavior走査結果は返却/ログへ流さず、既知のusage項目だけを投影する。

## 検証

fixtureはモデルを一切起動せず、受信したinitializeとusage RPCのみを許可するCLI doubleを使う。子プロセスを作り、成功・失敗・切断・timeout後の終了回収も確認する。認可/所属は隔離SQLite fixtureで検査する。

live readは通常のテストでは行わない。明示承認のある環境でのみ以下を実行する。出力はstatusだけで、account/quotaの値や認証情報を表示しない。

```bash
node scripts/dev-cargo.mjs test -p executors live_provider_usage_observations -- --ignored --nocapture
```

2026-10-26に独立した `--version` で確認したPATHの実CLIはCodex 0.163.0 / Claude Code 2.1.308。user報告のCodex 0.164.0と混同しない。live read結果と未検証範囲は実施後のChange Manifestを参照する。実モデル、thread/turn開始・resume、ログ走査、deploy/restartはこの検証に含まない。

## 一次契約

- [Codex App Server](https://learn.chatgpt.com/docs/app-server)
- [rust-v0.164.0 account型](https://github.com/openai/codex/blob/rust-v0.164.0/codex-rs/app-server-protocol/src/protocol/v2/account.rs)
- [rust-v0.164.0 thread usage型](https://github.com/openai/codex/blob/rust-v0.164.0/codex-rs/app-server-protocol/src/protocol/v2/thread_usage.rs)
- [Anthropic公開SDK v0.3.301 型](https://app.unpkg.com/@anthropic-ai/claude-agent-sdk@0.3.301/files/sdk.d.ts)
