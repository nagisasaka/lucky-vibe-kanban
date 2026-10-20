# LOCAL-26: チャット・TODO入力の検証

2026-10-20、Workspace `55fd706e-9e82-4d59-9a6a-4c9466d930dd`。
対象TODO: `1632014e-f932-4b0e-8e88-3d4d0aedfc77` / `8b0832e6-16d5-485d-907e-ec11d36492d2`。

## 変更と仕様判断

実装は中断時のLVK checkpoint `3360c2c7`に保存されていた。再開後はそのソースをbuildし、試験の処理中セレクタとcleanupを修正して検証を完了した。

- チャットの既存Session・新規Workspace入力は保存済み旧ショートカット設定によらずEnter送信。Ctrl+Enterは明示的な改行。共通Lexical処理のIMEガードを維持し、キーリピートでは送信しない。
- TODO追加はEnterとボタンが同一のsubmit検証を通る。空本文を送信せず、カテゴリが空文字・空白だけならエラーを表示して追加しない。Ctrl+Enterは選択範囲を改行に置換する。
- TODO・チャット送信に同期的な処理中ガードを追加。既存の下書き削除・scroll・steeringは変更しない。
- Shift+Enterなど既存の補助操作、既存未分類TODOの表示・編集、REST/MCPの任意カテゴリ契約は維持。未分類拒否は追加UIの要件として実装した。
- 主な根拠: `packages/ui/src/components/KeyboardCommandsPlugin.tsx`、`packages/web-core/src/features/todos/ui/TodoPanel.tsx`、`packages/web-core/src/features/workspace-chat/model/hooks/useSessionSend.ts`、`SessionChatBoxContainer.tsx`、`packages/web-core/src/shared/components/CreateChatBoxContainer.tsx`。

## 実行結果

このjob内の重いコマンドはすべて逐次実行。Cargoのみjob限定で並列数2とした。グローバル設定や共有sccacheの再起動は行っていない。

| コマンド | 結果 |
| --- | --- |
| `CARGO_BUILD_JOBS=2 RUSTC_WRAPPER=/usr/local/bin/sccache SCCACHE_DIR=<repository shared cache>/sccache cargo build --bin server --bin agent-process-host` | PASS、9分17秒 |
| `pnpm --dir packages/local-web run build` | PASS、tscと実frontend bundle生成。Browserslist・Tailwind・bundle size警告あり |
| `pnpm run ui:check` | PASS |
| `pnpm run web-core:check` | PASS |
| `pnpm run ui:lint` | PASS |
| `pnpm run local-web:lint` | PASS |
| `node scripts/check-unused-i18n-keys.mjs` | PASS |
| `node --test --test-timeout=120000 tests/chat/editor-display.test.mjs` | PASS、1 test |
| `node tests/chat/input-operations.isolated.mjs` | PASS、TODOとチャットの実画面操作 |

前回中断のbuild、会話同期を含む試験一式、当時timeoutした試験はPASSに含めない。再開時のE2E初回は、チャットが処理中に`contenteditable=false`となり試験の`[contenteditable=true]`セレクタが外れたため失敗。安定したセレクタへ変更し、処理中false／応答後trueをassertした最終実行がPASS。無関係な会話同期試験の再実行や修正は行っていない。

## 隔離E2Eの再現・期待操作・観測結果

前提として上記backendとfrontendをこのcheckoutからbuildし、同梱のPlaywright/Chromiumで `node tests/chat/input-operations.isolated.mjs` を実行する。スクリプトが新規instance、fixture repository、Project、Workspace、Sessionを生成し、終了時にcleanupする。

最終実行のURLは `http://127.0.0.1:46761`。ポートは毎回OSが割り当てる。`LVK_INSTANCE_DIR`は `.build/input-validation/instance-*` 配下で、専用DB/data/runtimeを使用。既存dev、live、nginx、他Workspaceには接続しない。

| 再現操作 | 期待・観測結果 |
| --- | --- |
| TODO本文を空白だけにしてEnter | 追加なし、ボタン無効、DB件数不変 |
| TODOカテゴリを空／空白にしてEnterまたは追加ボタン | カテゴリエラー表示、本文保持、DB件数不変 |
| 日本語本文で`isComposing=true`または`keyCode=229`のEnter | 両画面とも送信・追加なし |
| TODOで「一行目」→Ctrl+Enter→「二行目」→Enter | 改行を含む本文が実DBへ1件保存される |
| TODO追加POSTを保留中にEnter・form submitを5回発火 | 入力・ボタン無効、POSTは1件。保留解除後の実保存も1件 |
| TODO複数行を追加ボタンで保存 | 実DB件数が1増加 |
| チャット設定が旧`ModifierEnter`の状態で空白→Enter | follow-upリクエストなし |
| チャットでCtrl+Enterを挟んで二行入力→Enter | 正しいSession宛に改行入りpromptが1件送信される |
| 両画面でrepeat付きEnter | 送信・追加なし |
| チャットfollow-upを保留中にEnterを5回発火 | 編集不可、追加リクエストなし |
| チャットに制御した失敗応答を返し送信ボタンを押す | 編集可能に戻り、同じpromptで2件目のリクエスト。ガードが解除される |

## 実backendとmockの境界・未確認事項

- 実backend: このcheckoutからbuildしたserverが実bundleを配信。Project・Workspace・Sessionの準備、TODO追加・一覧・SQLite保存、通常画面のAPIを使用した。
- TODO POSTは遅延制御だけを行い、その後`route.continue()`で実backendに送った。保存応答はmockではない。
- チャットの`/api/sessions/*/follow-up`だけはPlaywrightで保留し、意図的な400応答を返した。実agent/providerの起動・応答成功は未検証であり主張しない。
- IMEはChromiumへの合成KeyboardEventでブラウザガードを確認した。OSの日本語IMEそのものを手操作した検証ではない。
- 全ブラウザ操作に未処理page errorなし。画像は `.build/input-validation/screenshots/todo-input.png` と `chat-input.png`。ログは `e2e-final.log`、`editor-test-resume.log` と各 `*-resume.log`。
- 稼働sccacheは指定の共有cacheと異なる `/home/appuser/.cache/sccache` を使用していた。共有サービスは再設定せず、cache統計は共有クライアント合計として記録した。

## Cleanup

最終実行はChromiumをclose、専用serverの終了をawait、一時instanceを削除。実行後の確認で、自Workspaceの重い処理・server・Chromiumと`instance-*`はいずれも残留なし。通常の`target/`、依存関係、frontend bundleと検証ログ・画像は保持した。merge、deploy、live再起動、TODO完了操作は実施していない。
