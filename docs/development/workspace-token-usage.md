# LOCAL-90: Workspace累積トークン

人間・上位AIが消費の偏りに気づくための、取得済み使用量の軽量ガイド。
料金推計、週次枠の消費率、現在のコンテキスト占有量ではない。

## 変更

- チャット末尾に「Workspace累積トークン」を表示し、詳細で入力・キャッシュ入力・出力・推論、取得範囲と最終更新を確認できる。
- LOCAL-29の既存の各run時間表示は維持。Workspace累積時間は追加しない。
- `GET /api/workspaces/{id}/tokens`、外部MCP `get_workspace.token_usage`、stdio MCP `get_workspace_tokens` は同じDB集計を読む。読取によるAgent起動はない。
- React Queryの5秒間隔の再取得で、開いているチャットの使用量を更新する。
- 既存のusage snapshot/host journal経路に、検証済みの累積スコープ情報を追加。新規 `workspace_token_usage` テーブルを、既存snapshot・host cursorと同じトランザクションで更新する。
- Workspace/provider/threadで最大報告値を保持。同じthreadのresume、複数run、再送、再接続を足し重ねない。別threadは加算する。失敗・キャンセルによって報告済み値を消さない。

## 取得できる範囲と制約

初版で集計するのは、既存Codex adapterが受け取る native `thread/tokenUsage/updated` の `tokenUsage.total`。
`inputTokens + outputTokens = totalTokens` を確認し、`cachedInputTokens` は入力の内数、`reasoningOutputTokens` は出力の内数として扱う。`last.totalTokens` は既存context表示用で、この集計には使わない。
Codexのcontext-full用合成値のように総量と入力/出力が整合しない通知は使用量の証拠とせず、集計対象外にする。

thread識別子・総量のない旧snapshotや、差分/累積の意味を確認できない汎用usage通知は集計しない。
過去の全履歴再走査や推計はしない。最初の新規通知が既存threadの過去分を含むことはあり、それはproviderが実際に返したthread累積値である。
親threadの既存投影経路を使い、子threadを独立に走査・推定・加算しない。providerの親累積値に含まれない子消費は不明。
同一threadでproviderが累積カウンターをリセットした場合、最大既報値を保持するが、リセット後の差分を推定しない。

未取得の総量・内訳はnullで、計測された0とは区別する。
一つでも有効な報告があれば `partial`、なければ `unknown`。
最終通知が全消費を網羅したかは証明できないので `complete` は返さない。
内訳は取得できた値のみの和で、キャッシュ・推論には内訳取得thread数を併記する。
欠損した後続通知によって既報の内訳を捨てない。この場合も全量保証はしない。

providerとthreadは保存し、APIにはprovider一覧と既存run attemptに保存された要求モデル一覧を返す。
要求モデルは実際に処理したモデルの証明ではなく、thread内のモデル切替ごとの消費配分も不明。モデル別の料金換算は行わない。
計測のための生プロンプト追加保存はない。

## 正常resumeの根拠（Codex rust-v0.154.0）

[session/mod.rs](https://raw.githubusercontent.com/openai/codex/refs/tags/rust-v0.154.0/codex-rs/core/src/session/mod.rs) のresumed分岐は `last_token_info_from_rollout` → `state.set_token_info` で既報情報を復元する。
その後の [state/session.rs](https://github.com/openai/codex/blob/rust-v0.154.0/codex-rs/core/src/state/session.rs) の `update_token_info_from_usage` → `history.update_token_info`、
[context_manager/history.rs](https://github.com/openai/codex/blob/rust-v0.154.0/codex-rs/core/src/context_manager/history.rs) の `update_token_info` → `TokenUsageInfo::new_or_append` を経て、
[protocol.rs](https://raw.githubusercontent.com/openai/codex/refs/tags/rust-v0.154.0/codex-rs/protocol/src/protocol.rs) は既存infoをcloneし、`append_last_usage` → `total_token_usage.add_assign(last)` で加算する。
この版の正常resumeは既報totalを復元後に加算する設計。実アカウントの精度試験を意味せず、異常resetや未報告分など上記の制約は残る。

## 検証記録

開始コミット: `9f16bce8b189e00aeb2ef8fc90b8d129e85ab1fd`。
実装コミット・実行結果は完了時の報告を参照。

隔離ブラウザ試験は `node tests/server/run-workspace-tokens.mjs`。
このrunnerは変更ソースでビルドした `target/debug/server` と `target/debug/vibe-kanban-mcp`、実frontend distを使用する。
`.build/tokens-instance-*` の使い捨てDBと動的localhostポートのみを使い、終了時に自分のプロセスを停止してDBを削除する。
通常起動中の開発版/安定版、他Workspace、共有DBを変更しない。
ブラウザfixtureは保存済み計測行を投入し、provider通知からの保存・重複排除はRust fixtureで別途検証する。
有料モデルrun、実アカウントによるusage正確性検証は行わない。独立レビューは上位AIの担当。

### 2026-10-21 実行結果

Rustコマンドは `RUSTC_WRAPPER=/usr/local/bin/sccache` と、このWorkspaceの
`.lvk-shared/cache/sccache` の正規パスを `SCCACHE_DIR` に指定して逐次実行した。
Cargo targetはこのWorkspaceの通常の `target/`。既存sccacheサーバーのcache場所も一致を確認。

| 実行コマンド | 結果 |
| --- | --- |
| `pnpm install --frozen-lockfile` | 成功、lockfile変更なし |
| `cargo run --bin generate_types` | 型・schema生成成功 |
| `target/debug/generate_types --check` | 成功、生成型一致 |
| `cargo test -p db workspace_tokens` | 2件成功 |
| `cargo test -p executors -p db -p server -p local-deployment -p mcp --lib workspace_tokens` | 最終ソースで5件成功。MCP crateはこのfilterでは0件 |
| `target/debug/deps/executors-b52f8ddd54adf950 executors::provider_adapter::tests` | 同じCargo検証で作成されたtest binaryを実行、39件成功 |
| `target/debug/deps/db-aa4176b675509be8 usage_is_upserted_as_latest_snapshot_without_canonical_history` | 既存snapshot回帰1件成功 |
| `target/debug/deps/mcp-9d803b30b982aa21` | stdio codec・scope・tool一覧など10件成功 |
| `pnpm run web-core:check` | 成功 |
| `pnpm run ui:check` | 成功 |
| `pnpm run local-web:lint` | 成功 |
| `pnpm --dir packages/local-web run build` | TypeScriptと実Viteビルド成功 |
| `cargo build --bin server --bin vibe-kanban-mcp --bin generate_types` | 成功、E2Eはこの変更ソースのバイナリを使用 |
| `cargo clippy -p executors -p db -p local-deployment -p server -p mcp --all-targets -- -D warnings` | 成功 |
| `cargo install sqlx-cli --version 0.8.6 --locked --no-default-features --features sqlite --debug --root "$PWD/.build/sqlx-cli"` | 成功。`CARGO_TARGET_DIR="$PWD/target"` で既存targetを再利用 |
| `pnpm run prepare-db` | 使い捨てDBでmigration・metadata生成成功 |
| `pnpm run prepare-db:check` | 成功。上記CLIをPATHに追加して実行 |
| `node tests/server/run-workspace-tokens.mjs` | Playwright + Chromiumで通常起動1件、同じ隔離DBでのサーバー再起動後1件が成功。最終再実行も両方成功 |
| `git diff --check` | 成功 |

SQLx生成は既存の未使用metadata14件を削除したが、LOCAL-90と無関係なので元どおり保持した。
その状態で再度 `prepare-db:check` が成功し、既存未使用クエリのwarningのみ残る。
Viteには既存依存データの古さ、Tailwind content、chunk sizeのwarningがあるがビルドは成功。
これらの別件修正は含めない。

E2Eで確認した値は、unknown → total 120 → total 200（input 165、output 35、cached 60、reasoning 10）。
内訳欠損threadを含む1/2の取得範囲、partial、ページ再読込、日本語ラベル、390px幅、
stdio MCPとAPI集計JSONの完全一致を確認した。試験fixtureはAgentRunを起動せず保存行を投入する。
通知処理・複数Session/run・同一thread resume・通知重複・失敗/キャンセル保持はRust fixtureで確認した。
最終E2Eのserverと子プロセスは停止し、各 `.build/tokens-instance-*` は削除済み。

### 未確認範囲・レビュー引継ぎ

- 実アカウント／有料モデルのusage精度、実Auth0経由の外部MCP接続は未試験。
- 全providerの計測、native子threadの独立計測、thread内のモデル別消費配分、カウンターリセット後の正確な差分は対象外／不明。
- 全backend suite、全workspace lint/check、正式Integration、Tauriは未実施。変更境界を対象に上記の検証を選択した。
- 独立レビューは上位AIが実施する。自己検証を独立レビュー完了とは扱わない。
- マージ、デプロイ、稼働開発版/安定版の再起動、本番DB変更、OpenWiki更新は実施していない。
- 再検証用に通常の `target/`、`node_modules/`、自Workspaceのfrontend `dist/`、ローカルSQLx CLIを保持。
  `.build/` の検証ログと `test-results/server/` の画面証拠も保持する。他Workspaceのファイルは削除していない。

### 2026-10-21 独立レビュー後の最小修正

基点は `2100743c59e9ec9cbc4a753313b0e771de2f0eed`。上位AIのレビュー判断に沿い、最新snapshotに依存して後退し得る報告試行数を削除した。
`observed_attempt_count` と表示専用の分母 `attempt_count` を共通集計型・API/MCP応答・生成型から除去し、UI coverage行と日英キーも削除。
everflagや履歴走査は追加せず、unknown/partial、thread数、キャッシュ/推論の内訳取得thread数を維持した。
日英のUI説明はCodex親threadの報告値であり、親に含まれない子thread使用量は不明と明記した。

変更対象はDB集計/fixture、生成TypeScript型、チャット末尾コンポーネント、日英翻訳、既存Chromium spec、本記録。
以下は今回の変更後ソースで逐次実行した結果（上記の初回実装検証とは別）。

| 実行コマンド | 結果 |
| --- | --- |
| `cargo test -p db -p server --lib workspace_tokens` | 3件成功。既存fixtureにvalid 42→cumulative_usage=None後の総量・内訳・partial・thread数保持を追加。外部MCPの共通集計とproject scopeも確認 |
| `cargo build --bin server --bin vibe-kanban-mcp --bin generate_types` | 成功。このバイナリでE2Eを実行 |
| `target/debug/generate_types` / `target/debug/generate_types --check` | 生成・一致確認とも成功 |
| `pnpm run web-core:check` | 成功 |
| `pnpm run local-web:lint` | 成功 |
| `pnpm --dir packages/local-web run build` | TypeScript・Vite成功、既存warningのみ |
| `node tests/server/run-workspace-tokens.mjs` | Chromium通常起動1件、隔離サーバー再起動後1件成功。unknown→120→200、partial、内訳1/2、APIとstdio MCP一致、試行数フィールド不在、日英の親/子範囲説明を確認 |
| `pnpm run prepare-db` / `pnpm run prepare-db:check` | 成功。既存未使用metadata14件の生成時削除は復元し、check成功（既存unused warningのみ） |
| `git diff --check` | 成功 |

Rustは前回同様sccache・自Workspace targetを使用。SQLx CLIは既存 `.build/sqlx-cli/bin` をPATHへ追加した。
代表の詳細画面と390px画面を確認。E2E終了後に隔離サーバー停止・一時状態削除まで完了した。
今回のログは `.build/review-*.log`、画面証拠は `test-results/server/` に保持する。
実アカウント精度、有料モデル、全suite、正式Integration、別providerの追加計測は実施していない。
マージ・Wiki更新・deploy・稼働版restart・live DB変更・自Workspace外cleanupも行っていない。
