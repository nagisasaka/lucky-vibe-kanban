# Lucky Vibe Kanban（LVK）

**カンバンで作業を計画し、コーディングエージェントを動かして、その変更をレビューする。**

LVKは、Git worktree、複数Session、視覚的なエージェントWorkflow、ローカル統合、
OpenWikiによるリポジトリの記憶を備えたセルフホストの開発環境です。
手元のコンピューターや自分のサーバーで動かし、ノートPCを切断している間も作業を続けられます。
エージェントのアカウント、プロバイダー契約、モデル利用料金は別途必要です。

[English](README.md) · [简体中文](README.zh-CN.md) ·
[ソースコード](https://github.com/nagisasaka/lucky-vibe-kanban) · [Apache-2.0](LICENSE)

## 配布方法

GHCRを通じてDockerイメージのみを配布し、現在の対象はLinux amd64です。
このフォークではnpmパッケージを公開しません。旧npxランチャー、Tauri desktop shell、
上流のCloud・relayは廃止済みです。ブラウザーでLVKサーバーのURLへアクセスします。
外部Runnerによる実行はExecution Bridgeで引き続き利用できます。

公開配置はHTTPSとBasic／Auth0認証を使い、初回はロック状態です。
信頼できるチーム向けで、無制限の公開エージェントサービスではありません。
プロバイダーやアプリの認証はLVKログインとは別です。
データの永続化は、中断したプロセスの自動再開を保証しません。

## 利用案内

- [サーバー導入・証明書・バックアップ・復元](openwiki/operations/server-container.md)
- [初期認証・メンバー・復旧](openwiki/concepts/server-access.md)
- [プロバイダーログイン・profile・MCP設定](openwiki/integrations/agent-providers.md)
- [Issueから統合まで](openwiki/workflows/task-to-integration.md)と[自動マージ](openwiki/concepts/formal-integration.md)
- [リポジトリの記憶と正規Wiki保守](openwiki/operations/openwiki-maintenance.md)

## Claude WebからSkillとMCP接続先を導入する

配布ファイルが公開mainに反映された後、Claudeの有料プランで
**Customize → Plugins → Add → Add marketplace → Add from a repository** を開き、
`nagisasaka/lucky-vibe-kanban` を登録します。組織の利用制限がある場合は管理者の許可が必要です。
一覧から `lvk-orchestration` を追加し、Skillsに同名Skill、Connectorsに
`https://luckyvibekanban.com/mcp` のLVK接続先が表示されることを確認します。
ZIP作成・アップロード、別リポジトリ、公開ディレクトリへの申請は不要です。
この配布設定は既存の接続先・認証設定を変更しません。表示確認と実接続・認証の成功は別です。

Skillの編集元は `skills/lvk-orchestration/` だけです。Claude用の `skills/` は
Git直接導入用の生成物で、参照ファイルも含めて正本と同一内容を収録します。
正本更新後は `python3 scripts/claude-marketplace.py --sync` を実行し、生成物も同時にコミットします。
Claude配布内容を更新する際は既存Claude manifestのversionも上げます。
`python3 scripts/claude-marketplace.py --check --ref HEAD` はコミット済み配布ツリーを検証し、
専用CIも不一致を検出します。利用者のインストール時には生成処理を実行しません。

Marketplaceのsourceはリポジトリルート基準の `./plugins/lvk-orchestration/claude` です。
その中に既存manifest、`.mcp.json`、Skillと参照先が収まり、ルート外リンクは使いません。
仕様根拠：[公式の導入手順](https://support.claude.com/en/articles/13837440-use-plugins-in-claude)、
[Plugin構成](https://claude.com/docs/plugins/build)、
[sourceの解決規則](https://code.claude.com/docs/en/plugins/marketplace-reference)、
[Anthropicの同一リポジトリ内source例](https://github.com/anthropics/knowledge-work-plugins/blob/main/.claude-plugin/marketplace.json)。
CLI validateや配布ツリー検証の成功は、Claude Webでの実インストール成功を意味しません。
公開後のMarketplace登録・Skillとコネクタ表示は人間が確認してください。

## LVKを開発する

ツールチェーン、セットアップ、ビルド、検証、復旧は[開発ガイド](openwiki/operations/development.md)、
開発時の規則は[AGENTS.md](AGENTS.md)を参照してください。
配布設定は[deploy/server](deploy/server)、イメージの検証・公開は
[publish-server.yml](.github/workflows/publish-server.yml)が担当します。公開と配備は別の操作です。

初回のsource checkoutでは、指定ツールチェーンと`cargo-watch`を用意し、
`pnpm install --frozen-lockfile`、`pnpm run dev`の順に実行します。

READMEは人間がAIの支援を受けて保守します。運用詳細はAIが保守するOpenWikiに集約し、
統合済みsourceと照合します。source・tests・configurationを優先します。
LVKでは別の文書サイトを再作成せず、Wikiで知識を保守します。

## 名前と帰属

製品名は**Lucky Vibe Kanban**、略称は**LVK**です。現在の設定・共有リンクは
`LVK_*`と`.lvk/shared`を使用します。元のVibe Kanbanの保存先、`VK_*`設定、
ネイティブバイナリ名は変更していません。

LVK独自のrepo内管理ファイルは `.lvk/` に配置します。`integration-validation.json`、
`.gitignore`、LVK自体の開発用 `preview/` はGit共有できる設定です。ローカルの
`shared/` リンクと `attachments/` コピーだけを `.lvk/.gitignore` で除外し、
このignoreファイル自身は自己除外しません。既存の記述を保持し、必要な実行時除外を追記します。
`shared/tmp/` は同じ登録repositoryのWorkspace間の一時受け渡し、`cache/` は再生成可能な
再利用データ、`persistent/` はユーザーが残す成果物専用です。LVK内部の処理データは含みません。tmpはRun終了で消しません。
受け渡しが完了し、不要かつ利用中でなくなってから整理します。人間による一括整理も同条件です。
TTLや自動掃除は設けず、必要な成果は適切な保存先に残してください。
repository外の共有実体と中央添付キャッシュは移動しません。添付の基準はSessionの
agent作業ディレクトリ（未指定時はWorkspace root）、sharedはrepository単位です。
必須のLVKブロックを含む `AGENTS.md`、`CLAUDE.md`、`openwiki/`、`.openwikiignore`、
外部ツール設定は標準位置を維持します。
Wiki専用保守worktreeでは追跡設定を変えず、Gitローカルの `info/exclude` で
`.lvk/shared/` と `.lvk/attachments/` だけを除外します。このGit情報はworktree間で共有されますが、
sourceや他の `.lvk/` 設定は除外しません。通常Workspaceは `.lvk/.gitignore` を使います。
旧配置・旧履歴の移行、書換え、互換探索は行いません。

LVKは[Easy Vibe Kanban](https://github.com/toby1123yjh/easy-vibe-kanban)と
[Vibe Kanban](https://github.com/BloopAI/vibe-kanban)を基にしています。
両プロジェクトの貢献と履歴を引き継ぎます。[Apache-2.0](LICENSE)ライセンスで、
帰属表示は[NOTICE](NOTICE)を参照してください。
