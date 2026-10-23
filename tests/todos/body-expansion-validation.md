# TODO長文の4行省略 — Change Manifest

## 範囲と判断

- 初期main/HEAD: `a18cb18d72cd5d5b6068455e6561362e4cbbe768`。LOCAL-84統合済み。
- このWorkspaceでは実装・対象検証・commitまで。独立レビューと正式Integrationは上位AIへ引き継ぐ。main更新・デプロイは行わない。
- `TodoBody.tsx` が既存本文buttonを置換し、表示だけをCSS `line-clamp-4`で省略する。全文データと編集・保存・キャンセル・完了・メニュー・関連カードの既存処理は維持。
- 4行を超える実overflowだけに、独立した「もっと見る／折りたたむ」buttonを表示。`aria-expanded`、`aria-controls`、可視focusを備える。全7言語へラベル追加。
- 展開中も判定できる同幅・同書式の非表示計測spanを使用。計測spanは`aria-hidden`で操作不可。ResizeObserverとFontFaceSetのready/loadingdoneで再計測し、unmount時に解除。
- 連続文字は`overflow-wrap:anywhere`。選択範囲を持つ本文でのポインターclickは選択を維持し、Enter/Spaceの編集は維持する。
- 一時的な展開状態は表示コンポーネント内のみ。編集終了・再mount時は省略へ戻るKISS方針。タイトル・DB・API・本文切詰め・設定追加は行わない。
- LOCAL-25のカテゴリ配置を変更しない。既存の高低差配置試験は長文fixtureを展開して元の高さ条件を保持。LOCAL-81やLOCAL-84の実装を変更しない。

## 検証環境と互換性

- 読取再利用: `/var/tmp/lvk-instances/dev/runtime/worktrees/0da9-todo-ui-mcp/lucky-vibe-kanban/target/debug/{server,agent-process-host,vibe-kanban-mcp}`。
- server build outputの実SHA: `e517630a6ba0c556dcb361f309260fc42886f38a`。既存のLOCAL-84検証記録とも一致。
- `git diff e517630a HEAD -- crates shared/types.ts`で、差分が対象外Web Push context endpointの削除のみと確認。TODO API/model/schema/MCPの実装は一致。旧成果物の書換え・削除なし。
- shared resource snapshotはresources/holders/queueすべて空。独自一時`LVK_INSTANCE_DIR`・DB・動的loopback portを使用。Viteは本Workspace変更ソースを配信し、隔離backendだけへproxy。既存dev／本番へ接続しない。
- Vite起動と停止を既存isolated harnessへ追加。既定のbuilt frontend試験も維持。外部成果物の指定は`LVK_TODO_BIN_DIR`、変更ソースの配信は`LVK_TODO_VITE=1`。
- Chromium: `/opt/lvk-browsers/chromium-1217/chrome-linux64/chrome`。遅延フォント試験はインストール済みLiberation Monoを新しいFontFaceとして読込。有料モデル・agent実行なし。

## コマンドと結果

| コマンド | 結果・所要時間 |
| --- | --- |
| `pnpm install --frozen-lockfile` | 成功。時間未計測 |
| `pnpm run web-core:check` | 成功、25.830秒 |
| `pnpm run local-web:check` | 成功、24.453秒 |
| `pnpm --dir packages/local-web exec eslint ../web-core/src/features/todos/ui/TodoBody.tsx ../web-core/src/features/todos/ui/TodoPanel.tsx --config .eslintrc.cjs` | 成功 |
| 変更TSX/mjsへの既存Prettier | 実施 |
| `node --check tests/todos/{isolated,body-expansion,layout}.mjs`（各ファイル個別） | 成功 |
| `git diff --check` | 成功 |

隔離E2E実行コマンド:

```sh
LVK_TODO_BIN_DIR=/var/tmp/lvk-instances/dev/runtime/worktrees/0da9-todo-ui-mcp/lucky-vibe-kanban/target/debug \
LVK_TODO_VITE=1 \
LVK_CHROMIUM_EXECUTABLE=/opt/lvk-browsers/chromium-1217/chrome-linux64/chrome \
node tests/todos/isolated.mjs
```

最終E2Eは成功（114.098秒）。追加試験と既存回帰を全て通過し、browser pageerrorなし。
試験対象は本commit収録の製品コードと試験スクリプト（commit前に実行、以降は記録だけの変更）。
試験対象SHA／最終SHAの完全値は `.build/todo-validation/result.json` とrepository memoryに記録する（文書自身のSHAを自己参照しない）。
実装と対象検証は2026-10-23 19:15 UTC頃開始、最終E2E完了は19:23 UTC（約8分、環境時計。引継ぎ記録時間は別）。

追加E2Eの範囲: 短文、4行境界、多改行、日本語長文、連続文字、4行の実測と省略、展開／折りたたみ、1600/390/320px、展開中の幅変更、遅延font読込、Enter/Space、タッチ、全文一回click編集、保存・キャンセル後の実DB本文照合、文字選択、完了・メニューの独立。
既存E2Eも実API/SQLite/MCPの作成経路、CRUD/revision、Workspace/Project、関連カード、カテゴリ配置、1000px、dark/light、reloadを検査。

初期試行の失敗は試験ハーネスの準備・前提に起因:

1. Vite programmatic起動のcwdがrepo rootでBabel pluginを解決できなかった。通常起動と同じlocal-web cwdに修正。
2. cold Viteロードで初期15秒のUI待機が不足。Viteモードだけ60秒に延長。
3. 完了項目を隠す既存設定のままchecked表示を期待した。試験で完了済み表示を有効に修正。

これらを成功扱いしない。再実行は上記修正の確認目的。製品コードは型検査後に変更していない。

## 未実施・残リスク・成果物

- Rustビルド／Rust全体試験、無関係な全体check/lint/backend:test、production frontend build、実Auth0/ChatGPT、Firefox/WebKit、実機モバイルは未実施。frontend変更の型検査と実ブラウザ回帰を優先。正式Integrationの検証は別工程。
- Chromiumタッチエミュレーションは実機OSの文字選択UIまで保証しない。文字選択の回帰はDOM Rangeとpointer clickの合成イベントで確認。
- Viteの既存Browserslist/Tailwind警告とproxy reconnect表示あり。判定は実際のCSS形状・API保存値・pageerrorに基づく。
- ログ・画像: このWorkspaceの `.build/todo-validation/`。`e2e-attempt1/2/3.log`は失敗根拠、`e2e.log`は最終実行。`typecheck.log`、`local-web-check.log`、`eslint.log`、`screenshots/body-{collapsed,expanded}.png`ほか既存回帰画像を保持。
- repository memoryとsemantic draftは指定shared persistent pathに保存し、Gitへ追加しない。

後片付け確認: harness終了code 0。試験のserver/MCP/Vite/Chromiumプロセスと`instance-*`ディレクトリは残っていない。画像で省略記号・展開表示・mobile・light themeを確認。保持するログ／画像は約1.6MiB、通常のnode_modules/Vite cacheは追試用に保持。Rust targetを新設せず、空き約23GiBを維持。他Workspace・共有cache・既存serverを変更／停止していない。
