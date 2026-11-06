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
repository外の共有実体と中央添付キャッシュは移動しません。添付の基準はSessionの
agent作業ディレクトリ（未指定時はWorkspace root）、sharedはrepository単位です。
必須のLVKブロックを含む `AGENTS.md`、`CLAUDE.md`、`openwiki/`、`.openwikiignore`、
外部ツール設定は標準位置を維持します。旧配置・旧履歴の移行、書換え、互換探索は行いません。

LVKは[Easy Vibe Kanban](https://github.com/toby1123yjh/easy-vibe-kanban)と
[Vibe Kanban](https://github.com/BloopAI/vibe-kanban)を基にしています。
両プロジェクトの貢献と履歴を引き継ぎます。[Apache-2.0](LICENSE)ライセンスで、
帰属表示は[NOTICE](NOTICE)を参照してください。
