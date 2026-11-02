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
