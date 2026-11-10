---
type: architecture
title: ファイル・差分・プレビュー・端末の境界
description: Workspace 内のファイル・Wiki 閲覧、添付、差分配信、dev server preview、PTY、editor 操作で異なるパス・ホスト・寿命の契約を説明する。
tags: [workspace, files, wiki, diff, preview, terminal]
sources:
  - id: openwiki-source-f615018b2948c78d0299828c
    resource: repo://crates/executors/src/executors/claude/usage.rs
  - id: openwiki-source-bd15422ef1716eca23408f1f
    resource: repo://crates/git/src/lib.rs
  - id: openwiki-source-40ec9fc6ff784c4bb2d0c6e3
    resource: repo://crates/git/tests/diff_stats.rs
  - id: openwiki-source-a12b66a13b2ce47a85d54985
    resource: repo://crates/local-deployment/src/agent_process_registry.rs
  - id: openwiki-source-343f68138e6e2d3312d2774c
    resource: repo://crates/local-deployment/src/container.rs
  - id: openwiki-source-8a1832c7fd24c1e7222a8d0a
    resource: repo://crates/local-deployment/src/pty.rs
  - id: openwiki-source-7464a2d1db8e85c09420cf28
    resource: repo://crates/server/src/middleware/model_loaders.rs
  - id: openwiki-source-87ddf3d108f228e9ef8f8964
    resource: repo://crates/server/src/remote_mcp/dev_server.rs
  - id: openwiki-source-a9b6d4be264787a78357c9d6
    resource: repo://crates/server/src/routes/config.rs
  - id: openwiki-source-ecfa47de2e3842c785820c9b
    resource: repo://crates/server/src/routes/integrations/mod.rs
  - id: openwiki-source-b393e75029374eaf9cba5f7d
    resource: repo://crates/server/src/routes/integrations/runtime.rs
  - id: openwiki-source-65c0f36670a161dc05040e69
    resource: repo://crates/server/src/routes/mod.rs
  - id: openwiki-source-231241ac3c842dcd67525890
    resource: repo://crates/server/src/routes/openwiki.rs
  - id: openwiki-source-f178fbdd0db19291b533ad6c
    resource: repo://crates/server/src/routes/project_utilities.rs
  - id: openwiki-source-c1a5c9bda8d34af222d811c1
    resource: repo://crates/server/src/routes/repo.rs
  - id: openwiki-source-640a4edfdcc7be664fc62f5e
    resource: repo://crates/server/src/routes/shared_storage_http_tests.rs
  - id: openwiki-source-f1f6d741731d4c0b4e9e6cac
    resource: repo://crates/server/src/routes/shared_storage.rs
  - id: openwiki-source-7c1b10f112cb99ba82426b5a
    resource: repo://crates/server/src/routes/terminal.rs
  - id: openwiki-source-75b2a3e21f7d8cbca4f8565a
    resource: repo://crates/server/src/routes/workspaces/attachments.rs
  - id: openwiki-source-bdb4dae5722fe76ec5e44001
    resource: repo://crates/server/src/routes/workspaces/create.rs
  - id: openwiki-source-d13a9ab0f87dbc81d0cfe1b4
    resource: repo://crates/server/src/routes/workspaces/dev_server.rs
  - id: openwiki-source-37945a97e2b18bbb762fafc9
    resource: repo://crates/server/src/routes/workspaces/files.rs
  - id: openwiki-source-303d74f74def684d4690dea9
    resource: repo://crates/server/src/routes/workspaces/git.rs
  - id: openwiki-source-6c4d6d22a9d50e0def61c50d
    resource: repo://crates/server/src/routes/workspaces/integration.rs
  - id: openwiki-source-6459ca8fb9da2218e17b3ac9
    resource: repo://crates/server/src/routes/workspaces/mod.rs
  - id: openwiki-source-af328071d51bff75b5246d3b
    resource: repo://crates/server/src/routes/workspaces/usage.rs
  - id: openwiki-source-5590f47e4cee001a35a42117
    resource: repo://crates/server/src/routes/workspaces/wiki.rs
  - id: openwiki-source-42c7cffe07ec2d2c0ed4cc9d
    resource: repo://crates/server/src/routes/workspaces/workspace_summary.rs
  - id: openwiki-source-66c6304230dff031e0d0b855
    resource: repo://crates/services/src/services/config/editor/mod.rs
  - id: openwiki-source-cd98b68683664db6e92e6010
    resource: repo://crates/services/src/services/config/versions/v7.rs
  - id: openwiki-source-379c8cd3e1e9932f9adda9ee
    resource: repo://crates/services/src/services/container.rs
  - id: openwiki-source-81dd91cf7cbed81d1fcd4773
    resource: repo://crates/services/src/services/diff_stats_tests.rs
  - id: openwiki-source-b04f793d57fd8b570d4984e3
    resource: repo://crates/services/src/services/diff_stream.rs
  - id: openwiki-source-4294e32943918c50b31813b6
    resource: repo://crates/services/src/services/file.rs
  - id: openwiki-source-ead147478bd848437cb020e7
    resource: repo://crates/services/src/services/wiki.rs
  - id: openwiki-source-3bef0d0bfd4066b26b945f6c
    resource: repo://crates/services/src/services/wiki/openwiki.rs
  - id: openwiki-source-c128bdfa7a156621b004df06
    resource: repo://crates/services/src/services/workspace_usage.rs
  - id: openwiki-source-00394af5c4b9d56480285bf6
    resource: repo://crates/utils/src/lvk_directory.rs
  - id: openwiki-source-0b2eb3f83fc86f6a66a94a46
    resource: repo://crates/utils/src/shared_storage.rs
  - id: openwiki-source-965b490de6f8364cb7695944
    resource: repo://crates/workspace-manager/src/workspace_manager.rs
  - id: openwiki-source-89d0fb22262f0a0ac1df7d1e
    resource: repo://deploy/server/server.mjs
  - id: openwiki-source-5e4c1802d2c1ee81cad7ac45
    resource: repo://packages/ui/src/components/IssueWorkspaceCard.tsx
  - id: openwiki-source-bb23b715172e6051c10e1afc
    resource: repo://packages/ui/src/components/ReadOnlyLinkPlugin.tsx
  - id: openwiki-source-1383d6dc4e3d9d2ea475e735
    resource: repo://packages/ui/src/lib/chat-code-highlighting.ts
  - id: openwiki-source-69854837467c3beec23ecb8d
    resource: repo://packages/web-core/src/features/wiki/model/wikiNavigation.ts
  - id: openwiki-source-0cd94f38cc1a16e284fe2b1c
    resource: repo://packages/web-core/src/features/wiki/ui/WorkspaceWikiPanel.tsx
  - id: openwiki-source-cd106619cfd7af68eb9464fc
    resource: repo://packages/web-core/src/features/workspace-chat/ui/ConversationListContainer.tsx
  - id: openwiki-source-a045ed78b54878edd07211c7
    resource: repo://packages/web-core/src/features/workspace-chat/ui/ExecutionInspectionPanel.tsx
  - id: openwiki-source-3f4f76b473cff31732d54424
    resource: repo://packages/web-core/src/features/workspace-chat/ui/WorkspaceTokenSummary.tsx
  - id: openwiki-source-714cddfb6b8044e60077cdf6
    resource: repo://packages/web-core/src/features/workspace-files/model/chatFileLink.ts
  - id: openwiki-source-10a02a3880ad701da1e4c49f
    resource: repo://packages/web-core/src/features/workspace-files/model/codeHighlightLines.ts
  - id: openwiki-source-18dc5170d4dade4bcfbc64d8
    resource: repo://packages/web-core/src/features/workspace-files/model/useWorkspaceFileContent.ts
  - id: openwiki-source-0f1df48871fe6f036c219dff
    resource: repo://packages/web-core/src/features/workspace-files/model/useWorkspaceFilePreviewResolver.ts
  - id: openwiki-source-1be0c4423a1ed260c67b498f
    resource: repo://packages/web-core/src/features/workspace-files/model/workspaceFileRawUrl.test.ts
  - id: openwiki-source-e26b48aa743da73944d9c0b1
    resource: repo://packages/web-core/src/features/workspace-files/ui/viewers/CodeTextViewer.tsx
  - id: openwiki-source-a37c8f73174e9393571f496d
    resource: repo://packages/web-core/src/features/workspace-files/ui/viewers/MarkdownFileViewer.tsx
  - id: openwiki-source-48d5859237df58130423c433
    resource: repo://packages/web-core/src/features/workspace-files/ui/WorkspaceFilePreviewHeader.tsx
  - id: openwiki-source-ece894e6e087af564c07e38d
    resource: repo://packages/web-core/src/features/workspace-files/ui/WorkspaceFilePreviewPanel.tsx
  - id: openwiki-source-06868af84d952b8087cf3ebb
    resource: repo://packages/web-core/src/i18n/locales/ja/common.json
  - id: openwiki-source-4da1fcbefae4945006795406
    resource: repo://packages/web-core/src/pages/kanban/ProjectKanban.tsx
  - id: openwiki-source-5b956d3d9d7f17c236d42ebb
    resource: repo://packages/web-core/src/pages/kanban/ProjectUtilities.tsx
  - id: openwiki-source-9b9697ea8eafb71c1ca35e40
    resource: repo://packages/web-core/src/pages/workspaces/ChangesPanelContainer.tsx
  - id: openwiki-source-36ce95fe7094b2e05ad44d39
    resource: repo://packages/web-core/src/pages/workspaces/GitPanelContainer.tsx
  - id: openwiki-source-6559800b59b62e086689d27d
    resource: repo://packages/web-core/src/pages/workspaces/RightSidebar.tsx
  - id: openwiki-source-39d390b46b89f38f831e363e
    resource: repo://packages/web-core/src/pages/workspaces/WorkspaceFilesSurfaceContainer.tsx
  - id: openwiki-source-0894249b732085a9800940bd
    resource: repo://packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx
  - id: openwiki-source-9231a80ee56432d71077b70c
    resource: repo://packages/web-core/src/pages/workspaces/WorkspacesSidebarContainer.tsx
  - id: openwiki-source-235a3eb75f22fdecdbd66a8f
    resource: repo://packages/web-core/src/shared/components/EnvironmentPanel.tsx
  - id: openwiki-source-fac63ea2f2df3069fa2ed5c9
    resource: repo://packages/web-core/src/shared/components/SharedStorageBreakdown.tsx
  - id: openwiki-source-24d3b93959be3c330f4ca02c
    resource: repo://packages/web-core/src/shared/components/tasks/Toolbar/GitOperations.tsx
  - id: openwiki-source-867b74d482ff6cc365176503
    resource: repo://packages/web-core/src/shared/components/TerminalSection.tsx
  - id: openwiki-source-93851bff2ebf05ba57ca3277
    resource: repo://packages/web-core/src/shared/components/UtilitySidebar.tsx
  - id: openwiki-source-10caf2340da819f813f5ea62
    resource: repo://packages/web-core/src/shared/components/WYSIWYGEditor.tsx
  - id: openwiki-source-6d0dfae3c1582e1af83405d4
    resource: repo://packages/web-core/src/shared/dialogs/settings/settings/GeneralSettingsSection.tsx
  - id: openwiki-source-8c6e3fc2f0060d4f6b2a9810
    resource: repo://packages/web-core/src/shared/dialogs/settings/settings/SharedStorageSettings.tsx
  - id: openwiki-source-b30612082181a81aa92d89df
    resource: repo://packages/web-core/src/shared/hooks/useUserSystemController.ts
  - id: openwiki-source-97cd8794805cef30e1c2f047
    resource: repo://packages/web-core/src/shared/hooks/useWorkspaces.ts
  - id: openwiki-source-2b7aa45c584402b062848c47
    resource: repo://packages/web-core/src/shared/lib/claudeQuotaView.ts
  - id: openwiki-source-ea8e692dfd4142379f167b66
    resource: repo://packages/web-core/src/shared/lib/externalEditors.ts
  - id: openwiki-source-4fb8a1b1995ea82c4cf31ea2
    resource: repo://packages/web-core/src/shared/lib/mainTerminalRepo.ts
  - id: openwiki-source-09147e83e2e70cb032ece9e7
    resource: repo://packages/web-core/src/shared/providers/TerminalProvider.tsx
  - id: openwiki-source-3b6f5885b9d50925c82267dd
    resource: repo://packages/web-core/src/shared/providers/WorkspaceProvider.tsx
  - id: openwiki-source-5ef837b2bc54d286b3dedd8f
    resource: repo://packages/web-core/src/shared/stores/useUiPreferencesStore.ts
  - id: openwiki-source-7156bd32d206a84b272e44b5
    resource: repo://packages/web-core/src/shared/stores/useWorkspaceDiffStore.ts
  - id: openwiki-source-bf19bdc211e26b0fb155fc08
    resource: repo://tests/chat/file-links.md
  - id: openwiki-source-ba7e6a69f93643706f63da9e
    resource: repo://tests/chat/file-links.native.mjs
  - id: openwiki-source-d70e2f551bdb141bf2cd79a6
    resource: repo://tests/chat/mobile-navigation.native.mjs
  - id: openwiki-source-f806f5d600e6f619c7c88371
    resource: repo://tests/pets/integration.mjs
  - id: openwiki-source-be2dcdcefb33abee0a49e75a
    resource: repo://tests/server-access/preview-static.mjs
  - id: openwiki-source-c6d955097c4a7a9e7b1873ee
    resource: repo://tests/server/specs/workspace-search.spec.ts
  - id: openwiki-source-dcac74ef871fd02e6315fa93
    resource: repo://tests/workflow/specs/diff-tree.spec.ts
generated: { by: "codex", at: "2026-11-10T20:48:58.515Z" }
verified:
  - by: openwiki/0.7.2
    at: 2026-11-10T20:48:58.515Z
---

# ファイル・差分・プレビュー・端末の境界

これらの機能は同じ [Workspace](../concepts/workspace.md) の成果物を扱うが、実ファイルの閲覧、Git との差分、アプリの実行、shell の操作は別の経路である。画面の操作手順と、その操作が成立する実装契約を以下にまとめる。

## Issue 番号から Workspace を探す

Workspace 一覧の検索欄には名前・branch のほか `106` や `LOCAL-106` を入力できる。対象が archive 済みなら Archived に切り替え、見つからない場合は Project / PR filter と関連情報の取得状態も確認する。同じ Issue の複数 Workspace や別 Project の同じ識別子が並ぶ場合があるため、番号だけで作業先を断定しない。[検索の適用](../../packages/web-core/src/pages/workspaces/WorkspacesSidebarContainer.tsx#L408-L495)・[複数結果と archive の回帰](../../tests/server/specs/workspace-search.spec.ts#L102-L131)

番号は部分一致であり、関連情報の取得前には名前・branch で探せる。表示件数との関係、再取得と失敗の意味は [Workspace 一覧検索の契約](../concepts/workspace.md#一覧検索と-issue-識別子)を参照する。対象を選んだ後は、以下の用途・履歴・ファイルの状態を確認する。

## 実行専用 Workspace の確認

実行中の inspection は [用途が execution_only の Workspace](../concepts/workspace.md#操作用途と実行所有者)を表示する。通常 chat composer の代わりに、owner kind/run、状態、公開結果、ファイルの有無、保存済み Session の選択、所有者の Stop を示す。新規 chat・Session・queue・executor 選択は作らず、現在の所有者が出している承認・質問への応答を残す。Wiki実行中には追加指示をsteerできる。自動マージは通常会話の専用担当が進める。実行と cleanup 完了後は通常 composer に戻り、所有者の履歴は残る。[inspection UI](../../packages/web-core/src/features/workspace-chat/ui/ExecutionInspectionPanel.tsx)

現行の自動マージは同じ割当Sessionの通常会話で候補・検証・Wiki・公開要求を進める。旧専用介入欄やgate承認ではなく、その会話へ質問や追加指示を送る。公開要求の受付とhostによるmain反映は別に確認する。[自動マージの正本](../concepts/formal-integration.md)。

`GET /api/workspaces/<id>/usage` と executions 一覧は既存 owner 記録から状態を投影する。自動マージ の Git 公開、Bootstrap の Workflow 完了、Sync の保存済み結果はそれぞれの正本から読み、Generator や AgentRun の成功を公開済みと推測しない。Sync の AgentRun が終端でも repository owner が残る間は `finalizing` である。成功 no-op は新しい Wiki commit がない成功として表示する。[状態の由来](../../crates/server/src/routes/workspaces/usage.rs)

Stop は 自動マージ の cancel、Bootstrap の所有フロー、Sync の監査付き Cancel へ委譲する。停止可能 phase を再検証し、Wiki publication 開始後は取り消さず結果の確定を待つ。終端または停止要求済みなら重複 Stop は無操作となる。[停止の境界](../../crates/server/src/routes/workspaces/usage.rs)。失敗・回復待ちの意味は [自動マージ](../concepts/formal-integration.md)と [Wiki 保守](openwiki-maintenance.md)に置く。

ファイル・Wiki・添付などの読取は `container_for_inspection` を経由する。lifecycle lock 内で状態を再読込し、execution_only または archived なら既存パスだけを使い、cleanup 済みや未準備の worktree を再作成しない。未 archive の interactive 環境は ensure 経路を保持する。表示に伴う復元や script 起動を追加する変更は、この用途の違いを守る必要がある。[共通の読取入口](../../crates/services/src/services/container.rs)

## 累積トークンを確認する

チャット末尾の「Workspace累積トークン」を開くと、入力・出力、入力の内数であるキャッシュ、出力の内数である推論、内訳取得 thread 数、最終更新を読める。対象は選択 Session だけでなく Workspace 全体であり、各 run の既存時間表示は別に残る。[末尾への配置](../../packages/web-core/src/features/workspace-chat/ui/ConversationListContainer.tsx#L1027-L1031)・[表示](../../packages/web-core/src/features/workspace-chat/ui/WorkspaceTokenSummary.tsx#L25-L63)

React Query は Workspace ID を key に `/api/workspaces/{id}/tokens` を30秒間隔で再取得する。未取得は「不明」、取得済みでも `partial` は「一部のみ」と表示する。取得エラー時は「取得失敗」を出し、保持済み data があっても詳細内訳を表示しない。0 は不明に置換しない。[取得と分岐](../../packages/web-core/src/features/workspace-chat/ui/WorkspaceTokenSummary.tsx#L12-L60)・[日本語](../../packages/web-core/src/i18n/locales/ja/common.json#L1714-L1726)

`/tokens` は保存済み集計の読取で、同じファイルに実装される `/usage` の実行所有者 inspection / Stop とは別機能である。欠損を補うために AgentRun を起動せず、UI・stdio MCP・外部 MCP は共通集計を使う。[読取 handler](../../crates/server/src/routes/workspaces/usage.rs#L319-L328)。取得範囲と最大値の寿命、未報告の子 thread や料金推定の限界は [Workspace 累積トークン](../concepts/workspace.md#workspace-累積トークン)、操作入口は [stdio](../integrations/agent-providers.md#外部-mcp-client--lvk)と [外部 MCP](../integrations/remote-access.md#workspace-の報告済みトークンを読む)で確認する。

## Project の Todo を確認する

Workspace の Todo panel は作業木内のファイルではなく、現在の Card link が示す Project の共有メモを読む。未関連なら Project を選ぶ。[所属・保存・更新競合](../concepts/project-todos.md)を確認し、メモの完了を実行終了や Card Done と取り違えない。チャットを残した panel 切替の配置は [Web の状態管理](../architecture/web-and-sync.md)を参照する。

<a id="workspace-のファイル閲覧"></a>

## Repo 内ファイルを読む

files API は tree / directory / content / raw の GET を公開する。repo_id は WorkspaceRepo membership で解決し、その Workspace の container_ref/保存済みworktree_name（未設定時はrepo.name） を root とする。任意の filesystem path を読む API ではない。[router](../../crates/server/src/routes/workspaces/files.rs)・[root の解決](../../crates/server/src/routes/workspaces/files.rs)

パスは slash を正規化し、NUL、絶対パス、Windows drive、親への .. を拒否する。さらに canonicalize した最終 target が Repo root 内にあるか確認し、外への symlink は NotFound とする。directory の列挙でも root 外へ解決される entry を省く。[パス検証](../../crates/server/src/routes/workspaces/files.rs)・[列挙](../../crates/server/src/routes/workspaces/files.rs)・[traversal / symlink test](../../crates/server/src/routes/workspaces/files.rs)

directory は最大2,000 entries、text preview は512 KiBまでで truncated を返す。content は Text / Image / Binary / Unsupported に分かれ、NUL や不正 UTF-8 も binary 判定に使う。先頭だけの preview を file 全体と解釈しない。[上限](../../crates/server/src/routes/workspaces/files.rs)・[content 分類と読み取り](../../crates/server/src/routes/workspaces/files.rs)

raw は streaming 応答で no-store と nosniff を付ける。明示された raster image MIME のみ inline とし、それ以外は application/octet-stream / attachment で返す。HTML や SVG を raw URL で main UI の実行 content に変えない。[raw 応答](../../crates/server/src/routes/workspaces/files.rs)・[MIME policy](../../crates/server/src/routes/workspaces/files.rs)

Content-Disposition は inline 画像にも attachment にも、正規化された basename の ASCII `filename` fallback と正確な UTF-8 `filename*` を付ける。大文字小文字・拡張子・日本語・空白をURL末尾の raw に置き換えず保存名へ伝える。fallbackでは非ASCII・quote・backslash・percent等を安全な文字に置換し、UTF-8側はspaceを%20、asteriskを%2Aにencodeする。stream本体とMIME/認証/所属/path境界は変えない。[応答の組立て](../../crates/server/src/routes/workspaces/files.rs#L247-L301)・[header生成](../../crates/server/src/routes/workspaces/files.rs#L656-L679)

実file fixtureはpreview上限超のbinary bytes、NUL/CRLF、不正UTF-8、画像/HTML/SVGとheader injection、認証・symlinkを検査する。browserの保存ダイアログや元の報告再現は未確認で、headerの正しさを実browser測定と混同しない。[raw回帰](../../crates/server/src/routes/workspaces/files.rs#L922-L1080)・[raw応答と保存名](../../crates/server/src/routes/workspaces/files.rs)

raw URL は local のものをそのまま使い、退役した remote host ID または host-scoped URL は拒否する。host ID なしの外部 URL は保持する。[URL test](../../packages/web-core/src/features/workspace-files/model/workspaceFileRawUrl.test.ts)

## チャットから Repo 内ファイルを開く

### モバイルのツリーとpreviewの往復

Files surface は選択targetのオブジェクト同一性で戻る操作を管理する。Filesへ戻ると現在のtargetだけを dismissed として保持し、別のopen要求で新しいtargetが来ればpreviewを再表示する。このため同じファイルへの会話リンクを再度開いても表示でき、無関係な再renderだけでツリーからpreviewへ戻らない。desktopの左右並列表示は同じ選択を使う。[surface](../../packages/web-core/src/pages/workspaces/WorkspaceFilesSurfaceContainer.tsx#L24-L97)・[実アプリ回帰](../../tests/chat/mobile-navigation.native.mjs)

### 読み取り専用コードの色分け

`CodeTextViewer` は共有chat用Prism grammarとテーマclassを再利用する。文書全体の改行を正規化してtokenizeしてから行へ分割し、複数行コメント・templateと入れ子tokenの意味を保つ。Reactのtext/spanとして描画し、入力HTMLを実行せず、editorや編集機能を追加しない。行番号・選択行の中央scroll・横スクロールは既存viewerの責務のままである。[tokenの行変換](../../packages/web-core/src/features/workspace-files/model/codeHighlightLines.ts#L17-L69)・[描画](../../packages/web-core/src/features/workspace-files/ui/viewers/CodeTextViewer.tsx#L14-L74)

未対応grammar、NUL、共有上限16,384文字超、4,096文字超の行、token/segment予算4,096または深さ32の超過、tokenizer失敗では受け取った本文全体をplain行で返す。色分けの予算はbackendの512 KiB preview上限とは別で、色分けを諦めても本文を切り詰めない。content/language単位でmemoizeし、chat側の共有実装は変更しない。[fallback](../../packages/web-core/src/features/workspace-files/model/codeHighlightLines.ts#L11-L69)・[共有上限](../../packages/ui/src/lib/chat-code-highlighting.ts#L5-L23)

[model/component単体](../../packages/web-core/src/features/workspace-files/model/codeHighlightLines.test.ts)と[Chromium回帰](../../tests/file-viewer/syntax.test.mjs)は複数行grammar、安全なHTML風文字列、plain fallback、390/1440pxのlight/dark、行120へのjumpと縦横scrollを確認する。Chromium fixtureは実componentに合成本文を渡す試験であり、認証済みWorkspaceからの実API取得試験とは区別する。

チャットの Markdown file link は、現在の Workspace / Repo に属するファイルを既存 preview で読むための参照である。`[source](src/example.ts:120)` を開くと file panel が選択される。リンクを別の Workspace の会話に移したときも、解決基準は表示先の Workspace である。本文 renderer から panel までの責務は [Web の接続構造](../architecture/web-and-sync.md#チャットのファイルリンクを既存-panel-へ渡す)にまとめる。[resolver の入力](../../packages/web-core/src/features/workspace-files/model/useChatFileLink.ts#L7-L23)

相対パスは Repo 基準で、`./` と Repo 名 prefix を扱う。Repo が一つなら省略できるが、複数 Repo で所属を一意に解決できなければリンクを無効のままにし、推測して開かない。絶対形式は現在の `container_ref/保存済みworktree_name（未設定時はrepo.name）/` 配下だけを repo-relative target に変換する。Windows 形式も slash を正規化して同じ条件を使う。元の host checkout の `Repo.path` へ fallback しない。[チャットの変換](../../packages/web-core/src/features/workspace-files/model/chatFileLink.ts#L36-L77)、[共通の Repo 解決](../../packages/web-core/src/features/workspace-files/model/useWorkspaceFilePreviewResolver.ts#L77-L157)

行指定は `:line[:column]`、`#Lline`、`#Lline-Lend` 等を受け付けるが、保存する位置は正の安全な整数の**開始行だけ**であり、column や範囲の選択を約束しない。行 suffix を外してから URL decode を一度だけ行うため、encoded `#` / `:` はファイル名として残る。空白は percent encoding または `[label](<path with spaces:2>)` を使える。後者の transformer は local resolver がある read-only editor で有効になる。[位置と decode 順序](../../packages/web-core/src/features/workspace-files/model/chatFileLink.ts#L21-L35)、[angle destination](../../packages/web-core/src/shared/components/WYSIWYGEditor.tsx#L547-L566)

text/code viewer は選択行に `aria-current=location` を付けて中央へスクロールする。Markdown は行指定がある場合にソース表示へ切り替え、指定なしでは従来の Markdown 表示を使う。描画済み本文の位置をソース行と扱わないための選択である。存在しない行や backend が切り詰めた範囲外には選択行 DOM がなく、追加読込や最終行への補正はしない。[viewer の分岐](../../packages/web-core/src/features/workspace-files/ui/WorkspaceFilePreviewPanel.tsx#L101-L121)、[行の選択](../../packages/web-core/src/features/workspace-files/ui/viewers/CodeTextViewer.tsx#L17-L48)

空の値、通常の `#section`、network path、親参照、control character、不正な percent encoding、未処理の query / fragment、現在の root 外の絶対パスや任意 URI scheme は local resolver が拒否する。HTTPS は plugin が外部リンクとして扱い、HTTP や `file://` は有効化しない。local action がなければ `href` を除き `aria-disabled=true` とする。[入力の検査](../../packages/web-core/src/features/workspace-files/model/chatFileLink.ts#L21-L55)、[リンクの有効状態](../../packages/ui/src/components/ReadOnlyLinkPlugin.tsx#L20-L58)

フロントエンドで解決できたことは、ファイルの存在や読取許可の証明ではない。target の Workspace / Repo / path を既存 content API に渡し、欠落・symlink 逸脱等の取得失敗は既存 preview error と Retry を表示する。最終的な membership と canonical-root 検査は [Repo 内ファイルの読取境界](#repo-内ファイルを読む)のままである。[取得](../../packages/web-core/src/features/workspace-files/model/useWorkspaceFileContent.ts#L13-L26)、[失敗表示](../../packages/web-core/src/features/workspace-files/ui/WorkspaceFilePreviewPanel.tsx#L76-L102)

### ファイルリンクの検証範囲

[resolver の unit test](../../packages/web-core/src/features/workspace-files/model/chatFileLink.test.ts)は相対・現在 root・Windows・行 suffix・decode・曖昧な Repo・拒否入力を扱う。[component と実 backend の E2E](../../tests/chat/file-links.e2e.mjs)は合成会話を使い、内容表示、Enter、空白・日本語、Markdown source、欠落・symlink、content/raw の拒否を検査する。これだけでは通常 layout の切替を保証しないため、`--native` は seed した保存済み会話を通常 Workspace route で開き、リンククリックから実 content API、native panel、120 行目の選択と viewport 内への scroll まで確認する。[native 試験](../../tests/chat/file-links.native.mjs#L5-L81)

[実装時の検証記録](../../tests/chat/file-links.md)には開発時の成功と失敗・修正の記録がある。今回の Sync は凍結 source `271f62441d4a7ecb7f43ff0cd77e02f0371c762f` について renderer、layout、resolver、viewer、backend と試験定義を静的に照合した。自動マージ は明示的な検証省略と凍結入力に記録されており、この Sync で製品テストを再実行したことや過去の無関係な説明を再検証したことを意味しない。agent による会話生成、実 Basic/Auth0 login、モバイル layout はこの native 試験の検証範囲外である。

## Wiki の本文・ツリーと表示元を確認する

Wiki は `openwiki/` の読取専用表示に統一され、repository 選択・検索・ページツリーと本文を持つ。旧 `.llm-wiki` の形式選択はなく、残っている旧ファイルも読み替えない。OpenWiki の作成・Sync は repository memory settings から行い、reader は初期化や破損修復をしない。[形式と操作の分離](../../packages/web-core/src/features/wiki/ui/WorkspaceWikiPanel.tsx)

表示は **Current workspace**、すなわち選択 repository の現在の worktree であり、未公開の変更も含む。本文 header の repository、Workspace 名、branch、ページ path を確認する。別 worktree の成果物や未公開 Wiki を「target branch へ公開済み」と取り違えない。ファイル変更後は `Reload Wiki files` で再取得する。[表示元と再読込](../../packages/web-core/src/features/wiki/ui/WorkspaceWikiPanel.tsx)、[API の snapshot source](../../crates/server/src/routes/workspaces/wiki.rs)

ツリーは公開された Wiki snapshot のページだけから組み立て、index を入口にする。検索は本文、タイトル、summary、tags 等へ空白区切りの全 term を適用する。OpenWiki の階層内相対リンクや `/openwiki/...`、`[[wikilink]]` は既知ページへ解決するが、Wiki root 外の source path を任意ファイル読取へ変換しない。ページ切替はパス単位であり、リンクの fragment を保持した節スクロールまでこの resolver が保証するわけではない。[ツリーと検索](../../packages/web-core/src/features/wiki/model/wikiNavigation.ts)、[リンク解決](../../packages/web-core/src/features/wiki/model/wikiNavigation.ts)

本文は既存の `MarkdownPreview` を再利用する。desktop では Wiki アイコンまたは右サイドバーの `Open Wiki` で右メインペーンに開き、既定では左に会話を残す。同じアイコンで閉じると右ペーンは未選択になり、以前の Changes / Logs へ戻す専用の復帰操作はない。他のペーンへ直接切り替えることもできる。表示切替で [Session / AgentRun](../concepts/session-and-agent-run.md) を作り直すものではない。[renderer](../../packages/web-core/src/features/wiki/ui/WorkspaceWikiPanel.tsx#L339-L352)・[共通ペーン操作](../../packages/web-core/src/shared/stores/useUiPreferencesStore.ts#L561-L607)

保存済み設定で会話が非表示なら、Wiki を開くだけでは表示に戻らない。共通チャット表示操作、または右ペーンを閉じる操作で戻す。右サイドバーを隠していた場合も Wiki が強制的に開き直すことはない。モバイルでは Wiki タブからツリーと本文を開き、会話には Chat タブで戻る。これは desktop の右ペーン選択を解除する操作とは異なる。保存値の由来を識別できない制約、共通 layout の補正、下書き保持と試験範囲は [通常ペーン操作と保存済み設定](../architecture/web-and-sync.md#通常ペーン操作と保存済み設定)、共有 reader の責任は [Wiki の状態管理](../architecture/web-and-sync.md#wiki-のナビゲーションと本文を一つの状態で動かす)にまとめる。[チャット表示操作](../../packages/web-core/src/shared/stores/useUiPreferencesStore.ts#L534-L552)・[閉じた後の layout](../../packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx#L199-L210)・[モバイル入口](../../packages/web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx#L175-L185)

adapter は `openwiki/` 以下の階層 Markdown を読み、dot-prefixed entry と `INSTRUCTIONS.md` をページから除く。root・子 entry の symlink を拒否し、index を含め最大2,000ページ、1ページ512 KiB、合計16 MiBを超えれば snapshot 全体をエラーにする。切り詰めた Wiki を正常応答にはしない。frontmatter は optional だが、ある場合は正しい YAML mapping を要求する。Claims 解釈・設定書込・初期化・修復はしない。[adapter](../../crates/services/src/services/wiki/openwiki.rs)、[安全な読取](../../crates/services/src/services/wiki.rs)、[非変更・symlink・容量の test](../../crates/services/src/services/wiki/openwiki.rs)

通常Repoはmembershipを要求する。DirectFolderは旧Repo行が残っていてもWorkspace Repo Wikiの解決を拒否し、旧repo_id=workspace.idのfallbackを使わない。旧機能の終了と Card の保存文脈は [Card context](../concepts/card-context-and-llm-wiki.md)、生成物の公開条件は [OpenWiki 保守](openwiki-maintenance.md)を参照する。[scope の解決](../../crates/server/src/routes/workspaces/wiki.rs)

<a id="添付の実体と-workspace-配置"></a>

## Session の添付

Session の添付は local File を作業先へ渡す機能である。Board 上の file 参照との関係は [Issue の共同作業](../concepts/issue-collaboration.md) を参照する。

添付 upload は保存レコードを作成し、Session の作業先へ file ID でコピーする。raw 添付の取得は Session が Workspace に属するか確認し、保存済みagent_working_dirに依存せずWorkspace root配下の .lvk/attachments を使う。最終 canonical path が添付ディレクトリから外れれば拒否する。Repo file viewer と添付の基準 root は同じとは限らない。[upload](../../crates/server/src/routes/workspaces/attachments.rs)・[添付のパス](../../crates/server/src/routes/workspaces/attachments.rs)・[Session scope](../../crates/server/src/routes/workspaces/attachments.rs)

配置では親 `.lvk` を通常directoryとして検証し、追跡済み添付先、親リンク、既存先の内容不一致を拒否する。ignoreは `.lvk/.gitignore` の実行時子だけに集約し、添付子の `.gitignore` を別途作らない。旧履歴・旧prefix互換や旧添付の移動はしない。中央cache内のDB file_pathと作業先の相対pathを混同しない。[配置と回帰](../../crates/services/src/services/file.rs)・[名前空間](../../crates/utils/src/path.rs)

Workspace 作成は明示された attachment_ids を関連づけ、linked_issue があれば local の Issue にリンクする。Cloud 添付を取得する経路ではない。[作成処理](../../crates/server/src/routes/workspaces/create.rs)

### 保存と検証済み配置を区別する

MCPからはProjectへ保存した添付IDを、明示したWorkspace/Sessionへ配置できる。保存・配置だけではRunを開始しない。添付はWorkspace共有であり、Sessionを指定しても秘密領域にはならない。Remoteのhost fileParamsと両transportのinline入力、上限・URL・receiptの契約は [MCPで素材ファイルを渡す](../integrations/remote-access.md#mcpで素材ファイルを渡す)に集約する。

FileServiceはUnicode名を保ち、hashだけで別名・別MIMEを誤って再利用せず、同じbytesでも名前が違えば別レコードを保持する。配置は全IDの実在を確認し、保存元と既存先のサイズ/SHA-256を検証する。欠損・改変・symlinkを無視せず、既存改変先を上書きしない。新規配置は一時ファイルからno-clobberで公開する。通常UIのコピー経路も同じサービスを使い、続行時もWorkspace共通rootへ新添付を配置してからRunを作り、旧Session保存pathと旧ファイルを移動しない。[保存](../../crates/services/src/services/file.rs#L89-L175)・[コピー](../../crates/services/src/services/file.rs#L256-L380)・[実行前検証](../../crates/server/src/routes/sessions/mod.rs#L449-L465)

複数配置は各要素のsaved/associated/verifiedと全体all_verifiedを分ける。一部失敗なら成功分は残るが実行は始めない。未知ID・Project/Workspace/Session不一致を副作用前に拒否する。Project参照のある未配置ファイルは孤立掃除から保護され、参照がある限り無期限保持となる。per-file上限は総容量制限ではなく、TTL・定期GC・総保存quotaはない。[配置](../../crates/server/src/routes/attachment_transfer.rs#L242-L310)・[保持](../../crates/db/migrations/20261029160000_project_attachments.sql)・[清掃の再確認](../../crates/services/src/services/file.rs)

## 差分は現在の比較基準から再構成する

Git diff は専用の local WebSocket 経路を持つ。diff stream は現在の base commit と worktree から差分を計算し、Repo ID と複数 Repo 用 prefix を付けて replace patch を配信する。WorkspaceRepo の target_branch が変わったら base を再計算して stream を reset する。[API](../../crates/server/src/routes/workspaces/git.rs)・[snapshot](../../crates/services/src/services/diff_stream.rs)・[target 変更](../../crates/services/src/services/diff_stream.rs)

archive 済みで worktree が削除済みまたは存在しない場合は、保存された branch と base commit の差分を一度配信し、Ready を返す。watcher や worktree 再作成は行わない。これは削除前の未 commit ファイルの保存ではない。[snapshot の選択](../../crates/local-deployment/src/container.rs#L1836-L1849)・[branch snapshot](../../crates/services/src/services/diff_stream.rs#L169-L196)。履歴と branch を残す [手動 archive](../concepts/workspace.md#archive削除期限-cleanup)と、明示的な再開を区別する。

stats-only と累積本文容量の上限により、diff の old/new content を省くことがある。本文がないことと変更がないことを同一視しない。追加・削除行数は可能なら省略前に計算して保持する。[省略の契約](../../crates/services/src/services/diff_stream.rs) レビュー後に変更を統合する手順は [Task から統合まで](../workflows/task-to-integration.md) に続く。

### Workspace summary の差分集計

Board の Workspace 差分は履歴レコードの既定値ではなく、既存 summary API の現在の Git 集計から取得する。各 Repo の Workspace branch と target_branch から base を解決し、その base と worktree の差分を合算する。commit 済み・未 commit・untracked を含むため、直前 commit だけの差分ではない。[集計](../../crates/services/src/services/diff_stream.rs#L66-L115)・[API への組立て](../../crates/server/src/routes/workspaces/workspace_summary.rs#L140-L190)

| 結果 | files / additions / deletions の意味 |
| --- | --- |
| 正常な clean 集計 | `0 / Some(0) / Some(0)`。既知のゼロ |
| 差分取得成功、行数不明を含む | file 数は既知。追加・削除は各々、一件でも不明なら合計も `None` |
| 任意の Repo で base 解決・diff 取得失敗 | 集計全体を `None`。成功 Repo の部分和を完全な合計として出さない |
| container_ref 不在、Repo 関係の読取失敗 | 集計未取得として `None` |

[集計](../../crates/services/src/services/diff_stream.rs#L35-L115)は確認済みbinaryを変更ファイル数へ含め、text行数には加算しない。text読取欠損やサイズ省略の未知は各方向の合計へ伝播する。Git の片側本文が欠けた場合、それを空の本文と見なせるのは実際の Added / Deleted だけである。Modified の読取欠損や text ↔ binary を全行追加・削除と誤認しない。[Git の判定](../../crates/git/src/lib.rs#L709-L726)

API の `files_changed` / `lines_added` / `lines_removed` は独立した optional field で、内部集計の未知を保持する。互換用に残る旧公開 `DiffStats` 型の非 optional field から、この API の意味を推測しない。[API 型と内部集計の分離](../../crates/server/src/routes/workspaces/workspace_summary.rs#L27-L72)・[伝播](../../crates/server/src/routes/workspaces/workspace_summary.rs#L178-L190)。`useWorkspaces` は未知を undefined へ保ち、Kanban はその summary 値を Workspace カードへ渡す。カードは既知0も表示し、未知 file 数と行数を `—` で示す。[hook](../../packages/web-core/src/shared/hooks/useWorkspaces.ts#L58-L83)・[カード](../../packages/ui/src/components/IssueWorkspaceCard.tsx#L279-L293)。時計の未知は別の [Run 時刻契約](../concepts/session-and-agent-run.md#終了の証拠と再接続)に従う。

[Git fixture](../../crates/git/tests/diff_stats.rs)は commit 済み・untracked・削除・text/binary 相互変換、[読取失敗 fixture](../../crates/git/src/lib.rs#L1878-L1903)は Modified の片側欠損、[service fixture](../../crates/services/src/services/diff_stats_tests.rs)は正常0・binary・複数 Repo の部分/全失敗を検査する。[実 API browser fixture](../../tests/kanban/isolated.mjs#L130-L170)は隔離 Git の `3 files / +3 / -1` と clean0を確認する。実 provider の状態や大量 untracked の性能を保証する試験ではなく、この文書保守では再実行していない。

### 差分の未取得・エラー・空を区別する

差分 panel と右側 file tree は、初期 loading、取得失敗、正常に取得した空差分を別の状態で表示する。通信失敗を「変更なし」やゼロ行の成功と解釈しない。再接続中に同じ対象の最後の snapshot が残る場合は、古い表示であることとエラーを示す。[差分 panel](../../packages/web-core/src/pages/workspaces/ChangesPanelContainer.tsx)、[file tree](../../packages/web-core/src/pages/workspaces/FileTreeContainer.tsx)

`WorkspaceProvider` が Workspace / host の切替と unmount に合わせて共有差分 store をリセットし、前の対象の snapshot・error・初期化状態を残さない。store 自体が host ID を保持して選別する構造ではない。既存ファイルを見られることや cached diff があることを、現在の target での検証・統合成功の証拠に使わない。正常な empty と未取得・再接続の見分けは [実 component fixture](../../tests/workflow/specs/diff-tree.spec.ts)で確認する。[scope 境界の処理](../../packages/web-core/src/shared/providers/WorkspaceProvider.tsx)、[store の初期値と reset](../../packages/web-core/src/shared/stores/useWorkspaceDiffStore.ts)

## Preview と開発サーバー

DevServer は Workspace の Repo に登録された dev_server_script を、各 Repo の cwd で Bash ScriptRequest / DevServer として管理する。coding AgentRunや任意shell常駐とは別のExecutionProcessである。UI・stdio両mode・外部HTTP MCPは共通 manager を使い、同じWorkspaceのlockで開始/停止を順序付ける。新規開始・停止はinteractive、未archive、worktree未削除、自動マージ未予約を検査し、MCPは明示Project所属も確認する。[共通開始](../../crates/server/src/routes/workspaces/dev_server.rs#L32-L131)・[対象検査](../../crates/server/src/routes/workspaces/dev_server.rs#L144-L184)・[UI入口](../../crates/server/src/routes/workspaces/execution.rs#L20-L40)

既存running processがあれば重複startは再利用し、暗黙restartしない。古いrecordのcleanupが未確認なら新しい開始を拒否する。scriptが一つもない場合は失敗し、複数Repoの逐次起動途中の失敗はそれ以前の起動を全件rollbackしない。stopはこのWorkspaceのDevServerだけを対象にし、停止・settled済みはno-op、未確認cleanupは再照合対象として残す。[開始の再利用とsettlement](../../crates/server/src/routes/workspaces/dev_server.rs#L32-L131)・[停止と結果](../../crates/server/src/routes/workspaces/dev_server.rs#L234-L305)

statusは観測のみで、各Repo scriptの最新recordに古いrunningを加えて返し、部分起動失敗を隠さない。DBのrunningをOS identityで確認したときだけ running、確認できなければ running_unconfirmed、非runningでもcleanup不明なら stopping_or_unconfirmed、settledなら stopped となる。registry破損を修復して空/停止へ変えず、厳密read失敗を返す。process runningはアプリreadiness、外部URL到達、TLS・認証の成功ではない。[観測と状態](../../crates/server/src/routes/workspaces/dev_server.rs#L186-L232)・[非修復registry読取](../../crates/local-deployment/src/agent_process_registry.rs#L250-L280)

応答の accepted / requested_action / target_state_observed と現在statusを読み分ける。外部MCPの耐久receipt成功だけで起動完了とせず、応答喪失やuncertainでは元operationと現在statusを確認する。stdioは同じmanagerへの直接HTTP adapterで、外部のrequest_id receiptと同じ形ではない。[外部MCPの認可・回復](../integrations/remote-access.md#managed-devserver-を操作する)・[応答](../../crates/server/src/routes/workspaces/dev_server.rs#L299-L305)・[stdio](../../crates/mcp/src/task_server/tools/dev_server.rs#L50-L79)

変更時はregistry非修復、並行/重複start、無関係process保全、両transportの認可/receiptとUI共有状態を確認する。[隔離DevServer runner](../../tests/server/run-managed-dev-server.mjs)は、実agentの承認待ち到達・取消と、人の承認後のtool成功を区別する。[検証入口](development.md#起動回復受動参照mcp-操作を変更するとき)を参照する。

Dev server の稼働だけではチャット送信をブロックしない。専用停止と他 script の活動判定は [Session の入力契約](../concepts/session-and-agent-run.md#会話表示と入力)を参照する。

この script 実行は [coding AgentRun](../concepts/session-and-agent-run.md) とは別の ExecutionProcess 経路である。Settings → Repositories の Dev Server Script、またはWorkspace Logsの編集操作でscriptを設定する。Dev Server操作は開始時にLogsを開き、Repo別のlogと検出URLを表示する。Runningはprocess状態なので、外部browserでapp readinessを別に確認する。

旧subdomain/API preview proxy、HTML検査script/Eruda注入、公開handoffと専用cookieは撤去済みである。外部ブラウザーとprivate tunnel、またはアプリ自身の認証を備えた独立CSVルートを使う。旧preview設定は警告して無視する。[外部browserで確認する](#差分レビューと外部アプリの確認)・[公開配置の廃止契約](server-container.md#埋込みpreviewの廃止)。

ビルド済みVite previewを起動する[静的配信fixture](../../tests/server-access/preview-static.mjs)は、`/@fs` と `/@vite/client` で非公開ファイルや開発endpointが露出しないことをHTTP fetchで検査する。これは撤去済みhandoffや実Auth0接続の成功を意味しない。

## Kanban の main と Workspace の対象を区別する

両ビューの右サイドバーは共通の常時展開環境 panel と開閉 utilities を持つ。Kanban のファイル/Wiki は Project に登録された main repository を読み、本文をボードを残す分割詳細へ表示する。Workspace はその worktree を読む。main の API は Project に属する repo を解決し、既存 canonical root/path/raw 配信 helper を再利用する。任意の Workspace path を main として受け付けない。[main 読取](../../crates/server/src/routes/project_utilities.rs)・[API の差替え](../../packages/web-core/src/features/workspace-files/model/FileApiProvider.tsx)

Wiki は対象 repo の OpenWiki が有効な場合だけ入口を表示し、展開しても Git/端末等を消さない。main snapshot の source は main_repository、Workspace は既存の未公開変更を含む snapshot である。再読込は生成/Sync の開始ではない。[Wiki provider](../../packages/web-core/src/features/wiki/ui/WorkspaceWikiProvider.tsx)・[main snapshot](../../crates/server/src/routes/project_utilities.rs)

## Terminal と editor

Kanbanのファイル見出しは「ファイル」で、file-treeの複数repository選択とmain repoの詳細表示は維持する。terminal専用のrepository selectorとそのmenu領域は削除済みで、API順で最初の有効pathを持つmain repo rootを既定cwdにする。取得error・空一覧・空pathしかない場合はterminalを無効表示し、暗黙cwdでshellを起動しない。mountやrepo表示だけでは開始しない。Kanban右panelの重複した「閉じる」ボタンも削除し、既存のpreview × は残る。[既定rootの規則](../../packages/web-core/src/shared/lib/mainTerminalRepo.ts)・[ファイルとterminalの配置](../../packages/web-core/src/pages/kanban/ProjectUtilities.tsx)・[既存×の開閉試験](../../tests/kanban/utility-close.mjs)

最上部の装飾Petとタイトル/鉛筆の設定導線は [Petの形式・保存・帰属](../concepts/pets.md) を参照する。これらの変更のfixture検証は実PTYや公開認証後画面の確認を意味しない。


端末は両ビューで単一。Workspace は interactive 用途・非archive・非削除を要求し、Repo が一つならその worktree、複数なら Workspace root を初期 cwd とする。Kanban は登録 main repo を起動前に選び、稼働中は対象を固定する。input/output は base64、resize は別 command で、会話 Session の resume ではない。[route](../../crates/server/src/routes/terminal.rs)・[main 選択](../../packages/web-core/src/pages/kanban/ProjectUtilities.tsx)

項目の明示展開だけで新規接続を作り、項目そのものの折畳みで所有 shell/job/child を停止する。確認・警告・tooltip・別終了ボタン・複数タブはない。sidebar 全体の開閉、狭幅でのファイル/Wiki 選択や component unmount は停止意思ではない。exit と検出した通信断は無言で折畳み、自動再接続/再生成せず、次の明示展開だけが一つの新規端末を作る。[表示と所有](../../packages/web-core/src/shared/components/TerminalSection.tsx)・[provider](../../packages/web-core/src/shared/providers/TerminalProvider.tsx)

Linux は未回収 shell の pidfd identity を anchor に POSIX session と捕捉可能な子孫を停止し、wait と reader join 成功後だけ map を解放する。他端末・同 cwd の別実行を推測で kill しない。archive guard は所有端末解放後にも他プロセスを検査する。[PTY cleanup と実プロセス試験](../../crates/local-deployment/src/pty.rs)・[owned session](../../crates/utils/src/process/owned_group.rs)

通信断検出は両端45秒監視と tick 待ちが目安で即時保証ではない。client の close は到達不能serverの停止証明ではなく、通常の明示終了はserverのcleanup後に exit を返す。停止不能kernel、server強制終了、捕捉前に再親化したdaemonの完全回収、非Linuxの同等session cleanupは保証しない。[実PTYの検証入口](../../tests/server-access/workspace-wiki.mjs)・[heartbeat](../../crates/server/src/routes/terminal.rs#L138-L233)

## 外部エディターの URI 連携

外部エディター連携は、名前と URI テンプレートの順序付き一覧を server 共通の `Config.editor.entries` に保存し、対象パスからリンク候補を返す機能である。利用者ごとの IDE 設定や server 上のプロセス起動ではない。起動・protocol handler・接続認証は browser / OS / IDE が担当する。旧 CLI / Custom command 実行、拡張導入、IDE availability 検出の代わりに、すべての入口が同じ候補一覧を使う。[設定と展開](../../crates/services/src/services/config/editor/mod.rs#L1-L68)・[API](../../crates/server/src/routes/workspaces/integration.rs#L30-L99)

### 設定の初期化と保存

`entries: null` または欠落だけが未初期化で、`[]` は意図的な空一覧である。通常の user-system 読込時に browser の `window.location.hostname` を埋めた `vscode://vscode-remote/ssh-remote+HOST<lvk_workspace_dir>` の VS Code 行を作り、通常の Config 保存で固定する。保存済みの一覧は別 hostname からアクセスしても再生成しない。初期行も編集・削除できる通常データであり、専用初期化 API や初期化競合を排他する仕組みはない。[初期値](../../packages/web-core/src/shared/lib/externalEditors.ts#L1-L17)・[読込と保存](../../packages/web-core/src/shared/hooks/useUserSystemController.ts#L33-L49)

設定と onboarding は共通 `ExternalEditorSettings` で追加・編集・削除・順序変更を扱う。General Settings の deep merge は通常配列を index 単位で扱うため、editor は merge 後に全置換する。これを通常の merge だけに戻すと、削除した行や空一覧が保存 draft に残り得る。[一覧編集](../../packages/web-core/src/shared/components/ExternalEditorSettings.tsx)・[置換と理由](../../packages/web-core/src/shared/dialogs/settings/settings/GeneralSettingsSection.tsx#L147-L161)・[onboarding](../../packages/web-core/src/features/onboarding/ui/LandingPage.tsx#L378-L383)

server は保存前に表示名の空白のみ・80文字超・trim 後の完全一致重複とテンプレート構文を検査し、エラー理由を返す。保存に成功してから in-memory Config を置換する。onboarding analytics は URI ではなく登録件数を記録する。[検証](../../crates/services/src/services/config/editor/mod.rs#L40-L51)・[保存と event](../../crates/server/src/routes/config.rs#L183-L240)

### 対象と worktree の寿命

| 操作元 | 解決するディレクトリ / ファイル |
| --- | --- |
| Workspace 全体 | Repo が一つならその worktree、複数またはゼロなら Workspace root |
| Workspace の Repo | membership を確認した `container_ref/保存済みworktree_name（未設定時はrepo.name）` |
| ファイル preview / diff | 明示 `repo_id` と Repo 相対 `file_path`。ディレクトリ変数は Repo root、ファイル変数は解決したファイル |
| Repository settings | 登録済み元 Repo の `Repo.path`。Workspace worktree ではなく、body の Repo/file 指定は拒否 |

Workspace API は `inspection_root` を使い、削除済み・未準備・不在の実体を拒否する。`container_for_inspection` は通常 interactive Workspace を復元し得るため、このリンク解決には使わないという理由がコードに記録されている。既存の絶対ディレクトリを canonicalize するだけで、archive 状態にかかわらず欠落 worktree を作成・復元しない。[対象解決と理由](../../crates/server/src/routes/workspaces/integration.rs#L101-L156)・[実体の条件](../../crates/services/src/services/workspace_usage.rs#L272-L288)・[元 Repo](../../crates/server/src/routes/repo.rs#L161-L185)

ファイル指定は Repo が一つでも `repo_id` を必須とし、file viewer の相対パス正規化・canonical-root 検査を再利用する。絶対パス、drive、親参照、NUL、欠落、Repo 外へ解決される symlink を拒否し、最終 target が通常ファイルでなければ失敗する。[ファイルの検査](../../crates/server/src/routes/workspaces/integration.rs#L131-L143)・[共通境界](../../crates/server/src/routes/workspaces/files.rs#L324-L394)。diff のパスは Repo 名 prefix を持つため UI で最初の prefix を外して送る。preview は元から Repo 相対であり、同じ除去を二度適用してはいけない。[diff の変換](../../packages/web-core/src/shared/lib/externalEditors.ts#L19-L23)・[呼出元](../../packages/web-core/src/pages/workspaces/ChangesPanelContainer.tsx#L437-L442)・[preview](../../packages/web-core/src/features/workspace-files/ui/WorkspaceFilePreviewHeader.tsx#L73-L75)

現在の Workspace 入口は `POST /editor/open` である。リンク生成のみでも、execution_only や 自動マージ 予約中の Workspace では共通 mutation guard が拒否する。旧 editor path GET や SSH tunnel を開設する権限の説明を現行 API に持ち込まない。旧 remote host の SSH tunnel / editor relay は現行 router に存在しない。[route](../../crates/server/src/routes/workspaces/integration.rs#L40-L46)・[予約 guard](../../crates/server/src/middleware/model_loaders.rs#L65-L103)

### URI 展開と失敗の意味

`<lvk_workspace_dir>` と `<lvk_file_path>` は URI の path または query の値だけで使える。path では `/` を保ち、query 値では `/` も含め percent-encode する。変数内の `%` は `%25` になり、固定部分の正しい `%XX` は維持する。固定の日本語・空白等も encode する。scheme / authority / query 名 / fragment の変数、未知変数、不正 percent escape、制御文字、8192 UTF-8 bytes 超、path 展開による意図しない authority 生成は拒否する。[展開](../../crates/services/src/services/config/editor/mod.rs#L70-L177)

これは URI 構文検査であり、scheme の安全性を審査する allowlist ではない。任意 scheme を OS が処理できる保証もない。ファイル context がなければ file 専用テンプレートは `url: null` と理由を持ち、他の directory 候補は使える。対象パス自体の解決失敗は API 全体のエラー、テンプレートごとの適用不可は候補単位の結果として区別する。[候補生成](../../crates/services/src/services/config/editor/mod.rs#L54-L67)・[scheme と context の回帰](../../crates/services/src/services/config/editor/mod.rs#L220-L282)

操作ではまず候補を取得し、選択画面の実リンクを明示クリックする。リンクは `target=_blank` と `noopener noreferrer` を使う。再確認中・取得失敗時に前のリンクを出さず、空一覧と適用不可理由を表示する。server 応答やクリックは IDE 起動・接続成功を証明しない。[画面の責任分割](../architecture/web-and-sync.md#通知音エディターの接続先)・[候補の表示](../../packages/web-core/src/shared/dialogs/command-bar/EditorSelectionDialog.tsx#L58-L88)

### 旧設定と検証限界

旧 editor type / custom command / SSH 欄は deserialize 時に無視し、実行や URI への自動変換をしない。entries がなければ上記の通常初期化へ進む。ただし全体 Config の v7 移行入口は v6 を直接 deserialize するため、v5 以前から現行版への全設定移行まで保証しない。この既存制限を editor schema の互換読込と混同しない。[旧値の扱い](../../crates/services/src/services/config/editor/mod.rs#L18-L25)・[全体移行入口](../../crates/services/src/services/config/versions/v7.rs#L58-L69)・[v7移行の入口](../../crates/services/src/services/config/versions/v7.rs#L58-L69)

変更時は [URI・パス・browser fixture の検証境界](development.md#外部エディター-uri-連携の検証)を参照する。実 IDE / WSL / SSH 認証、Docker Attach、ネストした Dev Containers URI は未検証である。この Sync はソースと試験定義の照合であり、実機接続や自動マージ の検証成功を意味しない。

## 実行履歴を調査・保全する

画面の会話履歴や process logs だけでは provider の元フレームをすべて保持した証拠にならない。[Native Audit の保存先・version 照合・bundle エクスポート](../architecture/agent-runtime.md#native-audit-の保全エクスポート寿命) は再現調査の一次説明である。Workspace の明示削除は Session の Audit ファイルも background 削除するので、保全の判断は [Workspace の削除 lifecycle](../concepts/workspace.md#archive削除期限-cleanup) と合わせて行う。[削除対象](../../crates/workspace-manager/src/workspace_manager.rs)

## 差分レビューと外部アプリの確認

Workspaceを選び、Changes/Git panelのfile treeから対象fileを開く。比較base・未commit変更・未追跡fileを確認し、必要に応じてinline/split表示とwrapを選ぶ。行コメントはfeedbackとしてまとめてagentへ送り、送信は新しい作業指示になるため、読むだけのreviewと分ける。空diffや取得失敗の意味は [差分の状態](#差分の未取得エラー空を区別する)で確認する。[Git panel](../../packages/web-core/src/pages/workspaces/GitPanelContainer.tsx)

Dev ServerのlogからURL/portを確認し、LVKと同じbrowser環境から到達できる外部URLを開く。private appには `ssh -N -L LOCAL_PORT:127.0.0.1:APP_PORT USER@SERVER` のような運用者のtunnelを使い、browserで `http://127.0.0.1:LOCAL_PORT` を開く。APP_PORTはSSH接続先のnetwork namespaceで解決される。container内loopbackのappは、SSH接続先から到達する別経路を先に用意する。LVKのloginはappの認証にならない。独立CSV経路を公開する場合は [証明書・認証・再作成の手順](server-container.md#独立アプリの-csv-ルート)に従う。

Playwright/CDPで開く場合も明示されたURLを使い、console/network・画面・resize・reloadを確認する。browser選択やMCP serverの設定は [provider側のMCP設定](../integrations/agent-providers.md#保存した-profile-と-mcp-接続の操作)で行う。旧iframe注入・Eruda・DevTools URL設定は復活させず、file/Markdown/image/diffの内部previewとは区別する。


### 右サイドバーのWiki高さ

Kanban/Workspaceでは展開したWikiが他の項目を除く残り高さを使う。長いツリーは内部スクロールし、小画面では操作欄と最低1行を保ってサイドバー全体も縦スクロールする。固定高さを増やす変更ではない。検索・repository選択・ページ選択・開閉の処理は従来通りである。[高さ配分の構造](../architecture/web-and-sync.md#wikiツリーの高さ配分)・[Kanbanの配置](../../packages/web-core/src/pages/kanban/ProjectUtilities.tsx)・[Workspaceの配置](../../packages/web-core/src/pages/workspaces/RightSidebar.tsx)


## 共有cache/tmpの容量と手動削除

Kanban / Workspace の右サイドバーで **DISK** を開くと、関連Repositoryごとの `shared/cache` と `shared/tmp` を確認できる。Repository IDを重複排除し、同じ共有実体をWorkspace数だけ加算しない。表示時に取得してReact Queryで60秒キャッシュし、通常の資源監視pollへ再帰走査を追加しない。計測中・取得失敗・未取得と0 Bを区別する。右サイドバーには削除操作やProject横断の影響注記を置かない。[共通表示](../../packages/web-core/src/shared/components/SharedStorageBreakdown.tsx)・[DISKへの配置](../../packages/web-core/src/shared/components/EnvironmentPanel.tsx)

削除前には同じRepositoryを使う全ProjectのRun・DevServerと外部ツールを停止する。次の操作は元に戻せず、cacheは再生成に時間がかかり、tmpはWorkspace間の受け渡し内容を失う。tmpをRun終了で消す処理やTTLは追加しない。[設定の文言](../../packages/web-core/src/i18n/locales/ja/common.json)・[共有領域の用途](../concepts/workspace.md#共有ディレクトリの意味)

1. Settings → Repositories で対象Repositoryを選び、表示名とパスを確認する。
2. **共有 cache / tmp** の「このレポジトリを使う他のプロジェクトにも影響します。」を読み、cacheまたはtmpの個別削除を選ぶ。
3. 対象名と種類を確認して「中身を削除する」を明示実行する。キャンセルでは削除しない。
4. 完了表示と更新された容量を確認する。「容量を更新」でも再取得できる。失敗時も再計測するが、一部削除済みの可能性があるため成功や0として扱わない。[設定UIと結果処理](../../packages/web-core/src/shared/dialogs/settings/settings/SharedStorageSettings.tsx)

`GET /api/repos/{repo_id}/shared-storage` と `DELETE /api/repos/{repo_id}/shared-storage/{kind}` は登録Repositoryから共有実体を解決し、kindをcache/tmpに固定する。任意パスやpersistentは入力にできない。Linuxでは保持したdirectory FD、NOFOLLOW、openat2のsymlink/mount境界制約とunlinkatで中身だけを処理し、対象ディレクトリとWorkspaceの入口リンクを残す。リンク先のpersistent・ソース・別Repositoryへ再帰しない。非Linuxや必要なkernel機能がない場合は安全性を弱める代替へ進まず失敗する。容量は各entryの論理長の合計であり、filesystemの物理使用量と同一ではない。[固定API](../../crates/server/src/routes/repo.rs)・[削除と集計の境界](../../crates/utils/src/shared_storage.rs)

削除直前の利用中判定はProjectで絞らず、同RepositoryのRun状態、予約・終了未確認のprocess、runningのExecutionProcessを参照する。AgentRunがまだない `openwiki_sync` / `openwiki_bootstrap` ownerも、ownerのrepository_idまたはworkspace_reposから特定し、既存の準備中判定とcleanup_allowedで未完了なら409を返す。完了済みownerが履歴に残るだけでは恒久的に拒否しない。これはpoint-in-timeチェックであり、後続起動・外部ツールへの排他保証ではない。独自の包括追跡・分散lock・保留queueはない。[利用中判定](../../crates/server/src/routes/shared_storage.rs#L18-L100)

変更時の検証入口はutilsの隔離ファイルfixture、serverのbusy単体と `shared_storage_http_tests.rs` の隔離HTTP/SQLite、[browser fixture](../../tests/shared-storage/browser.mjs)である。persistent・別Repository・リンク保持、CSRF、AgentRunなし保守owner、失敗と0の区別、確認/取消・容量更新を扱う。実データの削除や全製品UIの証明ではない。今回の統合候補では差分ファイルと固定sourceの一致・346修正保持・差分空白を実行確認し、ユーザー指定により重複suiteを省略した。過去のsource/独立結合試験は別SHAの証拠であり、この候補のテスト成功とは呼ばない。


### 削除後のcanonical Wiki表示

Kanbanのmain Wiki APIはcanonical生成状態を使い、生成Wiki削除後に残るINSTRUCTIONSや人間管理文書だけでWikiを再表示しない。設定の既存状態取得が成功すると選択中repoの表示を再読込する。再構築中は公開成功まで現行Wikiを参照できる。設定・削除・全体再構築の操作と回復は[OpenWiki保守](openwiki-maintenance.md)で確認する。[main API](../../crates/server/src/routes/project_utilities.rs#L85-L94)・[未生成snapshot](../../crates/server/src/routes/openwiki.rs#L144-L166)

### LOCAL-349の保存配置と直接フォルダ

files/editor/diff/Wiki/terminal/dev scriptはWorkspaceRepoの保存配置名で子Repoを解決する。チャットの相対/絶対参照もこの物理名を優先し、同じ表示名の複数Repoを先頭一致で選ばない。登録元Repo.pathへのfallbackで別checkoutを開かない。添付の新規コピー基準は単一Repo・旧Sessionでも共通rootで、Repo file viewerのrootとは異なる。

Project utilitiesの関連Repo読取は現在のプリセットだけでなく既存リンク済みGit Workspaceの実Repoも含む。DirectFolderではGit panelとWorkspace Repo Wiki候補を除外する。これは任意folderのOS書込権限を制限・拡張する境界ではない。

### Claude 利用枠を取得できないとき

右sidebarの理由を確認する。取得中は重複processを起動せず待ち、開始間隔制限なら時間を空ける。`cleanup_unconfirmed`は回収未確認による閉鎖で、画面の再取得やSLOTの無条件開放で復旧しない。運用担当が所有processと正式診断を確認する必要がある。timeout表示は専用process回収完了後の結果であり、認証変更や新loginを自動実行しない。partial/未知値を0や無制限と扱わず、実際のaccount scope・既存公式認証とCLI互換性を[provider契約](../integrations/agent-providers.md#短命接続の所有と失敗)で確認する。[理由表示](../../packages/web-core/src/shared/components/EnvironmentPanel.tsx)・[理由とstatus](../../packages/web-core/src/shared/lib/claudeQuotaView.ts)・[回収](../../crates/executors/src/executors/claude/usage.rs)


### LOCAL-361のWikiリンク整理

旧形式を切り替えるopenwiki selector引数を廃止し、現行OpenWikiとして相対path、`/openwiki/`絶対path、拡張子省略、directory indexを解決する。既知availablePathsにある候補だけを返し、汎用 `pages/` fallbackとroot逸脱拒否は保持する。旧Wiki専用のtestは撤去したが、現行の非変更・symlink・過大page拒否のreader試験は残る。[resolver](../../packages/web-core/src/features/wiki/model/wikiNavigation.ts#L130-L188)・[reader試験](../../crates/services/src/services/wiki/openwiki.rs)

## Markdown本文の目次と引用表示

Issue本文、設計メモ、Wiki、Markdownファイルの閲覧では、h1〜h3が3件以上あると本文上部の「目次」を開ける。初期は閉じ、項目をクリックまたはキーボードで選ぶと対象paneの見出しへ移動してfocusを渡す。日本語や同名見出しも扱い、ページ/ファイル切替で参照と開閉をresetする。短い会話と編集中には表示せず、閉じたdetails内部は一覧へ含めない。保存本文やURL hashを変更する操作ではない。[共通の表示境界](../architecture/web-and-sync.md#markdown本文引用目次の表示境界local-368)

Wikiは既存リンク捕捉とページ別scroll復元を維持し、scrollKeyを目次のcontentKeyにも使う。MarkdownファイルはWorkspace/Repo/pathをcontentKeyにする。行指定リンクは従来どおりsource viewerへ開くため、描画見出しをソース行番号と混同しない。[Wiki](../../packages/web-core/src/features/wiki/ui/WorkspaceWikiPanel.tsx#L348-L361)・[ファイル分岐](../../packages/web-core/src/features/workspace-files/ui/WorkspaceFilePreviewPanel.tsx#L105-L128)

判断待ちは先頭に独立した `> [!DECISION]` を置いた引用だけが専用表示になる。標準alert5種も閲覧装飾されるが、人間の意図は通常引用に明示ラベルを付ける。表示器は原文を自動分類・保存せず、コピーにも元のMarkdownを使う。未決事項を解消してマーカーを外す場合は通常の権限のある編集として行う。新しいdetails記法や判断待ち件数の自動抽出は追加していない。[実コンポーネント検証と限界](../../tests/markdown/README.md)


## Workspaceから自動マージを依頼・診断する（LOCAL-364）

Git操作の自動マージを選ぶと、当該WorkspaceとRepoを選択した共通開始UIを開く。採用commit・target・検証/Wiki方針と追加指示を確認する。複数選択は一つの候補を一回公開する依頼で、単純な変更でも担当Codexを実行する。PR操作やrebase・共用競合解消は別用途として残る。[Git操作](../../packages/web-core/src/shared/components/tasks/Toolbar/GitOperations.tsx)・[開始UI](../../packages/web-core/src/features/kanban/ui/IntegrationPanel.tsx)

開始後は割当Sessionの通常会話で追加指示・質問へ応答し、依頼概要のdetailsから保存済み初回入力原文を読む。停止時は公開commit、未公開理由と次操作を確認する。read-only診断は期待するrepository path/候補branchと観測branch/HEAD・取得エラーを返すため、誤branchや担当stdio終了後でも既存UI/上位AI読取から確認できる。診断はbranch自動修復やmutation許可ではない。[診断API](../../crates/server/src/routes/integrations/mod.rs)・[対象整合とguard](../../crates/server/src/routes/integrations/conversation.rs)・[初回概要](../../packages/web-core/src/features/workspace-chat/ui/IntegrationInitialMessage.tsx)

Wiki receipt pending: はGit適用後のreceipt回復待ちとしてUIの復旧条件と揃える。公開要求受付をmain適用成功へ読み替えず、Gitを再mergeする代わりに既存の回復操作と結果を確認する。sourceのDone保留と公開成功も別結果として扱う。[回復と公開](../concepts/formal-integration.md)
