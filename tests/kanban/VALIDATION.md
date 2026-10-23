# コンパクトKanban 検証記録 / Change Manifest

実装・隔離試験・commitの引継ぎ記録。main更新、デプロイ、LOCAL-65への混在は行っていない。
独立レビューと正式Integrationは上位AIの担当。

## 変更と判断

- 既存の4状態列、状態判定、Issue/Workspace 1:N、既存の順序保存処理を維持。
- 各列のcontainer queryで1列/2列を切り替える。閾値33.5rem（標準root 16pxでは536 CSS px）、最小カード幅16rem、gap 0.5rem、左右padding各0.5rem。3列にはならない。
- 2560×1440 CSS pxで実列幅616〜617px、カード幅約296pxを確認。535/536/537/536/535pxの境界往復で読みやすい最小幅256pxを確保。文字サイズを変更せず、タイトルは16px・ellipsisのまま。
- Issue番号、タイトル、未archiveのWorkspace数、承認/入力待ち・実行・失敗・未確認更新・開発サーバー状態を常時表示。本文と既存Workspaceカードはnative detailsで展開。Workflow/Integrationの重要状態と入口は折り畳みの外に残す。
- 既存の「Workspace表示」設定は展開内一覧に適用し、重要状態の取得・表示を止めない。既存のsummaryとWorkflow queryを再利用し、重複取得・モデル追加はしない。
- 一次元前提のhello-pangeaのDOMをCSSだけでgrid化する案を避けた。既存依存のdnd-kitをgrid表示に限定し、DropResultへ変換して既存handleDragEndへ渡す。リスト表示のDnDは変更しない。
- DOM順を左→右→次段に対応させる通常grid。masonryなし。キーと展開DOMをリサイズ/summary更新で保持。キーボード列移動後は移動先カードへフォーカスを戻す。

## Revisionと環境

- 初期main / backend / 試験開始HEAD: `aa59ab5855ae90b0fe3cf65a2017ff60be52df61`。
- frontendはこのWorkspaceの変更ソースをViteから配信。E2Eはcommit前の変更込みソースで実行。試験時の`revision.json`と`tested-source-hashes.json`を保存し、最終commitの対応ファイルとSHA-256一致を確認する。最終commit SHAは`final-revision.json`および最終報告に記録する。commitのためだけのE2E重複実行はしない。
- backendは既存dev build artifactを読取再利用。`build_info.revision`は上記初期mainと一致、dirty=false。Rust/SQL/APIソース変更なし。Rust重ビルド不要。
- 新規の一時`LVK_INSTANCE_DIR`、SQLite、動的loopbackポート、Playwright Chromium。既存サーバーへの接続・既存DB変更なし。有料モデル未使用。
- 共有資源snapshotはresources/holders/queueすべて空。backend再利用＋ViteによりRust/production bundleビルドを避けた。
- 2560×1440 CSS viewport、DPR=1.5、browser zoom=100%（物理3840×2160・OS150%相当）。追加1280×900、390×844。物理4Kモニター実機での目視ではなく、Chromiumエミュレーションとスクリーンショットによる確認。

## 実施結果

| コマンド / 確認 | 結果 |
| --- | --- |
| `pnpm install --frozen-lockfile` | 成功。lockfile変更なし |
| `pnpm run web-core:check` | 最終PASS。初回のcollision container型エラーを修正後に成功 |
| `pnpm run local-web:check` | PASS |
| `pnpm --dir packages/local-web exec eslint --config .eslintrc.cjs ../web-core/src/features/kanban/ui/KanbanGrid.tsx ../web-core/src/features/kanban/ui/KanbanCardDetails.tsx ../web-core/src/features/kanban/ui/KanbanContainer.tsx --no-ignore --max-warnings 0` | PASS |
| 変更ファイルへの既存設定Prettier / `git diff --check` | PASS |
| `LVK_TEST_SERVER=/var/tmp/lvk-instances/dev/switch-operations/2218e595-8396-435b-8c2b-58f33e50a2cd/artifacts/server node tests/kanban/isolated.mjs` | 最終exit 0、Chromium E2E PASS、finally cleanup完了 |

実API経路: 30件＋Done追加3件、長いタイトル、空列、全4列の2列表示、1↔2境界往復、狭幅の1列表示、最小幅・非重複・行優先順、2つのWorkspaceリンク、本文/Workspace展開、詳細遷移、マウス列間移動と並べ替えのAPI保存、リロード後保持、キーボード横/次段/列間移動・Escape取消・フォーカス保持。

fixture経路: `/api/workspaces/summaries`の応答だけを上書きし、実行中＋承認待ち、入力待ち、失敗、未確認更新、開発サーバーの独立表示を確認。15秒の既存summary更新でもフォーカスとカード左上座標を保持。1280幅でも重要状態が見える。dark/ENとlight/JAを確認。

初期harnessは非表示Cancelledを含む5状態の扱いを修正。キーボード試験はブラウザ更新前に次キーを送らないよう100msのキー保持時間を設定。APIから追加したIssueの検証では既存同期の待ち時間に依存せずリロードする。これらの失敗を合格として数えていない。

## 成果物と未実施

`.build/kanban-validation/`:

- `browser.log`, `server.stdout.log`, `server.stderr.log`, `results.json`
- `revision.json`, `tested-source-hashes.json`, `final-revision.json`
- `compact-2560.png`, `compact-1280.png`, `compact-390.png`, `expanded-2560.png`
- `fixture-running-approval.png`, `fixture-attention.png`, `fixture-attention-1280.png`, `light-ja-2560.png`

最終成果物の保持先は最終報告を参照。起動したbackend/Vite/Chromiumは終了し、一時instance DBは削除済み。既存サーバーや他Workspaceのプロセスは停止・変更していない。

未実施: 実モデル実行/承認状態遷移、Workflow/Integration実行経路、実機タッチ操作、Firefox/WebKit、production bundle、Rust全体check/lint/backend:test。今回の表示変更には型・変更ファイルlint・実アプリChromium試験を選択し、別の状態遷移/実行系や全体チェックの反復を避けた。タッチは既存dnd-kitの遅延sensorを使用するが実機検証は残る。独立レビューと正式Integrationは未実施。
