# 自動マージの利用者向け名称

2026-10-27の名称統一。日本語は「自動マージ」、英語は「Auto-merge」。実行・承認・検証・反映・Wiki後処理の挙動は変更しない。

## 対象と確認方法

- UI: `common.json` の `todo.integration`、`integrationPanel`、`integrationGate`、`integrationReview`、`settings.json` の `settings.integrationValidation`。日本語・英語の見出し、開始、中止、履歴、検証省略、復旧案内を確認。既存のスペイン語・フランス語・韓国語・簡体字・繁体字の訳も同義の名称に変更。未翻訳キーは従来どおり英語へフォールバックする。
- AI: `skills/lvk-orchestration/SKILL.md` と `plugins/lvk/skills/lvk-work/SKILL.md`、同梱の結果確認ガイド、Plugin説明、MCPの開始・一覧・詳細・承認・中止の説明と応答、競合解消プロンプト、共有ディレクトリの案内。通常説明・開始確認・進捗・結果・ハンドオフで標準名を使い、内部ラベルを利用者向けの別名として提示しない。
- Configの説明コメントをRustの正本から変更し、`pnpm run generate-types` で再生成。生成差分は `shared/types.ts` の説明1行のみで、型構造・JSON schemaは変更なし。
- 今後作成するカード・Workspace・Session名、通知、操作エラーも変更。Git作者名・メール、機械識別子は維持する。
- READMEと現行の利用手順を更新。Git反映とWiki結果を別々に表示する既存UIを維持し、Wikiだけの失敗はコードのマージ失敗ではないとREADME・UI補足・Skillに明記。

検索は `rg -n -i '正式[ ]?Integration|正式インテグレーション|formal integration|自動インテグレーション|自律インテグレーション|自律マージ|auto merge'` を起点にし、対象モジュールの単独 `Integration` / `integration` / `統合` も確認した。変更後はREADME、同梱Skill、Plugin、現行利用文書、i18n、MCPの現行説明に旧機能名がないことを確認する（内部コメントは別扱い）。

## 残す表記と理由

| 残す箇所 | 理由 |
| --- | --- |
| Rust/TypeScript型、i18nキー、API URL、MCPツール名、DB列・保存キー・状態・イベント、`integration-validation.json`、`LVK-Memory-Integration` | 既存クライアント・保存データ・履歴との互換性。名前変更やデータ移行は行わない。 |
| 既存DB migrationの予約違反メッセージとそのDB試験 | 内部DB診断と過去migrationの契約を保持。migration本文の書換えや名称のためだけの新migrationは作らない。 |
| 内部コメント、過去の設計・検証記録、試験用データ、既存カード・ログ・コミット | 実装上の識別・過去原文として保持。現在の機能説明や新規生成文言とは区別する。運用ガイドを含む文書は現行利用部分のみ更新。 |
| `openwiki/` と既存 `.llm-wiki` | 通常開発では読取専用・保存対象。名称統一のための保守実行は行わない。 |
| GitHub/MCP/外部エディタなどのintegration、Arenaの統合、DesignNoteの `integrate` | 別概念または機械的action。無差別置換しない。 |

## 配布側Skillとの整合（LOCAL165）

上位AIから現ChatGPT Plugin v1.0.32で呼称更新済みとの連携を受領。外部PluginはこのWorkspaceから変更しない。同梱2つのSkillには名称統一の差分だけを適用し、承認・調査mode・並列・検証方針や既存の配布側との差分を全文上書きしない。

共通の推奨文言:

> 利用者向けの説明・開始確認・進捗・結果報告・ハンドオフでは、日本語「自動マージ」、英語「Auto-merge」を使う。内部ラベルを別名として提示しない。技術的なtool/API/DB/type/pathと必要な過去原文引用は保持する。AIを使わない機械マージもあり、必要な判断・承認では停止する。コードのマージ結果と後続Wiki更新結果を区別し、Wikiだけの失敗をコードのマージ失敗と報告しない。

同期元は上記の同梱Skillと `plugins/lvk/skills/lvk-work/references/result-checks.md`。公開・外部Plugin配布の操作はLOCAL165の担当に委ねる。

## 検証

- `pnpm install --frozen-lockfile`: 成功。lockfile変更なし。
- `pnpm run web-core:check` / `pnpm run local-web:check`: 成功。
- web-coreの既存Vitest: `IntegrationResolutionReview` 6件、`IntegrationInterventionPanel` 14件、`integrationApi` 7件、`IntegrationGateControls` 4件、`cardContext` 7件、計38件成功。
- `node scripts/check-unused-i18n-keys.mjs`: 成功。旧版JSONとの全キー・埋込変数比較、7言語の実i18next解決（ナビ・見出し・開始・中止・履歴・進捗・設定）も成功。
- local-webの既存ESLint設定を明示して変更したUI 2ファイルを検査: 成功。`cardContext.ts` は既存の未使用引数 `_pipelines` に関する2件で失敗し、変更前HEADの同ファイルでも同じ失敗を確認。名称変更に無関係な実装は修正していない。
- `node scripts/dev-cargo.mjs test -p server --lib routes::integrations::`: 28件成功、6件は既存の明示的な隔離E2Eとしてignored。機械マージ・競合承認・検証省略・受付・反映・取消・Wiki用outboxの回帰を含む。
- `node scripts/dev-cargo.mjs test -p server --lib remote_mcp::`: 122件成功、6件ignored。ツールcatalog/schema・受付・承認/取消・保存結果の読取・検証/反映/Wikiの分離を含む。
- `node scripts/dev-cargo.mjs test -p server --lib routes::integration_validation::`: 7件成功。保存競合・branch/HEAD変更・symlink拒否・既存データ保持を確認。
- `pnpm run generate-types` / `pnpm run generate-types:check`: 成功。生成型のコメントを除く差分がないことも確認。
- 変更したRustファイルはrustfmtで整形。文字列・説明を除くRustコード、機械的文字列キー・状態・ツール名が旧版と同じことを比較検証。

実ブラウザの日英画面・通知配送、実AIによる競合解消、稼働DB、外部認証、配布・デプロイは未検証。表示文言・翻訳解決と既存component/API/domain fixtureの検証を選択し、新しい状態機械や実行フローを導入していない。mainへのマージ・公開・再起動は実施しない。
