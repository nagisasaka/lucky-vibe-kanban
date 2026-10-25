# LOCAL-81 可読性・Workspace状況表示 / Change Manifest

2026-10-25。実装・局所試験・commit承認済み。main merge/deployは実施していない。
独立レビューは上位AI。プレビューBasic設定後の認証済み確認はユーザー操作待ち。

## Revision・SHA照合

- 初期HEAD: `455f42d2c77b3f512a8346be2c7b1a5294ea6d7e`。作業ブランチだけを初期main `271aa60f14dd1ac94b2cf2a9fe0e1878bef3d2ce`へfast-forward。
- 前回応答時点は未commitと報告したが、再開時のgit log/statusでは実装commit `21d9ff05b045249a0ff59d24530adec25a6b5644`、cleanを確認。これを今回のsource/test起点とする。
- 前回UI fixture検証のソースSHA-256と21d9ff05の製品ソースは一致。
- E2E後は検証記録更新とテストのPrettier整形のみ。整形後`node --check tests/kanban/isolated.mjs`成功。実行時test hashはそのまま保持し、製品ソースhashの一致を最終commitでも照合する。
- 今回のRustテスト・serverは21d9ff05の製品ソース。実API E2Eは同SHA＋今回のテスト追加。server build_infoは21d9ff05、development、dirty=true（テスト変更あり）。製品ソース変更なし。
- binary SHA-256: `45571b887be67d162960114b06627d57d1343052fe024422cfbf7cd3f6d907ae`。
- 最終commit SHAは成果物のfinal-revision.jsonと最終応答に記録。tested-source-hashes.jsonで製品/試験ソースを照合する。SHA表示だけのための重複ビルドは行わない。

## 変更内容

- KanbanCardContentの既存Markdown整形を再利用し、タグ後に本文最大2視覚行。空本文は余白なし、長URL折返し。details開閉中もpreviewを維持。
- Workspace一覧をdetails外へ移動。showWorkspaces・非archive範囲・順序・Workflow除外を維持。本文全文はdetails内。
- カードIntegration行と専用3秒polling/focus状態を削除。Navbar/store経由のIntegrationPanel、同Panelのqueryと正式機能は維持。
- BoardのNULL列ではなく既存localWorkspace summaryへ差分を配線。正常0とunknownを区別し、時間はChatElapsedTime、未知は—。既存canonical境界・15秒pollingは変更なし。
- Workspace stats/PRは狭幅で折返し。共有IssueWorkspaceCardの他利用画面にも表示改善が適用される。
- services内部DiffStatsの行数Optionを保ち、任意repo失敗で全体None。binary/内容省略はfilesへ数え、行数不明を伝播。既存nullable WorkspaceSummaryへ直接渡すためAPI/生成型/SQL変更なし。
- Gitの片側内容不明を追加/削除と誤認しない（実Added/Deletedのみその側を0扱い）。merge-baseとworking tree差分の取得方法は維持。
- 4状態、最大2列、文字サイズ、DnD/順序処理、新API/設定/件数上限/masonryは変更なし。大量Workspace時の高さ・隣の空白は合意どおり許容。

## 検証コマンドと結果

Rust共通環境: `CARGO_BUILD_JOBS=2`、`CARGO_TARGET_DIR=$PWD/target`、`RUSTC_WRAPPER=/usr/local/bin/sccache`、`SCCACHE_DIR=$(readlink -f .lvk-shared/cache)/sccache`。
重コマンドは順次実行。共有target/cargo clean/無関係な全体check/lint/backend:testなし。
sccache実際の保存先一致を前後確認。統計は共有利用者込みであり本試験だけのhit率とは扱わない。

| コマンド | 結果 |
| --- | --- |
| `pnpm run web-core:check` / `ui:check` / `local-web:check` | 前回PASS。製品ソースhash一致を今回確認 |
| 変更UI3ファイル・Container/Details2ファイルのESLint | 前回PASS。同ソース |
| `node tests/kanban/readability.mjs` | 前回PASS。同製品ソース・本部品fixture |
| `cargo test -p git --test diff_stats` | PASS 1件。commit済み/未commit/untracked、text↔binary、削除 |
| `cargo test -p git --lib summary_read_failure_tests` | PASS 1件。Modified読取欠損を削除扱いしない |
| `cargo test -p services summary_diff_unknown_and_multi_repo_failure` | PASS 1件。隔離SQLite/2repo、0・正数・binary・部分/全repo失敗 |
| `cargo build --bin server` | PASS。serverまでの消費側コンパイル確認 |
| `pnpm --dir packages/local-web run build` | PASS。Browserslist古さとchunk-size警告あり |
| `LVK_TEST_SERVER=$PWD/target/debug/server LVK_KEEP_TEST_INSTANCE=1 node tests/kanban/isolated.mjs` | 最終PASS。初回追加testは既存Dialogにないroleを参照して失敗、実際のheading参照へ修正 |
| `python3 tests/kanban/seed-preview.py` | PASS。停止済みの自分のfixture DBへ0/1/4/12 Workspaceサンプルを追加 |
| 外部HTTPSとChromium | PASS: root200、mode=locked、public、forced_mode=basic、業務API423、セットアップ画面。異なるoriginのloopbackアクセス403も確認 |
| `git diff --check` | PASS |

## 実APIとfixtureの区別

実API E2Eは変更ソースserver＋ソースVite、新規SQLite、一時Git repos、隔離Chromiumを使用。
実summary APIで3 files/+3/-1（commit済み＋未commit＋untracked）と0 files/+0/-0を取得し、実カードに配線されたことを確認。
実APIでIssue/Workspace1:N、詳細、列間DnD・並べ替え保存、keyboard/focus、Navbar Integration入口、全4状態の配置を確認。

2560×1440 CSS px/DPR1.5/zoom100%（4K・OS150%相当）、1280×900、390×844、各列535/536/537/536/535px往復。
カードタイトル16px・ellipsis維持、body2行、0/1/複数/多数Workspace、折返しと詳細全文は部品fixtureでも確認。
実行中/承認・入力待ち/失敗等はsummary応答overlayと部品fixture。実AgentRun/モデル・認証遷移の実行証拠ではない。
日本語lightと英語darkスクリーンショットを目視確認。

## preview2・operator手順

- URL: `https://preview2.luckyvibekanban.com`（現在locked、アプリ利用はBasic設定後）。
- Project: `47ce1fc7-9795-4787-b616-9380733543de`。
- PID: `1809660`、bind: `127.0.0.1:18444`。`/proc/PID/exe`とssで実binary/portを確認。
- binary: `/var/tmp/lvk-instances/dev/runtime/worktrees/2f33-2/lucky-vibe-kanban/target/debug/server`。
- instance: `/var/tmp/lvk-instances/dev/runtime/worktrees/2f33-2/lucky-vibe-kanban/.build/kanban-validation/instance-FTz2vi`。
- DATA_DIR: 上記instanceの`data/`。SQLiteは`data/db.v2.sqlite`、共有/live DBや認証DBからのコピーなし。
- `LVK_ACCESS_DEPLOYMENT=public`、`LVK_AUTH_MODE=basic`、`LVK_PUBLIC_URL=https://preview2.luckyvibekanban.com`。既存nginx/TLS/DNS/Auth0設定の変更なし。
- 保持DBは4状態のIssue、Workspace0/1/4/12と実Git差分2workspaceを含む。AgentRunは作成しない。状態アニメーションは試験fixtureのスクリーンショットで確認する。

ユーザー/operatorだけがterminalで次を実行する。出力tokenはブラウザへ直接入力し、チャット/ログへ貼らない（teeやログ保存を付けない）。この作業では発行していない。

```sh
LVK_INSTANCE_DIR=/var/tmp/lvk-instances/dev/runtime/worktrees/2f33-2/lucky-vibe-kanban/.build/kanban-validation/instance-FTz2vi \
  /var/tmp/lvk-instances/dev/runtime/worktrees/2f33-2/lucky-vibe-kanban/target/debug/server setup-token
```

1. 上記URLを開き、発行tokenでセットアップを解錠する。
2. Basicのユーザー名と12〜1024 bytesのpasswordをブラウザ内で設定し、ログインする。
3. 認証済み状態でProjectを開き、カード表示を確認する。secretを渡さず、設定完了を上位AIへ知らせる。

根拠: `crates/server/src/main.rs:58`のCLIは指定instanceのDB/鍵へ発行して終了し、serverを追加起動しない。
`crates/utils/src/instance.rs:5`でinstanceを隔離解決。`crates/server-access/src/store.rs:105`の未設定publicはLocalにfallbackせず、`:217`で業務API423。
`handlers.rs:79`でtoken交換、`:214`で管理sessionによるBasic保存、`store.rs:392`でpassword制約。Auth0変更は不要。

## 保持・cleanup・未検証

- 自分のE2E backend/Vite/browserは終了。公開確認browserも終了。preview serverのみユーザー確認まで保持。
- 保持物: 専用instance約2.2MiB、生成frontend約77MiB、Workspace target約18GiB（再ビルドcacheと稼働binary）。削除/上書きしない。プレビュー完了後も自身のPID/exe/portを照合してから自分のserverだけ停止し、専用instanceを整理する。
- スクリーンショット・logs・results/revision/hashを `.build/kanban-validation`、`.build/kanban-readability`、`.build/kanban-rust`、`.build/kanban-preview`とpersistent artifactsに保持。DB/認証鍵/tokenは成果物コピーに含めない。
- Basic設定後の認証済み外部確認は未実施（ユーザー設定待ち）。実AgentRun/有料モデル、実機touch、Firefox/WebKit、全Workspace/untracked大量走査の性能実測、独立レビューは未実施。
- Rust workspace全体試験/clippyはこの局所変更の検証に追加していない。Formal Integration/main merge/deployは未実施・未承認。
