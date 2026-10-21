# LOCAL-11 追加レビュー（2026-10-20）

後続のUX改善依頼で、初版はWorkspace重複領域のrun/stop拒否を維持する選択肢A（依頼上の選択肢1）が承認された。以下の「判断待ち」はレビュー当時の記録であり、今回の実装判断ではない。設定UIは「MCP連携 → 実行環境へのアクセス」に追加する。

調査開始時 HEAD は `144fd049fbf473389ea1939b793793572cd77af4`。指定基準と同一で、作業ツリーはクリーン、基準以後の修正はなかった。本変更は再現テストと説明の追加であり、ランタイムのアクセス方針・認可・実行機構は変更しない。mainへの統合、デプロイ、稼働LVKの操作は行わない。

## 1. Workspace 重複拒否：挙動は再現、アクセス方針は判断待ち

`crates/server/src/remote_mcp/environment.rs:254` の `execution_region` は全Workspaceの非NULL `container_ref` を取得し、正規化できたパスについて許可rootとの祖先・子孫・一致を拒否する。Project、archived、AgentRunの稼働状態による限定はない。cwdが重複していなくてもrootがWorkspaceを含めば拒否する。存在しない／正規化できないWorkspaceパスは判定から外れる。

適用先はrun/stop（同ファイル405、455行の受付前・実行時）で、ファイル読取やプロセス観察ではない。`environment_tests.rs:384` のHTTPテストに「AgentRunなし」「archived後もrun/stop拒否」「同じrootのファイル読取成功」を追加した。

理由は独立のWorkspaceリースを新設せず、既存作業との競合を保守的に避けるため。ただし既存ビルド／テストの実行要件を一部満たせなくする制約であり、「初版はWorkspace外のみ」が合意済みとは扱わない。また現行の二度の読取チェック自体も予約ではなく、後からのWorkspace登録等に対する原子的保証ではない。Codex停止中かのSELECTへ置き換えるだけではTOCTOUを解決しない。

### 既存基盤と実現可能な選択肢

- `crates/db/src/models/resource_coordination.rs:294,353`：既存資源操作はSession/interactive Workspaceを前提とし、`BEGIN IMMEDIATE`内で登録済み資源bundle・revision・fence・holderを原子的に取得する。Integration予約との検査もある。**登録してclaimした資源間**の排他であり、任意のcwdや通常のCodex実行が自動的に同じ資源をclaimするわけではない。
- `crates/server/src/routes/resource_coordination/runtime.rs:192`：既存script＋verificationをResourceCommandとしてExecutionProcessで実行。新しいschedulerは不要。ただし現在の直接MCPのargv/cwd、Session不要、長期service、exact-PID stopとは寿命・入出力・終了確認が異なる。
- `crates/services/src/services/integration_admission.rs:1`：`MUTATIONS`は短い単一サービス内の受付バリア。実行期間全体に保持するリースではなく、同ファイルも長時間保持を禁止している。
- `crates/db/src/models/integration.rs:212,226` と `crates/local-deployment/src/agent_run_port.rs:709`：Workspace予約とagent dispatch検査が存在する。ただし予約はIntegration run所有で、runtime操作を架空のIntegrationにして流用してはいけない。
- `crates/services/src/services/resource_coordination.rs:77`、`workspace_usage.rs:354`、`crates/server/src/routes/integrations/admission.rs:47`：未解決資源操作はcleanup/Integrationのidle検査対象。しかし通常agentの受付とruntime操作に共通する汎用Workspace予約にはなっていない。agent identity永続化のトランザクション（`crates/db/src/models/agent_runtime.rs:943`）も資源claimではない。

| 選択肢（未実装・ユーザー判断待ち） | 制約と実装コストの見積り |
| --- | --- |
| A. 判断まで現行拒否を維持 | 今回実施。変更コストは小さいが、Workspace内実行の要件は未解決。恒久仕様として承認したことにはならない。 |
| B. 有限のbuild/testを既存ResourceCommand経路に接続 | 中〜大。Sessionとcanonical資源の対応、全競合actorの同じclaimへの参加、認可・receipt・出力の接続が必要。単に資源を登録するだけでは通常Codexとの競合を防げない。現在の長期serviceにはそのまま適用できない。 |
| C. 既存受付バリア＋予約検査を共通Workspace予約へ拡張 | 大。agent dispatch、Git/cleanup、Integration、runtime起動を同じ原子的予約に参加させる。予約保持期間、終了／子孫確認、ホスト終了後のunknownと回復を定義する必要がある。別supervisorは不要だが、単独関数の変更ではない。 |

B/Cともpath alias・bind mountのcanonical資源対応を管理者が定義し、既存の共有資源制御を迂回しないことが前提。見積りは変更範囲に基づく相対評価で、実装完了の主張ではない。今回どのアクセス方針にも変更していない。

## 2. exec 遷移と環境：設定依存の制限を再現

`environment_process.rs:131,173,459` は現在のUID/cwd/executableと同一性を再検証する。`exec_transition_requires_target_grant_and_minimal_environment`（同668行）は既存shell scriptが`exec /bin/sleep`へ遷移したことを実際のexeで待って確認する。

- launcherだけ許可：起動・所有者の出力取得は可能だが、listから消え、inspectとstopは拒否。管理run_idでも外部既存プロセスのidentityでも許可は広がらない。
- launcherと遷移先を許可：同じPID/起動identityでinspect・管理対象stopが成功。他所有者のstopは拒否。
- `env_clear`＋固定PATH/LANG（同316行）を確認。HOME/CARGO_HOME/NVM_DIR/VIRTUAL_ENVは未設定。`existing_wrapper_can_prepare_explicit_nonsecret_environment`（同746行）は既存wrapperの明示exportとactivation fixtureによる必要パス設定が使えることを確認する。

したがって「既存scriptのexecは常に停止不能」という判定は非再現。launcherのみの設定では制限が再現する。managedとexternalの区別は所有者・rootを保持するregistry／durable launch receiptで行い（`environment.rs:277`）、さらに現在のgrantとboot/PID/start ticks、pidfdで確認する（`environment_process.rs:192`）。run_idの無条件信頼やallowlist撤廃は行わない。

最小戦略は既存の管理者grantにlauncherと実際の遷移先を明示し、必要なら通常のCodex経路で既存wrapperを整えること。HOME/CARGO_HOME/RUSTUP_HOME、nvmの明示source、venvの絶対interpreter/activationの注意を[利用手順](../self-hosting/runtime-access.mdx#existing-toolchain-scripts)に追記した。全環境継承や新しいenv設定APIは追加していない。wrapper変更が必要なら直接MCPで編集してはいけない。実際のCargo/nvm/venv製品環境は実行しておらず、activation fixtureの成功と区別する。

## 3. 子が生存中のホスト終了・再起動：限定した実プロセス検証で成功

`crates/server/src/remote_mcp/environment_restart_tests.rs:49,74,108` はlibtest実行ファイルを別OSプロセスとして起動し、**実際のHTTP MCP router、runtime processコード、migrate済みファイルSQLite receipt**を動かす。認証providerはローカルmock。隔離controllerだけをsubreaperにし、テストhostをSIGKILL/waitする前に子のexec遷移を確認する。同じDBで新しいhostを起動し、子の生存とidentity一致を確認する。旧テストの「子の停止後のrouter再生成」とは異なる。

確認内容：再観察成功、同じrequest_idのreceipt再照会・再送で同一run_id、起動回数ファイルが1回、別grant停止拒否、start ticksを変えたidentity拒否と元の子の生存、元のrun_idによる停止要求受付、controllerのwaitpidによる独立した終了確認、終了後のinspect/stop拒否。古いstdout/stderrは取得不能を返し、成功した起動receiptの内容は履歴のまま変わらない。取得不能な終了コードを生成しない。

これは製品の`server` main全体・shutdown hooks・コンテナ再起動の試験ではない。その範囲、本番Auth0/ChatGPT、実LVKサーバーでの試験は未実施。実際のPID番号再利用を強制する試験ではなく、同じ生存PIDに誤ったstart ticksを指定して拒否を確認している。spawnからreceipt確定までの各命令点でのクラッシュ注入も未実施。既存`operations`テストは未確定receiptをuncertainにして再実行しない挙動を別途検証する。

出力とmonitorは揮発性であり、ホスト消失後は元の実行期限による監視も失われる。今回の子には独立した30秒の有限寿命を設定し、通常経路では明示停止する。製品へのsupervisor、出力永続化、自動復旧は追加しない。

## 検証記録

実行時は既存AGENTS.mdに従い、`SCCACHE_DIR`をcanonical `.lvk-shared/cache/sccache`、`RUSTC_WRAPPER=/usr/local/bin/sccache`として実行。実サーバーや共有DBは使用せず、一時ディレクトリのDBと子プロセスだけを使用した。

- `cargo test -p server --lib remote_mcp::`：54 passed、0 failed、2 ignored。ignoredの2項目は通常単独実行しないsubprocess専用fixtureで、親テストから明示起動されている。
- 最後のfixture cleanup／終了済みstop検査追加後、`cargo test -p server --lib remote_mcp::environment`：19 passed、0 failed、2 ignored（専用fixtureは親から起動）。
- `cargo clippy -p server --all-targets -- -D warnings`：成功。`git diff --check`：成功。

受付成功はコマンド／サービスの成功ではない。本レビューのテスト成功は正式Integrationや製品全要件の完了でもない。アクセス方針の判断を残してコミットし、正式Integration・反映は行わない。

## b171f645 に対する追加修正（2026-10-20）

- LOCAL-11 migrationを `20261020190000_runtime_access_settings.sql` から
  `20261020200000_runtime_access_settings.sql` へ変更。SQLxは説明名でなく数値prefixを
  versionとして扱う。LOCAL-19 `7a8ca3364839dcb917caeebdfacee0a7f0632c40` の
  同prefixのkanban migrationと衝突していた。共有migrationの内容は一致し、LOCAL-19だけに
  存在した2ファイルはテストfixtureへ読み取りコピーした。productionのmigration集合には
  取り込んでいない。
- 当Workspaceのprepare用SQLiteは残存していなかった。先行検証の一時DBは破棄済み。
  新たな一時DBで、両側の集合を含むfresh、LOCAL-19からのupgrade、番号変更後LOCAL-11からの
  upgradeをSQLx Migratorで検査し、prefix一意性・適用履歴・再実行を確認した。
  **旧番号のLOCAL-11を適用済みのDBの移行は対象外**。そのDBには別の明示的な移行判断が必要で、
  実DBの調査・適用・履歴書換えは行っていない。
- legacyのファイル読取、JSON、grant検証の失敗だけを `InvalidLegacy` として分類する。
  管理者画面は理由と「無効・空の設定を保存して復旧」を表示し、明示操作でrevision 0を
  保存する。新しい保存revisionは1。閲覧だけではDBに書き込まず、MCPの読込は引き続き拒否。
  保存済みJSONの破損やDB障害は503のままで、認可を省略しない。
- ユーザー決定により取消後は自然終了を待つ。後続操作は拒否し、強制終了も自動再許可も
  行わない。既存timeoutを維持し、daemonが自然終了しない可能性をUI・文書で説明する。
  管理者停止UIや必須OS cleanupは追加しない。

検証: `cargo test -p server --lib remote_mcp::` は58 passed / 0 failed / 2 ignored
（専用プロセスfixtureは親テストから起動）。新規復旧テストは初回、テスト利用者を管理者に
設定し忘れたため403で失敗し、fixtureを修正して再検証した。UI/APIの関連Vitestは3 passed。
実ブラウザー操作、実Auth0/ChatGPT、実DBの移行、配布サーバーへの反映は未検証。

追加の静的検証:

- `cargo clippy -j 1 -p server --all-targets -- -D warnings`: 成功。
- `pnpm run generate-types` / `pnpm run generate-types:check`: 成功。
  初回生成はsignal 9で終了し、環境のOOM kill記録を確認。空きメモリ回復後、
  `CARGO_BUILD_JOBS=1` で再実行して成功した。
- `pnpm run prepare-db` / `pnpm run prepare-db:check`: 一時DBで成功。
  無関係な既存メタデータ14件の削除は戻した。checkは既存のunused-query警告付きで成功。
- `pnpm run web-core:check` / `pnpm run local-web:check`: 成功。
- `pnpm --dir packages/web-core exec vitest run src/shared/lib/runtimeAccessApi.test.ts src/shared/dialogs/settings/settings/RuntimeAccessSettingsSection.test.tsx`: 3 passed。
- 変更したUI本体のESLint（local-web設定）、変更UI・翻訳のPrettier、変更Rustのrustfmt、
  `git diff --check`: 成功。
