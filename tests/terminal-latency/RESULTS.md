# terminal → 次操作の遅延検証

対象TODO: `a530ff5c-1fd4-4d1b-bc71-e3efa071ae9a`。測定日: **2026-10-20 UTC**。

## 原因と変更

`crates/local-deployment/src/process_host.rs::monitor_process` が、終了後も
Codex control → AppServerClient → LogWriter → stdout pipe の参照を保持していた。
その参照を保持したままEOFを待つため、出力drainの5秒上限まで待っていた。
既存のプロセスcleanupが完了した後、drainの直前で `drop(control)` する。
監査tailの保存、監査close、永続terminal、canonical投影、UI通知の順序は維持する。
排他・権限・ownership・未完了処理の判定、スケジューラ、UI状態遷移は変更しない。

修正前の実pipe回帰試験は20:08:16.237979853 → 20:08:21.245018912、
5006msで失敗。修正後は20:08:49.894189918 → 20:08:49.898685471、4ms。
非ゼロ終了も4ms。出力tailが監査に残り、監査がcompleteであることを確認した。
provider完了証拠のないexit 0を成功扱いにせずcrashed、exit 1をfailedに保つ。

## 測定条件

同一コンテナ・同一workspace・同一frontend・同一fixture、Chromium
147.0.7727.15、Playwright 1 worker、各ケース各版1回。
変更前後とも自身のソースからserverとagent-process-hostをビルド。
前版は追加回帰試験を含むソースから上記dropとコメントのみを外してビルドし、
後版はそれを復元して再ビルドした。base commitは
`60aa28bd0148714582824cd0825114aef6efaf46`。
前版loopbackポートは39799、後版は35151。双方とも専用の新規DB/data/runtimeを使い、
外部公開・live dev/nginx・他workspaceへの変更なし。終了時に自身のinstanceの
プロセス停止を確認して一時DB/runtimeを削除した。

実backend範囲: Codex adapter、process-host、process cleanup、Native Audit、
SQLite/canonical、HTTP/WebSocket、React、Chromium。
**mock範囲は外部Codex app-serverプロセスのJSON-RPCのみ**。
実Codex CLI/cloud/model呼出し、外部ツール実行は行っていない。
接続再試行は実WebSocketフレームを転送し、切断だけを注入した。

開始はfixtureのterminal送出直前、終了はDOM MutationObserverによるSend有効化。
reloadのみ再navigation後の観測時刻であり、読み込み時間を含む。
同一ホストのUTC時計で計測し、unit試験の経過時間はmonotonic時計を使用。
HTTPのcanonicalAtはpollで観測した時刻で、DB commit時刻ではない。

## 前後の時刻と差分

すべて2026-10-20 UTC、時刻は時:分:秒。差分は後−前。

| ケース | 前 開始→Send有効 | 前 ms | 後 開始→Send有効 | 後 ms | 差分 ms |
| --- | --- | ---: | --- | ---: | ---: |
| completed | 20:05:44.600 → 20:05:53.656 | 9056 | 20:12:07.606 → 20:12:11.656 | 4050 | -5006 |
| failed | 20:06:08.581 → 20:06:17.620 | 9039 | 20:12:20.523 → 20:12:24.572 | 4049 | -4990 |
| cancelled | 20:06:32.551 → 20:06:41.611 | 9060 | 20:12:34.294 → 20:12:38.362 | 4068 | -4992 |
| reconnect | 20:06:54.661 → 20:07:03.704 | 9043 | 20:12:46.395 → 20:12:50.437 | 4042 | -5001 |
| reload | 20:07:18.699 → 20:07:30.167 | 11468 | 20:12:59.330 → 20:13:05.766 | 6436 | -5032 |

正常終了の段階別時刻（UTC）:

| 段階 | 前 | 後 |
| --- | --- | --- |
| provider送出 | 20:05:44.600Z | 20:12:07.606Z |
| native frame監査時刻 | 20:05:44.604587932Z | 20:12:07.610837380Z |
| audit close | 20:05:53.610507304Z | 20:12:11.615574478Z |
| host terminal記録 | 20:05:53.611707443Z | 20:12:11.618143202Z |
| browser terminal WS受信 | 20:05:53.634Z | 20:12:11.636Z |
| Send有効 | 20:05:53.656Z | 20:12:11.656Z |

native frameは前後とも約5ms以内に記録され、差はその後のcleanup/drain区間にある。
監査書込み全体やWebSocket/UIが5秒遅いという仮説は今回の観測に合わない。
EOF所有参照を再現したunit試験の5006→4msと合わせて原因を特定した。
残る約4秒は `crates/utils/src/process.rs::kill_process_group` の既存の
SIGINT/SIGTERM後の各2秒待機に対応する。この安全処理と5秒drain上限は変更していない。
各1回の比較であり、本番の分位点・負荷時性能を保証する測定ではない。

## 再現操作と観測結果

`python3 tests/terminal-latency/run.py` で専用アプリを起動し、5ケースを実行する。
各ケースで実行中にfollow-up APIを要求すると400/already runningとなることを確認。
正常終了・失敗はfixture通知、中断は実cancel API、再接続はWS切断、reloadは
ページを離れてterminal後に復帰する。Send有効化後に実ボタンを押し、
次runが1件だけ追加され成功することを確認した。前後とも **5/5成功**。

## 実行コマンドと結果

| コマンド | 結果 |
| --- | --- |
| `pnpm --dir packages/local-web run build` | 成功、TypeScript/Vite。Browserslist更新・Tailwind content・bundle size警告あり |
| `cargo build --bin server --bin agent-process-host` | 前後とも成功、自身のworkspace binary |
| `cargo test -p local-deployment process_host::tests -- --nocapture` | 変更後8件成功。新規回帰は変更前に意図した失敗を確認 |
| `cargo clippy -p local-deployment --all-targets -- -D warnings` | 成功 |
| `python3 tests/terminal-latency/run.py` | 前5/5、後5/5成功。専用プロセス停止・一時状態削除も確認 |
| `pnpm exec tsc --noEmit --target es2022 --module nodenext --moduleResolution nodenext --esModuleInterop --skipLibCheck tests/terminal-latency/latency.spec.ts tests/terminal-latency/playwright.config.ts` | 成功 |
| `node --check tests/terminal-latency/codex-fixture.mjs` | 成功 |
| `python3 -m py_compile tests/terminal-latency/run.py` | 成功 |
| scoped Prettier check、`git diff --check` | 成功 |

Cargoでは既存sccacheを利用した。統計は他clientとの共有値であり、このタスク単独のhit率とはしない。

## 証跡と測定上の注意

Git対象外の `test-results/terminal-latency-verified-before/` と
`test-results/terminal-latency-verified-after/` にbrowser.log、timings.json、
measurements.json、provenance.jsonを保持。provenanceにはソースと両binaryのSHA256を記録。
fixtureの実親プロセスが自身のagent-process-hostであることも各ケースでassertした。
初期調査では継承された `VIBE_KANBAN_AGENT_PROCESS_HOST` が別buildを指していたため、
その試行の測定値を全て除外し、明示的な自身のbinary指定で前後とも再測定した。
初期のCSRF/config/fixture protocol/selectorの調整中の失敗も成功扱いに含めていない。

再利用する通常のworkspace `target/` 約22GiB、`node_modules/` 約982MiBは
AGENTS.mdの指示に従い保持。専用DB/runtimeは削除済み。大きな複製buildは残していない。

## 未確認事項・仕様判断

実Codexサービスとの相互運用、Windows、本番高負荷の性能は未確認。
認可マトリクス全般の再試験やLOCAL-19の状態遷移再設計は対象外。
今回必要な追加仕様判断はない。残存するプロセス停止猶予を変更する場合は、
終了・子プロセスの安全性を含む別の検討が必要となる。
