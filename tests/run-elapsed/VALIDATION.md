2026-10-20 / Workspace `e72dc0bb-48a7-4328-88de-48c4cceefbc2`

**変更と根拠**

- ボードの完了時刻は `state.updated_at` を参照していた。この値は終了後のイベントでも更新される。`crates/db/src/models/agent_runtime.rs` と `crates/server/src/routes/workspaces/workspace_summary.rs` で、既存attemptの固定済み `finished_at` とrunの `created_at` を返す。
- `crates/services/src/services/agent_runtime.rs` は同じ終了時刻をSessionのrun一覧に返す。スキーマ変更・独自execution stateは追加していない。
- `packages/ui/src/components/IssueWorkspaceCard.tsx` は三点表示の右に経過時間、終了後は固定時間を表示する。
- `packages/web-core/src/features/agent-runtime/model/canonicalAgentConversation.ts` と `ConversationListContainer.tsx` は通常/Goal共通で各runの末尾に時間を表示する。再接続時の時刻判定は既存のcanonical revision比較を再利用する。
- `packages/ui/src/components/ChatElapsedTime.tsx` は終了時刻をactiveフラグより優先し、終了後のintervalを作らない。終了時刻不明の過去runにブラウザの現在時刻を補わない。
- Goal固有のprovider報告累積値は既存実装・定義を維持した。run経過は待機・再試行を含む開始から終了までの壁時計時間で、別run間を合算しない。追加の仕様判断を要する矛盾は確認していない。

**試験コマンドと結果**

| コマンド | 最終結果 |
| --- | --- |
| `cargo run --bin generate_types` | 成功。生成差分は追加の時刻フィールドのみ |
| `target/debug/generate_types --check` | 成功 |
| `pnpm run web-core:check` | 成功 |
| `pnpm run local-web:check` | 成功 |
| `pnpm run ui:check` | 成功 |
| `pnpm run local-web:lint` | 成功 |
| `pnpm run ui:lint` | 成功 |
| `pnpm --dir packages/web-core exec vitest run src/features/agent-runtime/model src/features/workspace-chat/model/goalElapsed.test.ts src/features/workspace-chat/ui/GoalProgressCard.test.tsx src/shared/hooks/useWorkspaces.test.ts --maxWorkers=1` | 10ファイル・62件成功 |
| `PLAYWRIGHT_BROWSERS_PATH=/opt/lvk-browsers pnpm run chat:test` | Chromiumの3件成功（会話同期、既存表示/エディタ、新規時間表示） |
| `cargo test -p db unseen_workspace_tracks_unacknowledged_terminal_transition_only` | 対象1件成功。終了後イベントで更新時刻だけが進んでも終了時刻を保持 |
| `cargo clippy -p db -p services -p server --lib -- -D warnings` | 成功 |
| `pnpm --dir packages/local-web run build` | 成功。最終表示位置の調整後にも再ビルド |
| `cargo build --locked --bin server --bin agent-process-host` | 成功。このWorkspaceのソースを使用 |
| `PLAYWRIGHT_BROWSERS_PATH=/opt/lvk-browsers node tests/run-elapsed/isolated.mjs` | 成功。下記の実backend範囲を確認 |
| `pnpm run prepare-db:check` | 失敗：`cargo sqlx`未導入。SQLxメタデータ検査には到達していない |

Rustは既定のWorkspace専用targetと既存の共有sccacheを使用した。SQL変更は動的クエリで、DB回帰試験と隔離アプリでは実SQLiteに対して実行した。これは未実施のSQLxメタデータ検査を成功とするものではない。

**E2Eの再現・期待・観測**

[README.md](README.md) のコマンドで再現できる。テストがOS割当のloopbackポート、専用 `LVK_INSTANCE_DIR`、専用GitリポジトリとDB/data/runtimeを作成する。実APIでProject/Issue/Workspace/Repo/Sessionを作り、providerに相当するcanonical run/eventのみfixtureとして専用SQLiteへ投入する。HTTPレスポンスのmockはない。

| 操作 | 期待 | 観測 |
| --- | --- | --- |
| 2 runのチャットを開く | 過去runは固定、現在runだけ進む | 過去12s、現在1m 3s。現在値が進むことをassert |
| 実行中のボードを開く | オレンジ三点の右で時間が進む | 1m 6s。値の進行をassertし画像で三点との配置を確認 |
| 遅れて到着する終了記録を反映 | 現在runがcanonical終了時刻で止まる | 42sへ確定し、待機後も42s |
| チャットを再読込する | 加算・再始動しない | 過去12s、現在42sを維持 |
| Goalの終了表示を見る | provider報告累積値を維持する | Saved Goal / Recorded time: 0m 7s |
| 完了後のボードを開く | チャットと同じ固定時間 | 42s。待機後も42s |

最終E2Eではブラウザのpage errorとHTTP 5xxは0件。初回fixtureのリポジトリ未設定によるGit APIエラーはfixtureを修正して解消した。時間フッターは右側操作UIと重ならない位置へ調整し、最終画像を目視確認した。

API/SQLite/WebSocket/ビルド済みアプリ/Chromiumは実物。providerによる実際のモデル実行・開始停止操作は試していない。通常/Goal、待機、cancelling、成功・失敗・中断・crash・audit failure、再マウント、複数runの網羅はcanonical fixtureを用いた単体/コンポーネント回帰試験で確認した。

**残した証拠と後片付け**

`.build/run-elapsed-validation/` に `evidence.json`、ボード/チャットの実行中・完了後スクリーンショット、各コマンドのログを保持する。検証用サーバー/ブラウザは停止を待ち、専用instance（DB/data/runtime/Git fixture）は削除済み。通常の `target/`（約20GB）と依存（約982MB）は次回検証で再利用するため保持した。
