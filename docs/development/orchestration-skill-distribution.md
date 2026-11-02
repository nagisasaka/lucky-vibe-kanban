# LOCAL-242: orchestration Skillの正本と配布

共通正本は `skills/lvk-orchestration/`。登録済みOpenAI版1.0.40を出発点に再構成した。`plugins/lvk/skills/lvk-work/` は既存の接続・操作Skill、`assets/skills/auto-merge/` は自動マージ担当実行Skillであり、上書きしない。既存配布経路 `scripts/chatgpt-mcp/` の設定や認証も変更しない。新しい配布scriptは、登録Appに結び付いたorchestrationパッケージを共通正本から作るための補助である。

## 固定した入力

取得日時 `2026-11-02T10:10:33Z`、version `1.0.40`、release_id `pluginrel_6ae855d3ee948191aad1136167077267`。plugin_id `plugin_asdk_app_6ad4b7896d3c819193e998d31d059e3d`、USER / PRIVATE。引き継ぎ添付全体は47,829 bytes、SHA-256 `7837ce00cbe889e9031881c879018ae0c14962ed7d178d8d7bbba9f35c78b4c3`。全4ファイル、欠落・サイズ制限なし、Skill全文45,198 UTF-8 bytesを確認した。生スナップショットはGit対象外の添付に保持し、配布には含めない。

`plugins/lvk-orchestration/openai/` の3設定ファイルはこの登録版とJSON値が一致（末尾の余分な空行のみ整理）。versionは登録前の基準値であり、新版登録済みという意味ではない。App ID・表示・接続・認証の変更を伴わずに本文と参照資料を更新する。Claudeは `.claude-plugin/plugin.json` と `.mcp.json` のパッケージ形式を用い、接続先は作成時に明示する。実際の認証・インストールは対象クライアントで別途確認する。

## 旧配置と新配置

| 登録版の見出し・規則 | 新しい正本 | 整理内容 |
| --- | --- | --- |
| North star | SKILL.md | 人間の思考、実装・体験から思考へ戻る循環、並列AIの位置付けを保持 |
| Conversational collaboration / Response style | references/using-lvk.md | 設計対話、委譲の度合い、Goalの選択、用途別報告と時刻の扱いを保持 |
| Understand intent / Response lifecycle / Reuse | references/execution.md | Issue・Workspace・Session・Run、同目的のSession継続、待機・直接指示・状態確認を集約。移管の詳細はauto-mergeへリンク |
| Language consistency | SKILL.md | ユーザー言語で依頼・報告、技術リテラル保持を共通原則へ統合 |
| Investigate / Selective specification review / Review comments / REVIEW execution | references/investigation.md | 調査・REVIEWの重複を統合。権限、停止条件、成果物のGit混入防止、review_targetと保存確認を保持 |
| Todos / Design Notes / Priority and tags | references/records.md | 意図と実行の区別、append-only、speaker、receipt、system tag境界を保持 |
| Independent Workspace previews | references/previews.md | プロセス寿命・経路・認証・データ分離を保持 |
| Safe executionの資源判断・実行委譲 | references/execution.md | 例外のない直列化にはせず、実測に応じた並列実行判断を保持 |
| Safe executionの調査・一時成果物 | references/investigation.md | 製品変更との境界と自動commit除外を保持 |
| Terminology / Auto-merge / Validation / Reuse末尾の移管 / destructive archive | references/auto-merge.md | 開始→検証・公開→再開/移管→片付けへ接続。アーカイブ確認の重複を完了ゲートへ統合 |
| Developer-only self-restart | references/lvk-development.md | 開発者のみの機能・認可・受付/完了/再接続後確認を保持 |

単なる移動以外の変更は以下である。権限緩和は行わない。

- 旧「競合なしならCodexなしのmechanical merge」と「hostが固定検証セットを実行」の記述を現行の一会話方式へ修正。`crates/server/src/remote_mcp/launch.rs` の `start_integration`、`crates/server/src/routes/integrations/conversation.rs` と `runtime.rs`、`assets/skills/auto-merge/SKILL.md` を照合した。担当Codexが候補・修復・検証/理由付き省略・有効Wikiを扱い、hostは収束・証拠・Git公開を検査する。
- Wikiは同候補branchの別commit、公開受付とGit反映は別、target前進時は安全な祖先関係の場合だけ完成候補をそのまま公開、と明記した。旧検証結果を新SHAの結果へ付け替えない。
- 移管後の片付けを明示的な完了ゲートにした。旧Issueと全Workspace、archiveと物理削除を別々に確認し、未処理・確認待ち・完了・失敗を報告する。既承認の同じ削除範囲は再確認せず、busy拒否を迂回しない。
- `crates/server/src/routes/workspaces/archive.rs` と `local_remote/issue_archive.rs` の `archived` / `worktree_deleted` / 削除エラー、事前確認と再検査を照合。単なるCancelledやarchive成功では物理削除成功を推定できない。
- 直接操作の契約は `crates/mcp/src/task_server/tools/{sessions,design_notes,issue_tags,repository_read}.rs`、remote側の `launch.rs` / `issue_comments.rs` と照合。実装上の存在と接続先での露出は区別する。

## 再生成と担当間の受け渡し

レビュー後、配布元コミットを確定し、リポジトリルートから次を実行する。出力先は自動commit対象外のタスク専用ディレクトリとする。既存の出力ファイルは上書きしない。

```bash
python3 scripts/package-orchestration.py --commit <full-commit-id> --output <artifact-directory> --claude-mcp-url https://<lvk-host>/mcp
```

scriptは作業木ではなく指定commitのSkill・設定を読み、同じ本文と全参照資料を両zipへ入れる。OpenAIは登録済み3設定、Claudeは専用manifestと明示したHTTP MCP設定を使う。`distribution-source.json` と外部の `distribution-receipt.json` に配布元SHA、同梱一覧、zipハッシュを記録する。zipはクライアントへ渡す候補パッケージであり、登録APIの入力形式への適合やアカウントでの有効化を保証しない。

ChatGPTはレビュー修正を同じ実装Sessionへ戻し、登録側の後発変更と基準版を比較し、必要な変更をGit正本へ取り込んでから配布元を確定する。登録後version / release_idと配布元SHAの対応を保存し、本体と必要な参照資料を実際に取得して確認する。保存成功と会話中の実読は別結果とする。main前の配布なら「候補コミットから配布中・main未反映」と報告する。

配布後の関連変更は配布元とのdiffを確認し、内容が変われば新しいcommitから再配布する。main統合後も配布元との対応を照合する。登録前は「候補成果物作成済み・未登録・main未反映」とする。

## 確認の境界

基本確認はfrontmatter、相対リンク、全参照資料の同梱、両zipの内容一致、登録版設定の保持、差分検査。アプリbuildや製品E2Eはこの文書・配布変更の確認には含めない。

LOCAL-236→238への手順適用では、相互リンクとCancelledの後にも旧IssueとWorkspaceのarchive・物理削除の確認が残る。未承認なら旧片付けを確認待ちとし、238の独立作業を止めない。既承認の同対象・範囲なら再確認しない。部分移管は旧Issueを閉じず、人間の単純なキャンセルから自動archiveを導かない。これは文書上のwalkthroughであり、236/238を再操作した結果ではない。

ChatGPT登録・参照資料の実読、Claudeアカウントへのインストール/有効化/実利用、実MCP認証・App接続は未確認。版を分けた大型評価・fixtureや新たな配信MCPは追加しない。実利用で改善し、このIssue全体の完了はChatGPT側の登録・実読確認まで引き継ぐ。

今回の実行では、標準 `quick_validate.py` はPyYAML未導入で実行不能だった（合格扱いしない）。代わりにPython標準ライブラリでfrontmatterの必須値、全相対リンク、7参照資料、登録設定のJSON値一致（整形前にはbyte一致も確認）、配布scriptの構文を確認した。依存追加は行っていない。配布script自体も両zipを生成し、リンク・同梱内容・CRCを検査する。
