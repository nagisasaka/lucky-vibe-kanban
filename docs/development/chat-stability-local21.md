# LOCAL-21 チャット画面の回帰検証

この変更は最新追従・履歴追加・送信済み入力・steering 入力表示を対象にする。
Kanban の状態遷移／名称、Run の状態・通知・永続イベント生成は変更しない。

## 原因と変更

| 対象 | 確認した原因 | 修正 |
| --- | --- | --- |
| 最新追従・切替 | 非仮想化の末尾が伸びても行数と仮想領域サイズが変わらない場合、補正が発火しない。rAF の最後の通知が initial を上書きでき、遅い初回通知は逆に再ジャンプを起こせる。 | 内容領域の ResizeObserver、初回の非空表示だけのジャンプ、Workspace/Session をキーにしたリスト状態の分離。 |
| 過去履歴 | preserve-anchor は実質 no-op で、既存行のサイズ補正だけでは prepend の挿入量を相殺できない。末尾境界も prepend 前の数値添字だった。 | 同じ semantic key の行と画面上の位置を保持。アンカー補正中は仮想リストとの二重補正を避け、利用者の操作で解放する。末尾境界も行 identity で移動する。最新2 Run＋活動中 Run の初期購読は維持。 |
| 送信済み入力 | 制御値の Markdown 読込が末尾改行を正規化し、onChange で親に戻ると、送信待ち中の新規編集として revision が変わる。 | 外部値の読込を Lexical update tag で識別し、ユーザー編集として返さない。同値入力は revision を増やさず、キー送信は flush 後の callback を使う。送信ボタンの制約を保ち、本文は送信待ち中も編集可能。 |
| steering 表示 | backend は既に command ID を使って canonical user event を保存するが、表示順が native audit sequence と canonical sequence を混用し、steering を古い応答より前に配置する。 | 永続イベントは canonical sequence 順。live delta は独立の番号体系として配置し、永続順を壊さない。仮のユーザーメッセージや新しい保存経路は追加しない。 |

## 再実行するコマンド

リポジトリルートで実行する。共有サービスや共有DBを使わない。

```bash
pnpm install --frozen-lockfile
pnpm run chat:test
pnpm --dir packages/web-core exec vitest run src/features/agent-runtime/model/canonicalAgentConversation.test.ts src/features/agent-runtime/model/canonicalAgentTimeline.test.ts src/features/workspace-chat/model/sessionDraft.test.ts
pnpm run web-core:check
pnpm run ui:check
pnpm run local-web:check
pnpm run ui:lint
pnpm run local-web:lint
pnpm --dir packages/local-web run build
# AGENTS.md の sccache 設定を適用し、チェックアウト固有の target を使う。
cargo build --locked --bin server --bin agent-process-host
python3 tests/chat/run-isolated.py
```

`run-isolated.py` はビルド済みバイナリを別 loopback ポートで起動する。
実行ごとに `test-results/local21/e2e-<uuid>/instance/` を作り、DB・data・runtime・tmp を分離する。
サーバー停止とポート閉鎖確認を finally で行い、証拠は同じ実行ディレクトリに保持する。
実 Codex を起動するテストを含むため、認証済み環境での明示的な受入検証用である。
稼働中 dev、nginx、公開ポートの設定は変更しない。

## 証拠の区分

- `chat:test` は実 Chromium と本番 React hook / editor / ConversationList を使う隔離コンポーネント試験。通信は合成であり、製品全体や provider の受入証拠とは区別する。
- `isolated.spec.ts` の履歴試験はビルド済み製品UI・実backendの Workspace/Session を使い、AgentRun一覧とWebSocketイベントを合成する。最新2件、遅延した履歴、prepend座標、最新追従、過去閲覧、Workspace切替を確認する。
- 同ファイルの入力試験は実backendの Scratch 保存を使い、follow-up 応答だけを成功・遅延・失敗に制御する。末尾改行、追加入力、再読込、Enter/Ctrl+Enter、IMEを確認する。
- 実 Codex 試験は専用の空ディレクトリで限定的な sleep/reply を実行し、UIからsteeringを送る。実backendのイベント履歴、再接続、再読込で user input が同じ Session/Run に一度だけ存在することを確認する。

## 実行結果

2026-10-20 の最終結果:

- `chat:test`: Chromium 3試験合格。旧socket／遅延callbackの分離、rAFで合流した初期通知、遅い初期通知、連続prepend、末尾領域の伸長、入力成功／失敗／追加入力、IMEを含む。
- 上記 Vitest: 3ファイル・22テスト合格。canonical replay重複除去、steering順序／live表示、Session/Runのidentity、送信snapshotを確認。
- `web-core:check`、`ui:check`、`local-web:check`、`ui:lint`、`local-web:lint` 合格。変更ファイルのPrettier確認、`git diff --check` 合格。
- `pnpm --dir packages/local-web run build` 合格。`cargo build --locked --bin server --bin agent-process-host` 合格（初回7分09秒、既定dev profile）。sccacheは指定共有cacheを利用し、統計はリセットしていない。
- `python3 tests/chat/run-isolated.py`: **3/3合格、約1.4分**。最終インスタンスは `http://127.0.0.1:39791`、実行ID `5bcfb116-b536-45d9-a527-ee59b047434b`。server終了コード0、ポート閉鎖、当該instanceのagent process registryが空であることを確認した。
- sourceはbase commit `60aa28bd0148714582824cd0825114aef6efaf46` にこの変更を適用したcheckout。`test-results/local21/build.json` にbuildコマンド、変更packageファイルのSHA-256、frontend indexのSHA-256を記録し、テスト後に作業中sourceとの一致を確認した。最終source commitも保存証拠のbuild.jsonに記録する。

最終E2Eの証拠は `test-results/local21/e2e-5bcfb116-b536-45d9-a527-ee59b047434b/` の `e2e.json`、`cleanup.json`、`server.log` と4枚の成功スクリーンショットにある。レポート・ビルド情報・画像はrepository共有領域の `persistent/validation/local21-69ce54b1/` にも保持する。DB・native auditは隔離instanceのdataに残し、使い捨てcompile cacheだけを削除した。通常の `target/`（約12GB）と依存パッケージは再利用のため維持する。

実Codexの成功証拠:

- Workspace: `6bb729ec-2882-476e-9c7c-47498afddd2d`
- Session: `83521694-e478-42ee-951b-8b0a8d91e72c`
- Run: `7743244a-2c91-4558-b5e7-2d662f2e1fcf`
- steering user event: `04e61f2b-bac3-428f-ab43-638fb95e1a5a`

初回のE2E準備では設定のmodule形式／CSRFヘッダー不足があり、修正した。次の実行では初回WelcomeガイドがWorkspace切替を遮ったため、専用instanceの既読設定を明示した。また、steeringの入力本文はMarkdownのエスケープを含むため、永続化検査は画面のplain textではなく実際にPOSTした本文と比較するようにした。これらの失敗時trace・レポートも隔離証拠ディレクトリに残る。

未検証範囲: 履歴の長さと到着タイミング、通常follow-upの成功／失敗は合成制御であり、全providerや実運用データの長期負荷試験ではない。実providerを通したのは上記Codexのsteering経路。Rust sourceの変更はなく、Rust unit test全体は実行していない。共有dev、他Workspace、公開環境には接続・デプロイしていない（共有資源登録の読取確認を除く）。

修正前のソースに追加テストを適用した比較では、末尾領域の追従待ちがタイムアウトし、prepend の同じ行が -80px から 1912px へ移動した。steering の並びは `before, steer, after` に対して `steer, before, after` になった。末尾改行の外部値読込と送信を重ねる入力試験でも、送信成功後に入力が消えずタイムアウトした。比較後には作業中ソースを復元している。
