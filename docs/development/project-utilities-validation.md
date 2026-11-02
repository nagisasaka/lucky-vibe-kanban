# Kanban / Workspace 右サイドバーの検証

2026-11-02 UTC。開始時の `main` と一致する `6fadb8e8` を基準に、Workspace内で実装・検証した。共有開発インスタンスのDB・実行成果物には適用していない。

## 表示対象と取得根拠

- 共通 `UtilitySidebar` / `EnvironmentPanel` を両ビューで使う。ヘッダーのCPU・RAM・DISK・アカウント利用枠を撤去し、同期エラー表示は維持する。Pet予定領域は既存クローバーSVG。
- ホスト名はLVKから見える `/proc/sys/kernel/hostname`。Dockerの識別根拠は `/.dockerenv` で、識別できない環境は境界未確認と表示する。ホストVPS名とは断定しない。
- 既存resource-observationの `/proc/stat` / `/proc/meminfo` によるCPU・RAM・SWAPはLinux system-visible値。コンテナ専用値とは断定しない。DISKは既存のworktree_base配置先FS。SWAP使用率から現在のメモリ逼迫を判断しない。
- コンテナ開始時刻を証明できる既存経路がなく、uptimeは「不明」。`/proc/uptime` やPID 1の開始時刻を代用しない。最終正常確認は別表示。
- 全Workspaceの `running` / `current` AgentRunと、現在のattemptの `running` process registryを結び、Sessionを重複なく数える。待機・承認待ち・起動待ち・終了済み等を除外し、SQL取得失敗はnullとする。
- Codex / Claudeは既存の認証済み公式CLI制御経路を使用する。アカウント枠とSessionコンテキストを分離し、期限切れ・未取得を正常値0や無制限に変換しない。
- mainユーティリティはProjectに登録されたrepo.pathを解決する。既存のパス境界検証・ファイルツリー／プレビュー・Wiki controller・PTYを再利用する。historical DTOのworkspace_idはこの経路だけProjectスコープを運ぶ。Workspace APIへ送らない。

## 実行した検証

| 検証                                                                                                   | 結果                                                                                      |
| ------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------- |
| local-web production build、web-core / UI型検査                                                        | 成功                                                                                      |
| local-web / UI lint                                                                                    | 成功                                                                                      |
| 変更したweb-core部品の対象lint                                                                         | 成功。NavbarContainer / actionsは既存のshared→features import違反を除外して他の規則を検査 |
| `pnpm run i18n:check`                                                                                  | 成功。英日辞書と既存fallbackを維持                                                        |
| Wiki・ファイル・表示設定・Claude枠のVitest                                                             | 97件成功                                                                                  |
| resource / Codex modelのNodeテスト                                                                     | 13件成功                                                                                  |
| `node scripts/dev-cargo.mjs test -p server resource_observation`                                       | 19件成功、明示的live_smoke 1件は既存のignoreを維持                                        |
| `node scripts/dev-cargo.mjs test -p server routes::workspaces::files`                                  | 14件成功。symlink escape、認証、rawヘッダー・全バイト保持等を含む                         |
| `node scripts/dev-cargo.mjs clippy -p server --all-targets -- -D warnings`                             | 成功                                                                                      |
| `node scripts/dev-cargo.mjs build -p server --bin server -p local-deployment --bin agent-process-host` | 成功                                                                                      |
| `pnpm run generate-types`                                                                              | 成功。WikiSnapshotSourceへmain_repositoryを追加                                           |
| `node tests/server-access/workspace-wiki.mjs test-results/project-utilities`                           | 専用DB・一時repo・実API/PTYのChromium検証が成功                                           |

ブラウザー検証は1600px / 1000px / 390px、英語・日本語で行った。両サイドのナビゲーションとボードを残した本文ペインのリサイズ、Wiki／ファイルの切替、ターミナル初期cwdと接続の重複防止、Workspaceの未送信下書き保持、複数repoのWiki有効／無効分離、取得エラー時の不明表示を確認した。

利用枠のHTTP応答はブラウザー試験でstub化し、resource取得失敗は明示的に注入した。それ以外のファイル／Wiki／PTYは実APIで検証した。ブラウザー、serverプロセスグループ、子プロセス、listenポートの終了と一時データ削除も確認する。スクリーンショット・trace・result.jsonはGit対象外の `test-results/project-utilities/` に保存する。

## 未検証・対象外

実Codex / Claudeアカウントの残枠、配布Docker image、実Auth0、公衆ネットワーク、Firefox / WebKit、実機モバイル、dark themeの目視、全Rust workspace suiteと負荷試験は実行していない。fixture成功でこれらを保証しない。ホストOS更新・再起動要求の監視は追加していない。mainへのマージ、push、共有環境の再起動・デプロイ、canonical OpenWikiの更新は行っていない。

## LOCAL-297 継続：単一端末と実終了（2026-11-02 UTC）

今回の対象は `654fd68b7d2a1eb7a53f5237a7529f06466356d7` 上の未commit差分。前節の初回297検証とは別Runである。Issue revision `2026-11-02 18:50:29.092` の3.3（ユーザー指定18:49本文と一致）を確認し、LOCAL-291の目的を統合した確定仕様を実装した。291の取消状態は変更していない。

- 両ビューで同じ `TerminalSection` を使い、項目の明示展開だけが単一端末を作る。明示折畳みは所有する接続へ終了要求を送り、シェル・実行中ジョブを停止する。タブ追加、自動接続、再接続、確認、警告、終了専用ボタン、追加ツールチップは設けない。mainのrepo選択は起動前に行え、稼働中は固定する。
- 端末の所有を表示コンポーネントのmount/unmountから分離した。右サイドバー全体、狭幅のファイル/Wiki選択、端末の表示面切替では停止しない。自然exitと検出した通信断は項目を閉じ、次の明示展開まで生成しない。
- Linuxでは既存pidfdの所有確認を再利用する。未reapの自分のシェルをsessionのanchorとして、前景・背景の別job groupと捕捉可能な子孫を終了し、シェルwaitとPTY reader joinを待つ。close成功後だけmapを除去する。別ブラウザーやcwdが同じ別処理を対象にしない。既存archive guardは維持し、archived/deleted Workspaceの新規端末は拒否する。

### 今回実行した検証

| コマンド | 結果 |
| --- | --- |
| `pnpm run web-core:check` | 成功 |
| `pnpm --dir packages/local-web run build > /tmp/lvk-1784-terminal-frontend.log 2>&1` | 成功。既存Browserslist/Tailwind/chunk警告あり |
| 変更したweb-coreの8ファイルを既存local-web設定でESLint（`--max-warnings 0`） | 成功 |
| `pnpm run i18n:check` | 成功。追加文言なし、既存英日ラベルを再利用 |
| `node scripts/dev-cargo.mjs check -p server` | 成功 |
| `node scripts/dev-cargo.mjs clippy -p utils -p local-deployment -p server --all-targets -- -D warnings` | 成功。初回のcollapsible_if指摘を修正 |
| `node scripts/dev-cargo.mjs test -p local-deployment pty::tests --lib -- --test-threads=1` | 実PTYの2件成功（再実行も成功） |
| `node scripts/dev-cargo.mjs test -p utils process::owned_group::tests --lib` | 既存所有プロセス回帰3件成功 |
| `node scripts/dev-cargo.mjs build -p server --bin server -p local-deployment --bin agent-process-host` | 成功 |
| `node tests/server-access/workspace-wiki.mjs test-results/terminal-lifecycle-final` | 隔離DB・repo・実PTY・Chromiumで成功 |

ブラウザー検証では両ビューの空シェル、前景Pythonとsleep子プロセスの停止、シェルのreap、自然exit、接続close、明示再開ごとの接続1個を確認した。全体サイドバー非表示と390pxでのファイル/Wiki操作でも同じPID・接続を維持する。別ブラウザーの同じ対象の端末と同じcwdの独立sleepは存続した。Workspace archiveはそれらが生存中409、全所有者の解放後は通常prepare/confirmで成功し、guardを回避していない。

受信フレーム遮断とDate.nowの進行注入で、両ビューのクライアントwatchdogが無言で折り畳み、再生成しないことを確認した。別途、実時間でPingへ応答しないraw WebSocketを使用し、クライアントcloseなしでサーバーが約50秒後にシェルを停止・reapすることを確認した。物理ネットワーク断そのものを再現した試験ではない。

最後の端末repo選択欄のaria-label整理（既存文言のみ）はproduction build・型検査・対象lintで検証した。同じ動作のE2Eはその直前buildの結果であり、ラベル変更だけの全E2E重複実行は省いた。

初期E2Eは、終了直後のzombieをreap未完了のまま判定したこと、補助検証後のWiki表示状態、同名ボタン・Repository部分一致のlocatorで失敗し、既存ハーネス内を修正した。最初の失敗時には、signal終了済み子プロセスを再度exit待ちする検証cleanupの不備も修正した。その試験の専用ブラウザー・server・一時データを個別に片付けた。成功Runはserver群・ブラウザー・ポート・一時データの解放まで検査する。成果証拠はGit対象外の `test-results/terminal-lifecycle-final/`（約23MB、trace・画面・result）に残す。

### 検出・後片付けの限界

- 受信監視はクライアント45秒＋最大約5秒、サーバー45秒＋最大約10秒のtick待ちが目安。即時検出ではなく、ブラウザー休止、OS停止、スケジューリング、送信待ちで遅れ得る。新しいシェルは自動生成しない。
- 通信断のclient close完了は、到達不能なサーバーのcleanup完了証明ではない。明示終了の通常経路ではサーバーのkill/wait後に終了通知を返し、再開を既存接続のclose後まで待たせる。
- 停止不能なkernel状態、サーバー強制終了、捕捉前にsetsidして再親化したdaemonまで完全に隔離・回収するsandboxではない。cleanup失敗はログへ残し、成功扱いでmapを解放しない。子孫の終了とシェルのreapは確認するが、再親化した子孫のzombie回収はOS側reaperの責任となる。
- 実プロセス木の保証を検証した対象はLinux。macOS/Windowsは未検証で、非Linux fallbackは既存portable-pty child kill/waitの範囲に留まり、Linux同等のsession終了・自然exit監視を保証しない。
- 実Codex/Claude実行への非干渉は独立プロセスと既存provider所有テストで確認した範囲であり、実アカウントの起動試験ではない。実Auth0、実機モバイル、Firefox/WebKit、配布image、全workspace suite、物理回線断・プロキシ経由は未実行。

共有環境の変更、mainへのマージ、サーバー再起動・デプロイ、canonical OpenWiki更新は実施していない。
