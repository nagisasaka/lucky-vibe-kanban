# LOCAL-81 可読性・Workspace状況表示 / Change Manifest（ビルド前）

2026-10-25。実装・局所試験・commitは承認済み。main merge/deployは禁止。
重ビルドは資源調整回答81a05adc-a6cc-47b4-8178-57ccc5333170により保留中。
この記録は完了報告ではない。Rust検証、実API E2E、認証付き独立プレビュー、commit、上位AI独立レビューが残る。

## Revision

- 開始HEAD: `455f42d2c77b3f512a8346be2c7b1a5294ea6d7e`、clean、作業ブランチ `vk/2f33-2`。
- 初期main: `271aa60f14dd1ac94b2cf2a9fe0e1878bef3d2ce`。
- `git merge --ff-only main`で作業ブランチのみ更新。試験対象はこのSHA + 未commit差分。
- 最終commit SHA: 未作成。mainは更新していない。
- `.build/kanban-readability/tested-source-hashes.json`に対象ソース/テストのSHA-256を記録。

## 変更範囲と判断

- KanbanCardContent: 既存Markdownプレビュー整形を再利用し、タグ後に最大2視覚行。空本文は要素なし、長URLはanywhere折返し。展開とは独立。
- KanbanContainer / KanbanCardDetails: showWorkspacesを尊重してWorkspace一覧をdetails外へ。既存非archive範囲・順序・Workflow除外を維持。本文全文はdetails内。4状態・最大2列・DnDロジックは無変更。
- カードIntegration行、専用3秒polling、focus状態を削除。IntegrationPanelを維持しNavbar/store入口と同Panel固有queryは変更しない。実ブラウザで別入口の動作確認は残る。
- Workspace数値をBoardの常時NULL列ではなく既存localWorkspace summaryへ接続。
- IssueWorkspaceCard: 正常な0を表示、未知は—。時間はChatElapsedTimeを再利用しtimeAgo代用を削除。欠損/不正/逆転時刻は未知。statusと時刻の取得・15秒polling・canonical境界は無変更。狭幅で数値/PRを折返す。共有部品のため他のIssueWorkspaceCard利用画面にも適用される。
- services diff stats: 任意repo失敗で全体None、binary/省略内容等で行数Noneを伝播、files数は維持。serverはこの内部集計型を直接使用し既存nullable WorkspaceSummaryへ渡す。公開API型/生成ファイル/SQLは変更なし。
- Git: 実Added/Deleted以外は片側内容不明を追加・削除行数として数えない。既存merge-base/working tree/untracked取得を維持。
- 新API・計算基盤・polling・設定・件数上限・masonryは追加しない。既存全Workspace並列集計/untracked走査性能の実測は未実施。

## 実施済み

| コマンド | 結果 |
| --- | --- |
| `pnpm run web-core:check` | PASS（初回のoptional型/Integration props不一致を修正後） |
| `pnpm run ui:check` | PASS |
| `pnpm run local-web:check` | PASS |
| `pnpm --dir packages/ui exec eslint src/components/ChatElapsedTime.tsx src/components/IssueWorkspaceCard.tsx src/components/KanbanCardContent.tsx --max-warnings 0` | PASS |
| `pnpm --dir packages/local-web exec eslint --config .eslintrc.cjs ../web-core/src/features/kanban/ui/KanbanContainer.tsx ../web-core/src/features/kanban/ui/KanbanCardDetails.tsx --max-warnings 0` | PASS（初回config指定不足を修正） |
| `node tests/kanban/readability.mjs` | PASS（fixture CSSのconfig directive除去、keyboard event待機を修正後） |
| 対象ファイルのPrettier/rustfmt、`git diff --check` | PASS |

ブラウザ試験は本ソースの実部品をesbuild/Tailwindで処理して隔離Chromiumへ載せる**fixture**。
実DB/API/認証/IntegrationPanelの試験ではない。
2560×1440 CSS px、DPR1.5、ブラウザzoom100%（物理4K/OS150%相当）と1280/390幅、各状態列535/536/537/536/535px往復。
本文空/Markdown/コード・添付ラベル/長URL、Workspace0/1/4/12、running/approval/failed/terminal/unknown、0/正数/未知を確認。
詳細展開と閉じた状態でもpreview/Workspaceを維持、showWorkspaces OFFでも重要状態保持。
summary再描画時のfocus、Workspace Enter操作、keyboard列間DnDとfocus、pointer空列移動を確認。
スクリーンショットを目視確認。既存タイトル省略と文字サイズを維持。
`.build/kanban-readability/`にwidth-2560.png/width-1280.png/width-390.png、results.json、browser.logを保持。
ブラウザはfinallyで終了。外部サーバー/DBは使用せず、新規サーバーも起動していない。

## 準備済み・未実施

- `cargo test -p git --test diff_stats`：commit済み+未commit+untracked、text↔binary、実削除。
- `cargo test -p git --lib summary_read_failure_tests`：Modifiedの内容読取失敗を削除と誤認しない。
- `cargo test -p services summary_diff_unknown_and_multi_repo_failure`：隔離SQLite/2repoで0、正数、binary、部分/全repo失敗。
- 上記は**未コンパイル・未実行**。対象crate check/必要clippyも未実施。
- serverビルドと`LVK_TEST_SERVER=... node tests/kanban/isolated.mjs`。旧本文非表示assertionは更新済み。実API配線、保存/DnD、Integration別入口の回帰確認を追加する。
- 資源承認後、jobs<=2、sccache共有設定を確認し、workspace専用targetを使用。cargo clean禁止。重ビルド/全体check/lint/backend:testを開始していない。

## preview2準備・ブロッカー

- 稼働nginx `/tmp/lvk-server/nginx.conf:129`の既存TLS vhost `preview2.luckyvibekanban.com` → `127.0.0.1:18444`を確認。設定変更なし。
- 18444は確認時listenなし。外部HTTPS `https://preview2.luckyvibekanban.com` は502（未起動）。利用可能なプレビューURLとは報告しない。
- 同vhostは独立アプリが認証を所有する構成。新規fixture instanceには独立した認証準備が必要。live DB/認証DBはコピー・共有しない。
- Auth0を使う場合はpreview2向けの既存許可済みcallback/credential/ログイン手段が必要。認証方式変更、Auth0/nginx/DNS変更、認証無効化は行わない。認証済み外部動作は未検証。
- 起動前に再度資源snapshot/18444占有を確認する。LOCAL32の18443は使用しない。
- 起動後は専用fixture DBに4状態・複数/多数Workspaceをseedし、人間確認まで専用プロセス/ソース/データを保持する。現在プレビューのserver/dataは未作成。後日cleanupは自身の所有分だけを対象とする。
