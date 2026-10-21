# LOCAL-38 Supervised 承認配線 Change Manifest

対象ベース: `60aa28bd0148714582824cd0825114aef6efaf46`。
Workspace: `c167123c-80fb-44a9-a73e-6d92b9e96606`。

## 修正の目的と境界

process host が注入する Noop により、Supervised の command/file 要求まで
回答を待たず承認されていた。さらに Codex の AgentRun 承認制御は未接続で、
既存 UI が必要とする canonical 承認イベントと待機状態も配線されていなかった。

- host 内で既存 `Approvals` / `ExecutorApprovalBridge` を再利用する。
  DB を host から開かず、host が生存する限り承認 waiter を保持する。
- 既存 AgentRun API → host 制御 → client → bridge の回答経路を接続する。
  元の request handler が native request ID を保持し、応答する。
- client の canonical 承認要求・結果・待機数を既存 event stream へ流す。
  複数要求がある場合、最後の回答まで `awaiting_approval` を維持する。
- 不明・重複・期限切れ ID を承認しない。サービス不在・異常・取消では
  accept を返さない。終了時に cancellation token で待機を解放する。
- native 要求と元 ID / decision を監査ストリームに保持する。
  API 側での重複 `ApprovalResolved` 追加は行わない。
- DangerFullAccess と approval policy 未設定 / Never の全許可を維持する。
  Supervised は Never を UnlessTrusted に置き換えるため全許可分岐に入らない。
- 一般質問・Plan の旧契約は wrapper で維持する。native sandbox / reviewer
  の権限を緩和せず、UI・DB schema・一般質問・Plan の全面改修は行わない。

## 再現方法

旧 Noop を実 client の `request_tool_approval` に注入すると、
`auto_approve=false` でも command/file の両方で直ちに Approved になる。
さらに別 Python プロセスの app-server protocol fixture から実 JSON-RPC
client に要求を送り、旧 Noop では human response なしに accept が出ることを
確認する。同じ fixture を使い、修正後の待機・回答・取消を比較する。

これはモデルによる危険操作判断の再現ではない。fixture は command/file を
実行せず、要求と応答のみ送受信する。native sandbox / 実モデルの統合試験は
本記録の対象外。

## 実行記録

Cargo は workspace 固有 `target/`、`CARGO_BUILD_JOBS=2`、
`RUSTC_WRAPPER=/usr/local/bin/sccache` と canonical shared cache を使用。
sccache の実サーバーの cache location 一致を確認し、設定変更・reset はしていない。
重い処理はこの作業内で直列実行。

| コマンド | 結果 |
| --- | --- |
| `cargo test -p executors legacy_noop_injection -- --nocapture` | 1 passed。command/file とも人間の回答なしに Approved。 |
| `cargo test -p executors executors::codex -- --nocapture`（初回） | 121 passed / 1 failed / 6 ignored。新 fixture が turn/started を欠き、command/accept の終了待ちがタイムアウト。通知順を修正。 |
| `cargo test -p executors command_file_protocol -- --nocapture` | 1 passed、内部14ケース。accept/reject/feedback/cancel/missing/full-access/legacy-noop × command/file。 |
| `cargo test -p local-deployment process_host -- --nocapture` | 15 passed。既存 host journal / transport / cancellation と追加 bridge wrapper の試験。 |
| `pnpm install --frozen-lockfile` | passed。 |
| `cargo build --bin server --bin agent-process-host` | passed（7m11s）。ブラウザは Vite を使用し、build script の仮 dist は使わない。 |
| `python3 tests/supervised/run_isolated.py`（初回） | failed: 試験の `/api/config` 更新が CSRF header を欠いたため403。アプリと同じ `/api/access/status` 取得と header 付与へ修正。cleanup: remaining_pids / remaining_listeners とも空。 |
| `pnpm exec prettier --write tests/supervised/*.ts` | failed: root に prettier command なし。 |
| `pnpm --dir packages/local-web exec prettier --write ../../tests/supervised/approval.spec.ts ../../tests/supervised/playwright.config.ts` | passed。既存 package の formatter を使用。 |

- ブラウザ2回目: fixture が起動時 `config/read` に未対応で、承認要求まで到達せず失敗。fixture に空の config/origins/layers 応答を追加。cleanup 残存なし。
- ブラウザ3回目: 環境継承の `VIBE_KANBAN_AGENT_PROCESS_HOST` が他 Workspace の host バイナリを指していた。未回答 accept を観測したが、修正後検証として無効。runner から LVK_/VK_/VIBE_KANBAN_ の継承を除外し、この checkout の `target/debug/agent-process-host` に明示固定。DB・設定・ポートは一時 instance 内で、cleanup 残存なし。

- ブラウザ4回目: 正しい Workspace server / host で9ケース passed（2.4m）。command/file の accept・deny・cancel・full-access と2件並行承認。再読み込み前後の応答なし、元 ID への応答、終端後の遅延回答で追加応答なしを確認。
- `cargo test -p executors` 最終確認の初回: 315 passed / 1 failed / 6 ignored。ベースでも Codex mapper は v5 なのに既存試験が v4 を期待していた。試験だけを v5 期待へ修正し、旧 v4 mapper の version 維持も追加確認。製品 version は変更なし。

- `cargo test -p executors` 再実行: 316 passed / 0 failed / 6 ignored。別途 doctest 1 ignored。ignored は既存 fixture helper / doctest であり、成功扱いにはしていない。
- `cargo test -p local-deployment`: 78 passed / 0 failed。host、journal、制御、監査 launch 入力を含む。
- `cargo test -p services services::approvals`: 2 passed / 0 failed。timeout / early answer / cancellation-vs-ready-response を確認。

追加の最終検証は後段に記載。

## 隔離と後片付け

共有資源 snapshot は登録資源なし。稼働 dev / 安定版の DB・設定は試験に使わない。
ブラウザ runner は専用 `LVK_INSTANCE_DIR`、一時 SQLite DB、専用 loopback ports、
一時 `CODEX_HOME` を作成する。HOME は変更しない。
fixture trace は起動・turn パラメーターを記録せず、内部 MCP credential や
prompt の持ち出しを避ける。finally で自己所有のプロセス・ポートの解放を確認する。

マージ、デプロイ、稼働サーバー再起動、全 main の取り込みは行わない。

## 追加の最終検証

- `cargo clippy -p executors -p services -p local-deployment --all-targets -- -D warnings`: passed（6m41s）。
- `cargo test -p executors command_file_protocol -- --nocapture` 再実行: passed。サービス異常も加えた16ケースで、missing/broken/cancel は応答0、accept/reject は元IDへの対応を確認。
- 承認イベントの画面投影が既存 tool の command / arguments を上書きする問題も最小修正。既存 tool の詳細・identity を維持し承認状態だけを更新する。
- `pnpm --dir packages/web-core exec vitest run src/features/agent-runtime/model/canonicalAgentConversation.test.ts --maxWorkers=1`: 13 passed。
- `pnpm run web-core:check`、`pnpm run local-web:check`: ともに passed。web-core には standalone lint script なし。変更した2ファイルには package の Prettier を適用。

- `cargo build --bin server --bin agent-process-host` 最終 source 確認: passed（33.27s）。
- main は調査中に `9cbb4979` へ進んだ。変更対象と重なる差分は別位置の既存テスト追加のみで、本修正の依存ではない。main の取り込みなし。

- ブラウザ5回目（`.build/supervised-approval/e2e-1792554130232712601`）: 9ケース passed（2.3m）。各 run の audit integrity=complete、native request/response の元IDとdecision、canonical承認イベントのnative_ref/checksumを照合。cleanup: remaining_pids / remaining_listeners は空。
- 同実行のスクリーンショットで、待機 command が集約表示に隠れることと、file 承認本文が見出しだけになることを発見。既存承認カードへ command / arguments を表示し、pending_approval は集約から除外する修正を追加。
- `pnpm --dir packages/web-core exec vitest run src/features/agent-runtime/model/canonicalAgentConversation.test.ts src/shared/lib/aggregateEntries.test.ts --maxWorkers=1`: 22 passed。詳細保持と承認待ち非集約を確認。

- 表示修正後の `pnpm run web-core:check` / `pnpm run local-web:check`: ともに passed。
- sccache 終了時共有統計: hits 2917、misses 3324、Rust hit rate 63.38%。共有サーバーの累積値であり本作業単独の効果ではない。cache errors (Assembler) 2 も記録したが、今回の Cargo コマンドは最終的にすべて成功。cache location は変更していない。

## 検証範囲と未検証

- 実行した層: この Workspace の Rust server / process host / provider adapter / JSON-RPC client、SQLite canonical event store / native audit、既存 API / WebSocket / React UI、Chromium。
- fixture の層: Codex app-server と model の代わりに Python protocol peer を起動。実コマンド・実ファイル変更・モデル通信は行わない。native sandbox / reviewer による判断、実モデルの要求発生、Windows native 実行、サーバー再起動をまたぐ回復は未検証。
- ブラウザ表示は 600×900 の light theme。全 viewport / theme の見た目や通知配送は検証していない。
- services は承認関連の2件に限定し、server 全体・desktop・正式 Integration の包括ゲートは実行しない。本件の通常 Workspace commit の後に独立レビューを行う予定。
- 通常の `target/`（約31GB）と `node_modules/`（約982MB）は追加レビュー・試験のため保持。個別コマンドと初期失敗のログは `.build/supervised-approval/` に保持し、Gitには含めない。


## 最終結果

`python3 tests/supervised/run_isolated.py` 6回目は9ケースすべて passed。
最終 evidence は `.build/supervised-approval/e2e-1792554356364913570/`。
`pending-command.png` / `pending-file.png` を目視確認し、承認前の command、
変更対象pathとdiffが既存承認カードに表示されることを確認。
再読み込み・待機中にnative応答なし、回答後の元ID対応、並行承認の残件維持、
拒否・取消・全許可、監査 integrity / native_ref checksum を確認した。

cleanup は自己所有4プロセス、専用ポート40521 / 52329 / 35865を対象とし、
remaining_pids=[] / remaining_listeners=[]。runner終了後もPID消滅とport未接続を
独立確認。一時DB・CODEX_HOMEは削除済み。単体試験用stateとbuild scriptが作った
仮distも削除し、正常ビルドのtarget/dependenciesと試験証拠のみ保持した。
`git diff --check` は passed。通常Workspace commitとして保存し、マージ・デプロイなし。
