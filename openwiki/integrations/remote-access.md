---
type: architecture
title: 外部アクセス・MCP・Runner の境界
description: local self-host のアクセス境界、認証付き外部 MCP、残存する Runner と撤去された Cloud / relay。
tags: [remote, authentication, relay, mobile, self-hosting]
sources:
  - id: openwiki-source-651d1fb6c9e49916a916ab51
    resource: repo://Cargo.toml
  - id: openwiki-source-1e7d6f37595b8480ddaf530e
    resource: repo://crates/api-types/src/issue_filter.rs
  - id: openwiki-source-826684f900e82d99e3bfe4e1
    resource: repo://crates/db/migrations/20261029160000_project_attachments.sql
  - id: openwiki-source-672c8da0d18c56b60df745a2
    resource: repo://crates/db/src/models/board_tag.rs
  - id: openwiki-source-27eb1d689068b93b5282d2b6
    resource: repo://crates/db/src/models/design_note_read.rs
  - id: openwiki-source-d83dfc60bdfadf451d836df3
    resource: repo://crates/git/src/reference.rs
  - id: openwiki-source-237b255ffafe7ecea55eefc6
    resource: repo://crates/mcp/src/task_server/tools/attachments.rs
  - id: openwiki-source-c1a99ab3d402970b84e3cc23
    resource: repo://crates/mcp/src/task_server/tools/dev_server.rs
  - id: openwiki-source-4c00e6ad62d940e70c2ebbb3
    resource: repo://crates/mcp/src/task_server/tools/mod.rs
  - id: openwiki-source-28437ab4b181623345af79bc
    resource: repo://crates/mcp/src/task_server/tools/stdio_tests.rs
  - id: openwiki-source-c2eaddb5c69f49c5ee934a63
    resource: repo://crates/server/src/middleware/origin.rs
  - id: openwiki-source-e1757d002aa325145a915b38
    resource: repo://crates/server/src/provider_usage.rs
  - id: openwiki-source-448d823f6734081b48eb5350
    resource: repo://crates/server/src/remote_mcp/attachment_tests.rs
  - id: openwiki-source-b5d7c5a5b00efaa28b3e1777
    resource: repo://crates/server/src/remote_mcp/attachments.rs
  - id: openwiki-source-53c04d549d6401053afe5fc4
    resource: repo://crates/server/src/remote_mcp/code_query.rs
  - id: openwiki-source-0009a6c83972b5e5975ba606
    resource: repo://crates/server/src/remote_mcp/code_read.rs
  - id: openwiki-source-2a0ddba6878eea1524264906
    resource: repo://crates/server/src/remote_mcp/design_notes.rs
  - id: openwiki-source-87ddf3d108f228e9ef8f8964
    resource: repo://crates/server/src/remote_mcp/dev_server.rs
  - id: openwiki-source-8e100846c63b4921192f2eb4
    resource: repo://crates/server/src/remote_mcp/errors.rs
  - id: openwiki-source-0c4d7a1c881e7f1618f3bb2f
    resource: repo://crates/server/src/remote_mcp/evidence_read.rs
  - id: openwiki-source-235c61e44e26d792cb34ab1d
    resource: repo://crates/server/src/remote_mcp/execution_approvals.rs
  - id: openwiki-source-c2b406887bcde85e48eb07b0
    resource: repo://crates/server/src/remote_mcp/goals.rs
  - id: openwiki-source-0a439773e44e2649541571be
    resource: repo://crates/server/src/remote_mcp/goals/tests.rs
  - id: openwiki-source-3ec71fa8cd8936e31e523736
    resource: repo://crates/server/src/remote_mcp/integration_read.rs
  - id: openwiki-source-1083b3cf6ec502f1c6eb3811
    resource: repo://crates/server/src/remote_mcp/integration_write.rs
  - id: openwiki-source-baadfd16fa9d31d06a1e6eb1
    resource: repo://crates/server/src/remote_mcp/issue_archive.rs
  - id: openwiki-source-a43253b5be9686da077b8067
    resource: repo://crates/server/src/remote_mcp/issue_comments.rs
  - id: openwiki-source-a6a66833efdc90f958755f54
    resource: repo://crates/server/src/remote_mcp/issue_tags.rs
  - id: openwiki-source-9e15762bf7a432b7605f8466
    resource: repo://crates/server/src/remote_mcp/issue_write.rs
  - id: openwiki-source-83d25ea893da220229c11f8b
    resource: repo://crates/server/src/remote_mcp/launch_tests.rs
  - id: openwiki-source-b5eef61c63c11d3fe10b3ccf
    resource: repo://crates/server/src/remote_mcp/launch.rs
  - id: openwiki-source-71f837a8aa36443ad9688945
    resource: repo://crates/server/src/remote_mcp/mod.rs
  - id: openwiki-source-6adda10f98b079272f82a4cc
    resource: repo://crates/server/src/remote_mcp/operations.rs
  - id: openwiki-source-202693827acda4283453b799
    resource: repo://crates/server/src/remote_mcp/provider_usage.rs
  - id: openwiki-source-9d3b3180c15d3125ddbb2675
    resource: repo://crates/server/src/remote_mcp/questions.rs
  - id: openwiki-source-ccbefbdb32b7223afc06bb2b
    resource: repo://crates/server/src/remote_mcp/read.rs
  - id: openwiki-source-0545bff1ae2507a92137efaf
    resource: repo://crates/server/src/remote_mcp/repository_wiki.rs
  - id: openwiki-source-3ef7bc8590ef87216eade432
    resource: repo://crates/server/src/remote_mcp/resource_observation.rs
  - id: openwiki-source-0987a696c656ec0d4f318f44
    resource: repo://crates/server/src/remote_mcp/run_events.rs
  - id: openwiki-source-3651b7ce4535b584634e8bc8
    resource: repo://crates/server/src/remote_mcp/runtime_read.rs
  - id: openwiki-source-b8d968c49ef2f35fc1a18cce
    resource: repo://crates/server/src/remote_mcp/runtime_write.rs
  - id: openwiki-source-43c18ae2d53a6d4bee6c33b2
    resource: repo://crates/server/src/remote_mcp/tests.rs
  - id: openwiki-source-0ce0e179a46d5c36c5f05ecf
    resource: repo://crates/server/src/remote_mcp/workspace_archive.rs
  - id: openwiki-source-47f0a8c78f8e76d48dfdd192
    resource: repo://crates/server/src/resource_observation.rs
  - id: openwiki-source-8eb245a586a88c8ce0dfd2b5
    resource: repo://crates/server/src/resource_observation/disk.rs
  - id: openwiki-source-7cfd6d36fabe8e3d6102302d
    resource: repo://crates/server/src/resource_observation/linux.rs
  - id: openwiki-source-162bcfe1ee47c1047ccf7d52
    resource: repo://crates/server/src/routes/agent_runs.rs
  - id: openwiki-source-a98b89b4828bd8f76e2ca9bf
    resource: repo://crates/server/src/routes/attachment_transfer.rs
  - id: openwiki-source-b393e75029374eaf9cba5f7d
    resource: repo://crates/server/src/routes/integrations/runtime.rs
  - id: openwiki-source-2100c29dd665b0f698970419
    resource: repo://crates/server/src/routes/local_remote/board_api/issue_filter.rs
  - id: openwiki-source-65c0f36670a161dc05040e69
    resource: repo://crates/server/src/routes/mod.rs
  - id: openwiki-source-d8c782d4a4246db71e503283
    resource: repo://crates/server/src/routes/resource_observation.rs
  - id: openwiki-source-4d617b0e2fa188c6ad63ae56
    resource: repo://crates/server/src/routes/sessions/questions.rs
  - id: openwiki-source-af328071d51bff75b5246d3b
    resource: repo://crates/server/src/routes/workspaces/usage.rs
  - id: openwiki-source-7d8384e54754a811ab7ab92e
    resource: repo://crates/services/src/services/approvals.rs
  - id: openwiki-source-4294e32943918c50b31813b6
    resource: repo://crates/services/src/services/file.rs
  - id: openwiki-source-605a3efdc11efae80ada92b1
    resource: repo://crates/utils/src/mcp_error.rs
  - id: openwiki-source-4055aa6b8c5089e24c2a9f7c
    resource: repo://crates/utils/src/reference_file.rs
  - id: openwiki-source-74a1893f4c1e77eb654658f7
    resource: repo://docs/ai-mobile/tailscale-local-access.md
  - id: openwiki-source-d6e65c1c70d582aa76ba14d4
    resource: repo://docs/development/chatgpt-mcp-setup.md
  - id: openwiki-source-a6728d75b2bdf1a00440e8fa
    resource: repo://docs/development/chatgpt-mcp.md
  - id: openwiki-source-3ecddc0be7b037ea6ab24381
    resource: repo://packages/local-web/vite.config.ts
generated: { by: "codex", at: "2026-11-02T23:50:54.203Z" }
verified:
  - by: openwiki/0.7.2
    at: 2026-11-02T23:50:54.203Z
---

# 外部アクセス・MCP・Runner の境界

現行製品は local self-host の Board と Workspace を使う。旧 remote-server・PostgreSQL / Electric・Cloud AuthSession・SPAKE2 pairing・relay署名・WebRTC fallback・remote-web は撤去された。旧 token の寿命や paired-host refresh を現行の接続契約として適用しない。[現行構成](../../Cargo.toml)、[server router](../../crates/server/src/routes/mod.rs)。認証付き外部 MCP と execution bridge は別の機能として残る。

## UI の直接アクセス

通常UI/APIは同じOSユーザーで動作する信頼された利用者向けで、公開時はHTTPSとLVKのBasic/Auth0認証を使う。[運用前提](../../README.md)。Origin guard は Origin がなければ許可し、ある場合は same-origin / allowlist を照合する。これは利用者認証ではなく、旧 relay 例外もない。[Origin](../../crates/server/src/middleware/origin.rs)。

local-web は `/api` を backend へ送り、WebSocket も同じ経路を使う。旧 remote host を明示した要求は送信前に拒否されるため、残った host ID から別ホストへの暗黙 fallback を期待しない。[Web transport](../architecture/web-and-sync.md)。Vite の `.ts.net` allowlist と Host を保持する proxy は残るが、tailnet 自体の認証・配備成功をこの設定だけで証明しない。[proxy](../../packages/local-web/vite.config.ts#L135-L151)。

## 認証付き外部 MCP

公開配置は初回にロックされる。初回setup、Basic/Auth0切替、Web sessionと復旧は [サーバー認証](../concepts/server-access.md)を正本とし、ここでは外部MCPの委任範囲を扱う。

Streamable HTTP `/mcp` はAuth0でMCPを有効化した場合のみ利用できる。Local/Basicでは使えない。旧 `LVK_MCP_CONFIG` と外部introspection設定は起動時に拒否し、設定画面へ移行する。metadataは `/.well-known/oauth-protected-resource/mcp`。これは stdio MCP とは独立しており、内部会話モデルを起動しない。[旧 Astra の廃止](../concepts/project-chat.md)後も外部 MCP と Workspace 質問回答は残る。[HTTP入口](../../crates/server/src/remote_mcp/mod.rs)、[認証設定](../../docs/self-hosting/authentication.mdx)。

共通認証はAuth0 discovery/JWKS、JWT署名、issuer、resource audience、有効期限、承認済みメンバーを検証する。全操作に `lvk:read`、Issue/Todo・設計メモ・agent起動/制御・既存Wiki同期・Workspace archiveの準備/確定などの変更に `lvk:write`、自動マージの開始・取消に `lvk:merge` を要求する。Runtime Access の観測には追加で `lvk:runtime:read`、起動・停止にはさらに `lvk:runtime:execute` が必要で、一般のwrite scopeとは独立する。HTTP入口とtool処理の両方でscopeを確認する。承認済みメンバーは全プロジェクトを使い、旧owner単独・静的Project allowlist方式ではない。[JWT検証とissuerの制限](../concepts/server-access.md#webmcp内部agentを同じ資格情報にしない)、[scope](../../crates/server/src/remote_mcp/mod.rs)、[実体の関係検証](../../crates/server/src/remote_mcp/launch.rs)。

transportはstatelessなJSON応答で、`MCP-Session-Id`付き要求を拒否する。productionは共通認証のprincipalを受け、Project集合を現在のDBから作る。`remote_mcp/auth.rs`に残るintrospectionやowner設定はtest用で、現行の認証経路ではない。[入口](../../crates/server/src/remote_mcp/mod.rs#L75-L164)、[session ID拒否](../../crates/server-access/src/lib.rs#L473-L518)

ChatGPT 接続用 application は Web login 用と分離する。Auth0 作成支援の preview / apply と、保存済み専用 secret の明示 reveal は [サーバー認証の設定契約](../concepts/server-access.md#auth0-の作成支援と有効化を分ける)を参照する。設定を作成・保存できても、実アカウントでの接続成功を証明したことにはならない。

[Project Todo](../concepts/project-todos.md)も外部 MCP の対象で、read は取得、write は作成・更新・削除を許す。変更結果は下記の operation receipt で確認する。メモを登録することは agent の仕事開始ではない。

## 設計意図を記録・参照する

[設計メモ](../concepts/design-notes.md)は Project の本文版と会話からの独立追記を保存する。`list_design_notes` / `get_design_note` / `get_design_note_history` は read、`write_design_note` は read + write を要求する。本文ページは返された保存版を固定して読み、書込結果は確定したIDとrevisionだけの receipt で確認する。利用時の出典・明示本文編集・再送・削除後閉鎖の詳細は [設計メモのMCP境界](../concepts/design-notes.md#ui-rest-と-mcp-の境界)を参照する。

## Issue のレビューを共有する

外部 HTTP MCP は `list_issue_comments` / `get_issue_comment` を read scope、`add_issue_comment` / `edit_issue_comment` を read + write scope で公開し、Project と Issue の所属を確認する。通常の operation receipt による追記なので、モデルの起動・要求変更・承認を兼ねない。内部 stdio の catalog は別である。[登録と dispatch](../../crates/server/src/remote_mcp/mod.rs#L343-L350)・[handler の認可](../../crates/server/src/remote_mcp/issue_comments.rs#L85-L153)

利用時は `get_issue` の `review_target` を保持してレビューを投稿し、他の client は一覧プレビューから必要な全文を読む。レビュー対象の Issue revision と、本文の編集混在を防ぐ comment revision は別の値である。上限・再送・編集後の来歴・古い対象の意味は [Issue の共有レビュー](../concepts/issue-collaboration.md#共有レビューは通常コメントとして残す)を正本とする。特に全文はプレビューと連結せず、[revision を固定した分割取得](../concepts/issue-collaboration.md#長文を編集混在なく読む)を使う。[公開 tool 契約](../../crates/server/src/remote_mcp/issue_comments.rs#L65-L82)

本人の本文編集は、全文読取から得た `expected_comment_revision` と同じ issuer/subject の保存来歴を要求する。自己申告 label や legacy の著者名で所有権を代用しない。応答不明時は同じ request/input と元 operation を使い、要求や実行を変える操作へ拡張しない。[本人編集の正本](../concepts/issue-collaboration.md#自分の既存コメント本文を編集する)・[編集 handler](../../crates/server/src/remote_mcp/issue_comments.rs#L188-L239)

## Managed DevServer を操作する

`start_dev_server` / `stop_dev_server` / `get_dev_server_status` は明示 Project / Workspace を要求し、stdio の両 mode と外部 HTTP から同じ UI manager を使う。外部の観測には read + runtime:read、開始停止にはさらに runtime:execute が必要で、一般 write や [Runtime Access の設定](../concepts/runtime-access.md)で代用しない。Orchestrator stdio は別 Workspace を拒否する。[外部 scope](../../crates/server/src/remote_mcp/dev_server.rs#L13-L108)・[stdio adapter](../../crates/mcp/src/task_server/tools/dev_server.rs#L11-L79)

外部の開始停止は `request_id` を持つ耐久receiptを返す。同じID/入力で再送し、`get_operation` と現在statusを読み分ける。succeededはmanager受付の結果で、アプリreadinessの証明ではない。副作用後の観測失敗は uncertain となり、新IDで盲目的に再起動しない。重複startの再利用、停止後のsettlement、複数Repoの部分失敗と観測状態は [DevServer運用の正本](../operations/workspace-inspection.md#preview-と開発サーバー)に集約する。[receipt と失敗](../../crates/server/src/remote_mcp/dev_server.rs#L51-L108)

Codexが第三者toolの一回限り権限を求める場合は [Session の正式承認](../concepts/session-and-agent-run.md#codex-の一回限り-mcp-tool-承認)へ進む。tool認可を緩めたりOpenWikiのallowlistを転用したりしない。

## 実行承認を読み、単発の判断を送る

`get_question(kind=approval)` は保留中承認の ID、対象 Project/Workspace/Session/Run、operation、実引数、single_request scope、状態、正規 UI URL を返す。同じ attempt/tool_call_id の承認前 tool-start だけを引数の根拠にする。空 `{}`、取得不能、credential field の伏字を区別し、任意 provider extension や tool 結果をそのまま公開しない。[詳細の相関と伏字](../../crates/server/src/remote_mcp/execution_approvals.rs#L171-L232)

`decide_execution_approval` は明示 boolean と最新 revision、全対象 ID、request_id を受ける。既存の外部認証済み member（channel=mcp）と read/write scope を要求し、read-only や内部 Agent の自己承認は拒否する。UI 共通 Session lock・ResolveApproval へ渡し、元イベント、最新 Run/attempt、所属、予約状態と元 waiter の期限・対象を保つ。実行専用 Workspace でも正規 owner を検証し、session/恒久許可へ広げない。`answer_question` は承認を受け付けない。[専用判断](../../crates/server/src/remote_mcp/execution_approvals.rs#L100-L168)・[UI 共通 dispatcher](../../crates/server/src/routes/agent_runs.rs#L490-L565)

引数が相関できない許可は理由付き UI fallback、拒否は専用経路で可能である。変更/取消/期限切れ/解決済み/対象不一致は競合や失敗として扱い、同じ request_id と同じ入力は既存 operation を返す。`get_execution_approval` は保留後も読め、decision_reserved と provider の resolved を区別する。受付成功はツール完了ではなく、未観測なら tool_execution_complete=null / not_observed で返す。配送不明は uncertain とし、receipt と command/event を照合してから判断する。[読戻し](../../crates/server/src/remote_mcp/execution_approvals.rs#L48-L98)・[元 waiter の検証](../../crates/services/src/services/approvals.rs#L134-L190)

この実行承認は仕様回答や [自動マージの公開](../concepts/formal-integration.md)と別の権限である。署名 JWT/mock JWKS、専用handlerとservice/provider protocolの局所試験は実Auth0/ChatGPTや実ツール完了の検証ではない。[代表試験](../../crates/server/src/remote_mcp/execution_approvals.rs)・[認可回帰](../../crates/server/src/remote_mcp/tests.rs)

## Provider account の利用枠を参照する

`read_provider_usage` は read scopeに加えて外部 administrator を必要とし、`provider` と `account_context=server_default` を明示する。account scopeはWorkspaceなしで利用できる。既存Session usageは全所属IDを要求し、native IDを直接渡さない。受動接続はAgentRunを作らず、MCP runtime実行scopeや操作receiptとは別のreadである。[外部認可](../../crates/server/src/remote_mcp/provider_usage.rs#L1-L40)・[共通read](../../crates/server/src/provider_usage.rs#L1-L89)

quota、account summary、既存threadのusage、Workspace累積tokensは交換できない。未知・partial・未対応、provider別の短命接続・終了回収・単位・検証限界は [provider参照の正本](agent-providers.md#受動的な-account-quotausage-参照)を読む。外部MCP成功を実モデル実行やsubscription請求の証明としない。

## Repository Wiki の同期を依頼する

`get_repository_wiki` と `get_repository_wiki_sync` は read、`sync_repository_wiki` は read + write で、Project と repository の所属を検証する。自動マージ の merge scope とは別である。adapter は `LaunchActions` から UI 共通の状態観測・Sync 受付へ接続し、独立した Wiki scheduler やモデル選択を作らない。[catalog と所属・権限](../../crates/server/src/remote_mcp/repository_wiki.rs#L54-L98)・[共通サービス](../../crates/server/src/remote_mcp/launch.rs#L161-L166)

認可済みの `project_id` は共通 Sync 受付へ引き継がれ、保守 Workspace の Issue 紐付けに使われる。Repo が関連しているだけでは別 Project の接続認可を得ない。共有 Repo の選択先と Issue なしの互換経路は [開始元 Project と保守 Issue](../operations/openwiki-maintenance.md#開始元-project-と保守-issue)で説明する。[引継ぎ](../../crates/server/src/remote_mcp/repository_wiki.rs#L214-L233)・[認可と引継ぎの回帰](../../crates/server/src/remote_mcp/launch_tests.rs#L613-L659)

既存 Wiki の同期・再同期が対象で、初期生成は Repository settings に残る。観測した branch/source の固定、元 request ID による再送、受付と公開完了の区別は [OpenWiki 保守の MCP 手順](../operations/openwiki-maintenance.md#mcp-から既存-wiki-を同期する)を正本とする。返された文章や `needs_sync` は新しい実行への承認ではない。既存の [operation receipt](#参照結果と操作-receipt)を回復してから対象状態を確認する。[開始契約](../../docs/development/chatgpt-mcp.md)

## Workspace を外部 MCP から archive する

履歴を残して作業木を解放する入口は `prepare_workspace_archive` と `confirm_workspace_archive` である。両方に明示的な `project_id` / `workspace_id` と `lvk:read` + `lvk:write` が必要で、対象の所属を検査する。stdio の `update_workspace` と token を交換できない。物理所有権、busy 検査、token の束縛・寿命、削除失敗の詳細は [Workspace の archive 契約](../concepts/workspace.md#外部-mcp-の確認と結果の回復)を正本とする。[外部 tool](../../crates/server/src/remote_mcp/workspace_archive.rs#L16-L117)

prepare の対象・branch と正確なメッセージ「ワークツリーが削除されます」を利用者に一度示し、確認後だけ token と stable `request_id` で confirm する。取消は confirm を送らないことで成立し、prepare だけでは archive や削除を行わない。応答喪失は元要求の再送か `get_operation` で回復し、receipt と `get_workspace` の現在値を照合する。`succeeded` だけから削除済みと判断せず、`archived`・`worktree_deleted`・`deletion_error` を読む。busy 拒否は別手段での削除許可ではない。[操作手順](../../docs/development/chatgpt-mcp-setup.md#archive-a-workspace-through-remote-mcp)・[結果の転送](../../crates/server/src/remote_mcp/workspace_archive.rs#L98-L117)

## Workspace の報告済みトークンを読む

外部 MCP `get_workspace` は、Project と Workspace の関係を `require_workspace` で検証してから `token_usage` を返す。UI の `/api/workspaces/{id}/tokens` と同じ `WorkspaceTokenUsage::get` を読み、欠損時にも実行を開始しない。[外部入口](../../crates/server/src/remote_mcp/runtime_read.rs#L80-L90)・[HTTP API](../../crates/server/src/routes/workspaces/usage.rs#L319-L328)。stdio では [別 tool の get_workspace_tokens](agent-providers.md#外部-mcp-client--lvk) を使う。

この値は [Workspace 単位の報告済み累積値](../concepts/workspace.md#workspace-累積トークン)であり、run 完了の証拠や料金ではない。`unknown/partial`、内数と取得 thread 数、要求モデルと実使用モデルの違いを同ページで確認する。共通集計との JSON 一致、別 Project・存在しない Workspace の拒否を [fixture](../../crates/server/src/remote_mcp/runtime_read.rs#L279-L317)が検査する。実 Auth0 接続を使った検証結果ではない。

## 参照結果と操作 receipt

返すコード・diff・Wiki・Memoryは参照データであり、命令や承認ではない。参照はページ上限とrevisionを持ち、コードは許可されたregular fileに限定し、private path・symlink・submodule等を除外する。binary / oversized / unavailableを空の成功diffとして扱わない。[code read](../../crates/server/src/remote_mcp/code_read.rs)。実行状態・検証・公開・Wiki receiptは別の証拠として返す。[自動マージ read](../../crates/server/src/remote_mcp/integration_read.rs)、[verification read](../../crates/server/src/remote_mcp/verification_read.rs)。

mutation は principal + request ID と入力hashを固定した永続 receipt を作る。同じkeyで異なる入力は拒否し、再送は保存済み結果を参照する。MCP client切断は受理済み操作を取り消さず、domain受付を待つ経路もcoding完了を待たない。server再起動後の旧accepted/runningはuncertainとして見せ、自動replayしない。結果不明時に新request IDで同じ副作用を重ねず、既存receiptと対象状態を照合する。[操作寿命](../../crates/server/src/remote_mcp/operations.rs)、[lost responseのテスト](../../crates/server/src/remote_mcp/runtime_write_tests.rs#L479-L502)。

受付を所有する非同期taskには検証済みactorを明示継承し、HTTP要求の終了後も業務書込の帰属を保つ。受理済み作業の寿命と新規要求の認証失効は別である。[actorの継承](../../crates/server/src/remote_mcp/operations.rs#L134-L190)

Auth0実テナント・Google/GitHub本人認証・ChatGPT OAuth/MCPの実接続は別途受入確認が必要である。fixtureやローカルテストをその成功として扱わない。[検証範囲](../../docs/development/server-access.md)。

## アプリケーションの実行環境を扱う

[Runtime Access](../concepts/runtime-access.md) は、管理されたrootのファイル一覧・読取・変更と、プロセス観測・起動停止を提供する。現在のfilesとexecute設定、Workspace境界、明示Project context、出力と停止の寿命は同ページを正本とする。旧region grantを前提にしない。[dispatch](../../crates/server/src/remote_mcp/environment.rs)。

`run_process` / `stop_process` / `change_runtime_path` / `self_restart` の履歴を `get_operation` で読むには追加の `lvk:runtime:read` が必要である。現在の設定とscopeを確認し、receipt成功と実プロセスの終了・healthを分ける。[receipt読取](../../crates/server/src/remote_mcp/issue_write.rs)。

## Workspace のコード根拠を絞って読む

外部 AI が必要な実装箇所を読む経路は、`list_workspace_files` による所在の絞り込み、`search_workspace_files` による文字列検索、`get_workspace_file_ranges` による周辺行の取得である。既存の `get_workspace_file` は文字単位のページ取得として残る。これらは要求時に現在の作業木を読む参照機能で、検索用の常駐索引や過去本文の snapshot 保存を持たない。[公開 schema と dispatch](../../crates/server/src/remote_mcp/code_read.rs#L32-L90)・[検索・範囲取得](../../crates/server/src/remote_mcp/code_query.rs#L122-L179)。画面の [files API](../operations/workspace-inspection.md#repo-内ファイルを読む)とは入口も読取対象の契約も異なる。

### 対象と一覧の継続

Project / Workspace / repository の関係を共通の evidence reader で検証し、I/O 後にも所属を再照合する。checkout root は保存済みの [Workspace 種別と所在地](../concepts/workspace.md)から解決し、利用者指定の filesystem root は受け取らない。削除済み worktree を復元しない。[対象解決と再照合](../../crates/server/src/remote_mcp/evidence_read.rs#L133-L214)

候補は Git index の追跡済み regular file である。untracked、private path、Git symlink、submodule は検索範囲外であり、runtime の symlink に置き換えられても読取範囲は広がらない。共通 reader は各相対 path component を `O_NOFOLLOW` で開き、regular file だけを制限付きで読む。非 Unix で安全性を緩めた fallback はしない。これは任意コード実行や shell 検索ではない。[追跡対象](../../crates/git/src/reference.rs#L180-L212)・[private path](../../crates/git/src/reference.rs#L38-L60)・[confined read](../../crates/utils/src/reference_file.rs#L5-L75)

一覧と検索は `directory_prefix` と `filename_contains` を共有する。前者は先頭・末尾 slash のない repository 相対ディレクトリーで、`src` は `src/a` を含むが `src2/a` は含まない。後者は basename の大文字小文字を区別するリテラル部分一致で、glob や full path 検索ではない。一覧では両 filter を **paging 前** に適用し、継続時は同じ filter と一覧の `expected_revision` を渡す。filter 有効時はその値も一覧 revision に含め、未指定時は既存の revision 計算を保つ。この一覧 revision は後述のファイル本文 revision とは用途が異なる。[filter](../../crates/server/src/remote_mcp/code_query.rs#L13-L76)・[一覧と revision](../../crates/server/src/remote_mcp/code_read.rs#L113-L169)

### 検索の完了と文脈の短縮

検索は大文字小文字を区別する単一行の非重複リテラル一致であり、正規表現ではない。query は空でない1,024 UTF-8 bytes以下、CR/LFを含めない。path順に走査し、行・列は1始まり、列は Unicode scalar value 単位、`end_column_exclusive` は終端非包含である。LF / CRLF は行を区切り、末尾改行による余分な空行は作らない。既定は20一致と前後1行、指定上限は50一致と前後2行である。[入力](../../crates/server/src/remote_mcp/code_query.rs#L78-L99)・[検証](../../crates/server/src/remote_mcp/code_query.rs#L134-L151)・[一致位置](../../crates/server/src/remote_mcp/code_query.rs#L259-L345)

一要求は最大1,000ファイル、各512 KiB、走査予算16 MiB、協調的な2秒期限、結果と除外 item 合計64 KiBに制限される。各読取前に512 KiB + 1の余裕を確認し、非text・失敗時はその量を保守的に課金するため、`scan_bytes_charged` を実転送量と解釈しない。Git index は既存 reader の100,000 entry上限を持つ。期限チェックはファイル間と行間で行い、index列挙・個別 OS read を強制中断しないため、2秒の wall-clock 保証ではない。[走査・予算](../../crates/server/src/remote_mcp/code_query.rs#L259-L345)・[reader 上限](../../crates/git/src/reference.rs#L11-L33)・[index上限](../../crates/git/src/reference.rs#L180-L196)

`complete`、`stop_reason`、`exclusions` を一緒に読む。打切り理由は `time_limit` / `file_limit` / `scan_byte_limit` / `match_limit` / `output_byte_limit`。候補の binary、too_large、missing、unsupported、unsafe_or_unreadable は除外として返り、除外だけでも `complete:false`、`stop_reason:null` になり得る。空の `items` が選択範囲内の一致なしを意味するのは `complete:true` のときだけで、private path 等の範囲外まで調べた証明ではない。検索には continuation cursor がなく、制限に当たれば filter / query を狭めて再検索する。[完了条件](../../crates/server/src/remote_mcp/code_query.rs#L273-L345)

長い一致行や隣接行が残り出力予算に入らない場合、全文脈 → 一致の前後128文字・隣接行の先頭128文字 → 同32文字 → 一致文字列のみ → 所在メタデータのみ、の順で縮小する。`matched`、path、行列、revision は短縮しない。`context_ranges` は `context` の各文字列と一対一に対応し、実際の行と開始列・終端非包含列を示す。抜粋へ架空の省略記号は挿入せず、文脈がなければ両配列は空、`context_start_line` は null となる。[文脈の選択](../../crates/server/src/remote_mcp/code_query.rs#L190-L250)

`context_truncated:true` は要求した文脈の省略であり、走査の不完了を意味しない。最小の所在 item さえ入らない場合だけ、その一致を返せず `output_byte_limit` で打ち切る。すべての候補は `matched`・path・範囲情報を含む JSON escaping 後の bytes と区切り分で計量する。標準の応答 identity / link metadata は64 KiBの **item予算** の外側であり、応答全体の上限と混同しない。[計量](../../crates/server/src/remote_mcp/code_query.rs#L181-L250)・[出力打切り](../../crates/server/src/remote_mcp/code_query.rs#L319-L345)

### 同じ本文を確認して周辺行を読む

検索の hit または `get_workspace_file` が返すファイル revision を、`get_workspace_file_ranges` の各 `expected_revision` に渡す。初回読取では省略でき、指定した場合だけ本文の一致を要求する。1バッチは1–8範囲、各範囲は1始まり両端包含で最大200行。存在しない終端は黙って丸めず `range_out_of_bounds` となり、成功 item の `lines` は行終端を含まない。各 item 合計64 KiBの予算は残り item のメタデータを予約し、入らない本文は `output_byte_limit` とする。[入力検証](../../crates/server/src/remote_mcp/code_query.rs#L100-L120)・[範囲処理](../../crates/server/src/remote_mcp/code_query.rs#L348-L403)

成功 `status:text` と、`revision_mismatch`・範囲外・読取失敗・出力上限は同じバッチに混在できる。失敗 item は本文を返さず `complete:false` になるので、全 item の status を確認する。一方、不正な path / 範囲指定 / 対象関係、メタデータだけで予算超過、処理中に検出した checkout の branch / HEAD 不一致は要求全体のエラーである。[要求境界](../../crates/server/src/remote_mcp/code_query.rs#L134-L170)・[HEAD再照合](../../crates/server/src/remote_mcp/code_query.rs#L252-L257)・[部分失敗](../../crates/server/src/remote_mcp/code_query.rs#L348-L403)

ファイル revision は既存 text reader と同じ target・path・HEAD・text status・**観測した全文**の SHA-256 identity を使う。HEAD が変わらない dirty edit も追加取得時に不一致となる。不一致時は新たに検索または本文取得し、内容を見て範囲を選び直す。返された新 revision だけを無条件に差し替えない。応答の `snapshot:per_file_observation` は複数ファイルの atomic snapshot ではなく、同じファイルの重複範囲も独立に読む。読み終えた後の編集は次の revision 照合まで検出されず、同時の in-place write が一貫した全文になる保証もない。[互換 revision](../../crates/server/src/remote_mcp/code_query.rs#L173-L179)・[共通 hash](../../crates/server/src/remote_mcp/evidence_read.rs#L218-L226)・[既存 text reader](../../crates/server/src/remote_mcp/code_read.rs#L231-L251)・[範囲別の観測](../../crates/server/src/remote_mcp/code_query.rs#L363-L403)

### 変更時の確認範囲

検索用索引や snapshot 基盤を増やさず要求時の有界読取と既存 revision を再利用する構成は、[公開された利用契約](../../docs/development/chatgpt-mcp.md#focused-workspace-code-reads)と一致する。転送削減の fixture は小さな特定ケースの比較であり、一般の調査時間・削減率を保証しない。

回帰の入口は [filterとpaging、Unicodeとdirty revision](../../crates/server/src/remote_mcp/code_query.rs#L408-L581)、[除外・期限・出力予算・認可とsymlink](../../crates/server/src/remote_mcp/code_query.rs#L583-L711)、[巨大文脈・実表示範囲・JSON escaping・最小item境界・schema](../../crates/server/src/remote_mcp/code_query.rs#L812-L944)である。今回の Sync は凍結 source の MCP 登録・共通認可、Git reader、OS読取境界、これらのテスト内容から照合した。テスト再実行や実 ChatGPT / Auth0 接続は行っておらず、凍結入力で検証省略とされた 自動マージ を合格扱いにはしない。無関係な既存節全体の再検証を意味しない。

## MCP から Integration の検証方針を選ぶ

画面と対話では「自動マージ / Auto-merge」を使う。以下の tool 名・JSON key は互換識別子として保持される。[名称と互換境界の正本](../concepts/formal-integration.md#名称と互換境界)を参照する。

`start_integration` は既存の自動マージ 受付に、任意の `skip_validation` を渡す。`true` は設定済みの追加検証を省略する方針、`false` は追加検証を行う方針、未指定と `null` は受付時の server preference `integration_skip_validation` の継承である。この flag は既存検証証拠の有無を示さず、担当会話が実際の検証または根拠付き省略を記録する。証拠評価と保存上の NotRun の区別は [検証方針の正本](../concepts/formal-integration.md#検証方針は受付時に固定する)を参照する。文字列 `"true"` や数値は boolean として解釈しない。MCP 固有の検証 engine や保存先は作らず、domain が `IntegrationPayload.skip_validation` を固定する。[入力と domain への転送](../../crates/server/src/remote_mcp/launch.rs#L61-L74)・[受付呼出し](../../crates/server/src/remote_mcp/launch.rs#L513-L543)・[方針の固定](../../crates/server/src/routes/integrations/admission.rs#L242-L262)

受付成功 receipt の `result.skip_validation` と、`get_integration` の全 section（`summary` / `sources` / `validation`）の `skip_validation` は保存済みの解決済み boolean を返す。後から server preference が変わっても、再接続時に現在の設定から推測し直す必要はない。この値は検証件数と独立しており、ゼロ件を「省略」や「合格」と読み替えない。起動 receipt の `integration_accepted` は受付結果であり、自動マージ の完了、`published`、Wiki の確認結果は別に読む。[起動結果](../../crates/server/src/remote_mcp/launch.rs#L177-L189)・[保存値と検証件数の読取](../../crates/server/src/remote_mcp/integration_read.rs#L107-L175)

再送は同じ `request_id` と元の JSON 入力を保持する。比較するのは解決後の boolean ではなく、key 順を正規化した**元の入力全体**の hash である。そのため、同じ既定値へ解決される場合でも、省略から `null` への変更、flag の追加・削除・値変更は既存 receipt の入力不一致となる。domain 受付単独の省略再送規則と、MCP receipt の規則を混同しない。[fingerprint](../../crates/server/src/remote_mcp/operations.rs#L31-L45)・[入力照合](../../crates/server/src/remote_mcp/operations.rs#L210-L228)・[省略/null の再送テスト](../../crates/server/src/remote_mcp/launch_tests.rs#L444-L514)

この選択でも `lvk:read` と `lvk:merge`、Project / Repo / Issue / Workspace の関係、expected target / source commit の照合を通る。省略が変えるのは追加検証の方針であり、clean candidate、固定baseとsourceの祖先性、writer の収束、公開先mainの祖先性とCAS、公開と取消の排他、Wiki receipt は引き続き別の条件である。全体の lifecycle と未検証結果の意味は [自動マージ の検証・公開契約](../concepts/formal-integration.md#検証方針は受付時に固定する)を正本とする。[MCP の認可と関係検証](../../crates/server/src/remote_mcp/launch.rs#L423-L502)・[公開 guard](../../crates/server/src/routes/integrations/runtime.rs#L1258-L1381)

変更時には [schema の optional boolean/null](../../crates/server/src/remote_mcp/tests.rs#L299-L319)、[既定値二通り・四種の指定・再接続・全 section・不正型](../../crates/server/src/remote_mcp/launch_tests.rs#L444-L514)、[HTTP 経由の起動と read-only 取得](../../crates/server/src/remote_mcp/launch_tests.rs#L517-L588)、[domain の既定値解決](../../crates/server/src/routes/integrations/tests.rs#L357-L385)を確認する。HTTP discovery の試験は生成 schema の「追加検証」と既存証拠を区別する説明文も固定する。内部 stdio の [catalog fixture](../../crates/mcp/src/task_server/tools/stdio_tests.rs#L75-L83) は `start_integration` を公開しないことを確認するため、HTTP tool と stdio tool の可用性を同一視しない。これらは fixture による契約テストであり、実 ChatGPT/Auth0 接続の証拠ではない。この Sync では source とテスト内容を照合し、テストの再実行や live 接続確認は行っていない。

## MCP から Integration の会話と取消を扱う

自動マージは割当済みの一つのIssue / Workspace / Sessionで進める。質問や追加指示もその会話へ渡し、担当が候補・検証/省略・Wikiと公開要求を記録する。`get_integration` の `conversation` で各ID、候補SHA、証拠件数、公開要求と停止情報を確認する。[読取](../../crates/server/src/remote_mcp/integration_read.rs)・[自動マージの正本](../concepts/formal-integration.md)。

### 保存済み候補と証拠を読む

sectionは `summary` / `sources` / `validation` の三つである。`validation` は各証拠の `code_commit` を持ち、最終Wiki SHAの検証成功とは読み替えない。続頁は同じ `expected_revision` と `next_offset` を使い、1–50件の上限を守る。旧 `pending_gates` と `approve_integration` は現行入口にない。[読取とページング](../../crates/server/src/remote_mcp/integration_read.rs)。

### 受付履歴と現在の取消完了を分ける

`cancel_integration` は `lvk:read` + `lvk:merge` とProject所属を確認し、永続receipt経由でUI共通取消へ委譲する。adapterがIntegration状態を直接更新したり、Goalやコメントから承認を導いたりしない。取消は公開後のGit undoではなく、遅い取消のreceiptが `succeeded` でも新しい取消の受理を意味しない。結果の `cancel_requested`、`published`、`publication_intent` と現在状態を読む。[取消入口](../../crates/server/src/remote_mcp/integration_write.rs)。

`get_operation` は過去の受付結果、`get_integration(section="summary").lifecycle` は現在状態の確認に使う。lifecycleはrunと予約を同じSQLite snapshotで読み、`cancelled` かつ予約なしだけを `cancellation_complete:true` とする。terminalは cancelled / succeeded / failedで、summary全体や別要求とのatomic snapshotを保証しない。[同一snapshot](../../crates/server/src/remote_mcp/integration_write.rs#L46-L77)。

再送は同じprincipal・request ID・元の入力全体を保つ。取消の副作用を否定する型付き証拠がないエラーや、取消後の状態取得失敗は `uncertain` として読み戻す。受付結果は後から現在のterminal状態へ更新される履歴ではない。[操作と失敗分類](../../crates/server/src/remote_mcp/integration_write.rs#L79-L135)・[receipt](../../crates/server/src/remote_mcp/operations.rs)。

## 既存 Goal を MCP から操作する

HTTP MCP の `get_goal` と `pause_goal` / `resume_goal` / `cancel_goal` / `clear_goal` は、既存 Codex Goal を観測・制御する入口である。新規 objective の作成、達成の代理宣言、自動承認は提供しない。Goal と Run の寿命、Cancel と Clear の意味、保存 Goal の再開は [Session の Goal 契約](../concepts/session-and-agent-run.md#goal-の寿命)を正本とする。stdio の独立 catalog に同じ操作があると仮定しない。[公開契約](../../crates/server/src/remote_mcp/goals.rs#L75-L111)・[HTTP 登録](../../crates/server/src/remote_mcp/mod.rs#L374-L383)

読取には `lvk:read`、四つの変更操作には追加で `lvk:write` が必要である。既存の Project 認可・Workspace 所属検査、HTTP scope・access audit、永続 Operations を通り、`RuntimeActions` が UI 共通の command / Session follow-up へ委譲する。独自 Goal scheduler や直接の projection 書換えは作らない。[scope](../../crates/server/src/remote_mcp/mod.rs#L180-L210)・[監査と dispatch](../../crates/server/src/remote_mcp/mod.rs#L429-L476)・[共通 runtime](../../crates/server/src/remote_mcp/runtime_write.rs#L119-L127)・[受付](../../crates/server/src/remote_mcp/goals.rs#L271-L311)

### 対象と状態を固定する

`list_workspace_runs` で対象を選び、Project / Workspace / Session / AgentRun の四つの ID で `get_goal` を読む。これは UI と同じ**保存 projection**の観測で、provider への live RPC ではない。Goal status・budget・使用 token/時間、Run status・終端かどうか、両 projection status、`is_current_run` と revision を返す。過去 Run も読めるが、変更には最新 Run が必要である。objective 本文は `get_execution(section="goal")` で有界取得する。[観測](../../crates/server/src/remote_mcp/goals.rs#L75-L79)・[保存値と revision](../../crates/server/src/remote_mcp/goals.rs#L123-L155)

変更には同じ四 ID、UUID `request_id`、観測した `expected_revision` を渡す。revision は Run ID・Run の updated_at・投影 event sequence を結びつける。Codex かつ両 projection が `current`、Project・Workspace・Session の所属一致、最新 Run、非 archive の interactive Workspace を要求する。競合後に新 revision だけを差し替えず、対象状態を読み直して操作を判断する。[検証](../../crates/server/src/remote_mcp/goals.rs#L159-L190)・[共通 Run guard](../../crates/server/src/remote_mcp/runtime_write.rs#L242-L288)

| 操作 | 受付可能な状態 |
| --- | --- |
| pause | 非終端 Run の `active` Goal |
| resume | 非終端 Run では `paused` / `blocked` / `usageLimited` / `budgetLimited`。終端 Run では Goal が存在し `complete` でないこと |
| cancel / clear | 非終端 Run で Goal が存在すること |

上表に加え、共通 Run guard は非終端操作を pending / starting / running / awaiting_input / awaiting_approval に限定する。取消中を含むその他の状態を「非終端だから操作可能」と読み替えない。[状態行列](../../crates/server/src/remote_mcp/goals.rs#L159-L190)・[Run status の許可集合](../../crates/server/src/remote_mcp/runtime_write.rs#L265-L280)。終端 Run の resume が新 Run を開始するときの lock 受渡しと binding 欠落拒否は [保存 Goal の再開](../concepts/session-and-agent-run.md#保存-goal-の再開)に従う。

### 受付・配送・遷移を読み分ける

1. 元の JSON 入力と `request_id` を保持し、`get_operation` で receipt を回復する。同一 principal・request ID の同一入力は一度だけ dispatch し、異なる操作や入力への使い回しは拒否する。再送は [共通 receipt 契約](#参照結果と操作-receipt)に従う。
2. 活動中 Run への操作は `result.command_id` を使って `get_goal` の `command_delivery_status` を読む。dispatch 後に結果が不明なら receipt の `operation_id` が command ID に対応する。配送状態と Goal 遷移・Run 終端は別に確認する。
3. 終端 Run の再開は command 型の操作ではなく、新しい `result.agent_run_id` の状態を読む。`execution_accepted` は起動受付で、Goal 遷移の確認ではない。

事前検証失敗は `failed` / `goal_state_conflict` となる。一方、runtime dispatch 呼出しがエラーになった場合は、副作用がないと断定できないため `uncertain` / `dispatch_outcome_unavailable` となる。配送済みかもしれない操作を新 ID で自動再実行せず、receipt と対象 Run を照合する。成功結果も `goal_transition_confirmed:false` を返し、command 経路の `termination_confirmed:false` と再開経路の `run_terminal:false` を終端証拠に置き換えない。[結果と失敗分類](../../crates/server/src/remote_mcp/goals.rs#L250-L311)・[command identity](../../crates/server/src/remote_mcp/runtime_write.rs#L99-L106)・[再送テスト](../../crates/server/src/remote_mcp/goals/tests.rs#L25-L54)・[uncertain の非再配送](../../crates/server/src/remote_mcp/goals/tests.rs#L189-L208)

Goal が projection に存在しないという観測だけでは、provider の Goal 解除や process 終了は証明できない。[観測の限界](../../crates/server/src/remote_mcp/goals.rs#L145-L155)。変更時は [状態・所属・再送の fixture](../../crates/server/src/remote_mcp/goals/tests.rs)、[HTTP の四操作 write-scope 拒否](../../crates/server/src/remote_mcp/tests.rs#L315-L346)、[Session の実共有経路と検証限界](../concepts/session-and-agent-run.md#goal-操作の検証境界)を併せて確認する。今回の Sync は静的照合であり、実 Codex/Auth0 接続や製品試験の再実行を意味しない。

## 保存済み Codex 進捗の読取

`list_run_events` は UI と同じ canonical `agent_events` の保存済み進捗を read scope で抜粋する。新しい collector・保存先・live delta stream・subscription は作らず、native audit も読まない。最終回答は `get_execution`、通知は [Web Push](web-push.md)という別の利用経路である。[読取契約](../../crates/server/src/remote_mcp/run_events.rs#L1-L48)

Project / Workspace / Session / AgentRun は必須で、任意の Issue ID は所属の assertion である。Run・Session・Workspace・現在の Issue link、cursor と page は同じ DB read snapshot で照合する。過去 Run も読めるが Codex に限定され、別 Run の cursor や消失した cursor は再一覧を要求するエラーとなる。[scope と cursor](../../crates/server/src/remote_mcp/run_events.rs#L50-L84)。active member の全 Project access という既存認可を利用し、非公開 Project ACL を新設しない。

### 初回窓と継続 cursor

`limit` は1–50件の**走査レコード数**であり表示件数ではない。順序は attempt 番号、canonical sequence。既定 `start="beginning"` は先頭から、cursor が null / 省略の `start="latest"` は同じ snapshot の末尾 limit 件を選び、昇順で返す。末尾選択は privacy 投影の前なので、非公開 record だけで items が空になることもある。過去を飛ばしたことだけでは `has_more` を立てない。[窓の選択](../../crates/server/src/remote_mcp/run_events.rs#L85-L100)

非 null cursor は start より優先する。`next_cursor` は最後に走査した event UUID なので、除外だけの空 page に `has_more=true` があっても続きを読む。末尾で `has_more=false` でも cursor を保持すれば、別接続から追加入力を読める。128 KiB の item 合計上限に当たった未読レコードの手前で止め、前の cursor を残して has_more を立てる。初回 latest が完全に空で `next_cursor=null` なら、次は beginning に切り替える。latest のまま再読すると、その間の到着が limit を超えた場合に古い到着を飛ばし得る。[走査と byte budget](../../crates/server/src/remote_mcp/run_events.rs#L101-L123)・[初回空窓の回帰](../../crates/server/src/remote_mcp/run_events.rs#L454-L510)

### 公開本文と credential 形状の境界

公開するのは assistant commentary、lifecycle、許可済み tool 名と status、Goal status、子活動の限定 kind だけである。final/user/system text、全 Thinking（歴史的 Plan text と reasoning を安全に区別できない）、承認・回答本文、tool 引数・結果・call ID、provider extension、native reference を出さない。schema/payload version が1以外、32 KiB超の envelope、不正 JSON は省略する。[allowlist](../../crates/server/src/remote_mcp/run_events.rs#L85-L100)・[投影](../../crates/server/src/remote_mcp/run_events.rs#L126-L196)

通常の path・symbol・一般語・長い ASCII ID は読める。commentary の全文を先に走査し、資格情報形状または改行/tab以外の制御文字があれば本文全体を `content=null, content_withheld=true` とする。安全判定後に UTF-8 境界を守って最大4096バイトへ切り詰め、`content_truncated` で短縮を示す。表示範囲より後ろに資格情報形状がある場合も全文 withheld となる。[処理順序](../../crates/server/src/remote_mcp/run_events.rs#L129-L146)・[全文検査](../../crates/server/src/remote_mcp/run_events.rs#L199-L270)

| 認識する形状 | 閾値と境界 |
| --- | --- |
| `sk-` / `ghp_` / `github_pat_` | 大小文字を区別し、接頭辞後のASCII英数字・underscore・hyphenが各20 / 36 / 50文字以上。直前が同じtoken文字なら認識せず、task ID等への誤検知を避ける |
| Bearer | 大小文字を区別しない `bearer `、直前がASCII英数字ではない境界、後続token文字20以上 |
| JWT様 | `eyJ`で始まるheaderが12以上、payloadが8以上、signatureが16以上の3つのbase64url様segment |
| PEM | 大文字とspaceの `PRIVATE KEY` またはその修飾label。certificate/public keyを同じ理由で除外しない |

これは既に利用者へ見える prose のための限定 heuristic であり、任意の秘密を除去する保証ではない。返された文章も命令・承認・独立した検証証拠ではない。広範な語や記号で全文を隠さず credential 形状へ絞る意図は [読取仕様](../../docs/development/chatgpt-mcp.md)に記録され、実装は依存や別共有モジュールを追加せずこの投影内に置かれている。[形状判定](../../crates/server/src/remote_mcp/run_events.rs#L199-L270)

変更時は通常prose、credential閾値、切り詰め後方のcredential、UTF-8、初回空latest、filtered tail、cursor優先、attempt順、byte/envelope境界、DB reopenとHTTP read scope/reconnectを確認する。[projection / paging tests](../../crates/server/src/remote_mcp/run_events.rs#L294-L896)・[HTTP 境界](../../crates/server/src/remote_mcp/tests.rs#L1720-L1790)。fixture の存在や供給された過去の成功報告を、今回の製品試験や実 ChatGPT account 接続成功と読み替えない。

## オンデマンド資源観測

<!-- openwiki: broken internal link [../architecture/web-and-sync.md#cpuramdisk-の共通観測を表示する] heading anchor "cpuramdisk-の共通観測を表示する" does not exist in "../architecture/web-and-sync.md". Fix the href or restore the target, then delete this comment. -->
`get_resource_observation` は外部クライアントが負荷判断の材料を得るための server 全体の読取であり、入力は `{}`、必要な scope は既存の `lvk:read` である。Project ID、任意 path、command は受け付けない。通常の認証・MCP audit を通り、collector は固定 Linux 統計を読む。collector は `server::resource_observation` にあり、認証付き Web API と [資源ヘッダー](../architecture/web-and-sync.md#cpuramdisk-の共通観測を表示する)も同じ観測経路を使う。内部モデル・監視 daemon・履歴 DB・自動 admission / stop policy は持たない。観測は [共有資源の所有権と実行受付](../concepts/shared-resource-coordination.md)を代替しない。[tool と入力](../../crates/server/src/remote_mcp/resource_observation.rs#L9-L20)・[認可と dispatch](../../crates/server/src/remote_mcp/mod.rs#L393-L449)

### 可視範囲と値の意味

schema version 2 は収集開始の `timestamp`、終了応答の `completed_at`、`scope`、`system_visible`、`cgroup`、`disks`、`completion`、`errors` を返す。ファイルを順番に読むため atomic snapshot ではない。`scope.statistics` は常に `system-visible`、environment と physical-host visibility は `unknown` である。VPS / container の外側まで見えているとは断定できず、system-visible 使用率を container 内の使用率や実行に使える余力へ読み替えない。[応答初期値と順次収集](../../crates/server/src/resource_observation.rs#L41-L134)

| 観測 | 解釈 |
| --- | --- |
| memory / swap bytes | `/proc/meminfo` の total、MemAvailable、swap total と total − free。MemAvailable は MemFree とは異なる |
| online CPU count | online logical CPU 数であり、idle CPU 数や割当保証ではない |
| load average | 1 / 5 / 15 分平均。CPU percent ではない |
| CPU usage | `/proc/stat` の aggregate counter 差分。約500 ms待ち、実測 `sampling_duration_ms` を返す。idle と iowait を idle と扱い、guest を二重計上しない。counter の逆行・差分ゼロは unknown |

[収集と sample](../../crates/server/src/resource_observation.rs#L63-L134)・[時間計測](../../crates/server/src/resource_observation.rs#L171-L195)・[CPU 算出](../../crates/server/src/resource_observation/linux.rs#L134-L162)

cgroup は別枠で、process membership が `0::/`、mountinfo が `/sys/fs/cgroup` の cgroup v2 root mount を確認できた場合だけ固定 namespace-root files を読む。memory current / max、CPU quota÷period、effective cpuset 数と `effective_cpu_bound_cores` を返す。CPU bound は online count・quota・cpuset の最小値で、quota の明示 unlimited は上限を加えず、必要入力の unknown は結果にも伝わる。v1、nested membership、別 mount layout を mount-root の値で代用しない。非可視の祖先制限は unknown のままで、root の `max` は全祖先の無制限を証明しない。memory headroom の推定はしない。[限定 layout と値](../../crates/server/src/resource_observation.rs#L98-L142)・[quota / bound / mount 検証](../../crates/server/src/resource_observation/linux.rs#L50-L116)

### DISK の対象と読み取り専用の解決

`disks` は `worktree_base` 一件だけを返す。設定された worktree base（override を含む）の directory handle に `fstatvfs` を行うため、これは worktree 配下のファイルサイズ合計ではなく、その保存先 filesystem 全体の使用量である。data directory は観測しない。schema v2 は対象数と使用率の意味の変更を表すため、consumer は旧配列位置ではなく target で選択する。[単一対象](../../crates/server/src/resource_observation/disk.rs#L8-L30)・[v2 の移行契約](../../docs/development/chatgpt-mcp.md#observe-server-resources-on-demand)

<!-- openwiki: broken internal link [../architecture/web-and-sync.md#cpuramdisk-の共通観測を表示する] heading anchor "cpuramdisk-の共通観測を表示する" does not exist in "../architecture/web-and-sync.md". Fix the href or restore the target, then delete this comment. -->
既知の観測は `total_bytes`、`used_bytes`、`available_bytes`、`used_percent`、`percent_basis: "used / total"`、文字列の device ID `filesystem_id` を持つ。total は `f_blocks * f_frsize`、free は `f_bfree * f_frsize`、used は total − free、percent は `used / total * 100` であり、量と率が同じ物理 filesystem 総容量を基準にする。一般ユーザーが使える `available_bytes` は `f_bavail * f_frsize` の生 API 値として保持するが、分母には使わない。例えば total=1000、free=300、available=200 なら used=700、70% となる。「物理容量」はこの filesystem の分母の意味であり、上記の physical-host visibility が既知になるわけではない。[算式・counter 読取](../../crates/server/src/resource_observation/disk.rs#L32-L73)・[予約領域が異なる例と0/100%](../../crates/server/src/resource_observation/disk.rs#L83-L97)。表示単位と不整合値の扱いは [Web の表示契約](../architecture/web-and-sync.md#cpuramdisk-の共通観測を表示する)へ進む。

保存先は directory を作らない resolver で求め、不在を親directoryの値で代用しない。DirectFolder や配下の別 mount を網羅する測定でもない。欠落・非directory・読取失敗・counter overflow、total=0、free>total、available>free は DISK を unknown にする。他の成功した観測値は保持する。[読み取り専用のパス解決](../../crates/utils/src/path.rs#L117-L125)・[worktree base](../../crates/worktree-manager/src/worktree_manager.rs#L618-L624)・[失敗条件](../../crates/server/src/resource_observation/disk.rs#L32-L79)・[単一対象と非作成の試験](../../crates/server/src/resource_observation/disk.rs#L99-L137)

### 部分失敗と worker の寿命

数値は `known` と value、または `unknown` と定型 error を持つ。limit の明示 `max` だけが `unlimited` であり、欠落・不正・読取失敗をゼロや unlimited に変換しない。`completion: complete` は収集終了であり、全 field が known という意味ではない。timeout は受信済みの値を保持して `errors: ["timeout"]`、done のない worker 終了は `worker_failed` を返す。CPU sample が未到着なら duration は null のままである。[field と初期値](../../crates/server/src/resource_observation.rs#L33-L61)・[受信と失敗](../../crates/server/src/resource_observation.rs#L144-L169)

server process ごとに専用 worker thread は最大2本で、待機 queue はなく、満杯なら retry を促す tool error となる。収集応答の deadline は2秒、各 file の受入上限は64 KiB（超過検出用に1 byte追加読取）、観測 JSON は8 KiB上限である。sampling は Tokio 共通 blocking executor を占有しない。permit は worker 自身が所有し、呼出側の切断・timeout 後も thread が終了するまで保持する。この設計は stuck kernel read による無制限の thread 増加を防ぐが、サーバー自体が停止すれば応答できない。生ファイル、process inventory、command line、任意 path は結果に出さない。[上限](../../crates/server/src/resource_observation.rs#L10-L31)・[worker 所有権](../../crates/server/src/resource_observation.rs#L197-L220)

変更時には [parser の異常値・mount・CPU差分テスト](../../crates/server/src/resource_observation/linux.rs#L164-L232)、[部分値・timeout・permit保持・JSON上限・catalog テスト](../../crates/server/src/resource_observation.rs#L220-L361)、[HTTP read scope と入力境界](../../crates/server/src/remote_mcp/tests.rs#L1493-L1560)を照合する。限定 layout と可視性を保守的に扱う理由は [運用契約](../../docs/development/chatgpt-mcp.md#observe-server-resources-on-demand)にも記録されている。`live_smoke` は明示実行用の ignored test である。[開発時の検証記録](../../tests/resource-observation/VALIDATION.md)には単一 DISK の対象試験と smoke の成功、full-app harness の更新と構文確認が記録されているが、新仕様の full-app E2E は再実行されていない。今回の Sync は固定 source `8ea251d0bc234497e69e20588420900d011fddf3` とテスト定義の静的照合であり、供給された自動マージ は明示的な検証省略だった。テスト再実行、実 Auth0 / ChatGPT 接続、実際に異なる mount、他 OS の検証は行っておらず、無関係な既存節の再検証も意味しない。

## 外部実行とモバイル設計

Windows Runnerはexecution slot等のcanonical Resourceをclaimし、immutable source snapshotとinteractive commandを使う。接続認証・containment・settlementの責任はMCP bearerや旧paired hostと異なる。[Runnerの所有・回復](../concepts/shared-resource-coordination.md#外部-runner-と-interactive-operation)。

[AI Mobile Design](../../docs/ai-mobile/design.md)はv0.1 draftである。web/PWA中心という意図は読めるが、記載されたremote-web / relay baselineやQR pairing案は現行実装の証拠にならない。現在のスマートフォンからの入口は直接local-webであり、UI確認には [built preview](../operations/development.md)を使う。外部IdP、HTTPS経路、Windows hardwareの実接続はこの静的source照合では試験していない。

<!-- openwiki: broken internal link [../concepts/runtime-access.md#開発-instance-の専用自己再起動] heading anchor "開発-instance-の専用自己再起動" does not exist in "../concepts/runtime-access.md". Fix the href or restore the target, then delete this comment. -->
自己再起動は通常 run_process と別の [専用 handoff と結果照会](../concepts/runtime-access.md#開発-instance-の専用自己再起動)を使う。一般 operation receipt の成功は切替成功を証明しない。

## Issueの分類・検索・archiveを操作する

外部HTTPのIssue get/list/create/updateはpriorityとarchive状態を共有Boardへ接続する。`list_tags` / `list_issue_tags` は最大50件のcursorページ、`add_issue_tag` / `remove_issue_tag` はread+writeとstable request_idのreceiptで一件だけ変更する。createのtag_idsはIssue採番・INSERTと同じtransactionに入り、失敗は全部rollbackする。systemは手動変更できず、分類に実行承認の意味を与えない。[分類の正本](../concepts/project-and-issue.md#仕事の識別と分類)・[tool契約](../../crates/server/src/remote_mcp/issue_tags.rs#L44-L77)・[原子的作成](../../crates/server/src/routes/local_remote.rs#L789-L855)。

両MCPのIssue一覧はarchived省略/falseで未archive、trueでarchive済み、`"all"`で両方を選ぶ。searchはtitle/description全文の小文字化部分一致で空白をtrimしない。simple_idは大小無視の完全一致、statusは名前またはUUID、priority/parent/assignee/Tag条件はANDとなる。tag_idとtag_nameは同じTagを指す必要があり、別Tagを両方持つIssueでも不一致は0件である。既存Statusを読むだけで初期列を追加しない。[共通入力](../../crates/api-types/src/issue_filter.rs#L7-L67)・[read-only解決](../../crates/server/src/routes/local_remote/board_api/issue_filter.rs#L9-L74)。

共有するのはfilter解釈で、応答とpagingは統一しない。stdioはsort/offset/countを保持し、外部HTTPはUUID cursorと既存revision/url/metadataを保つ。filterを適用してからページ化し、同じ条件でcursorを進める。HTTPのarchive-only SQL fast pathにも同じarchive predicateを適用する。MCPのarchived条件はUIのhidden Status filterではない。[外部ページ化](../../crates/server/src/remote_mcp/read.rs#L178-L228)・[stdio用filter API](../../crates/server/src/routes/local_remote/board_api/issues.rs#L89-L113)。Todo/設計メモのbrowser検索はtrimするため、MCP検索と同じ入力正規化と扱わない。[Todo検索](../concepts/project-todos.md#一覧を検索する)・[設計メモ検索](../concepts/design-notes.md#一覧検索と下書きの関係)。

Issue単位のarchiveは `prepare_issue_archive` でcheckboxと全対象を一度示し、人の確認後 `confirm_issue_archive` をstable request_idで送る。両段階にread+writeと所属確認が必要で、tokenはLocal/UI/stdioと交換できない。`unarchive_issue` はIssueだけを復帰させる。busy・ABA・削除結果・uncertainの詳細は [Issue archiveの正本](../concepts/project-and-issue.md#issue-のアーカイブ)に従い、結果不明時はget_operationと現在値を照合する。[adapter](../../crates/server/src/remote_mcp/issue_archive.rs#L41-L140)。

変更時はshared filterとHTTP cursor matrix、stdio adapter/codec、Tag receipt/atomic create、archiveの署名JWT fixtureを別々に確認する。mock peerやisolated browserの成功は実Auth0/ChatGPT接続の成功とは別である。

## MCPで素材ファイルを渡す

Remote/stdioは既存Attachment/FileServiceを共用する。`upload_attachment` は認可済みProjectへbytesを保存し、`place_attachments` はProject/Workspace/Sessionの所属を検証してSession作業ディレクトリへ配置する。保存・配置だけではRunを作らない。添付はWorkspace共有で、Session専有の秘密領域ではない。[共有API](../../crates/server/src/routes/attachment_transfer.rs)・[stdio登録](../../crates/mcp/src/task_server/tools/attachments.rs)

Remoteはホスト提供の `file` をfileParamsとして宣言し、download_url/file_idを必須、mime_type/file_nameを任意propertyにする。Library IDやクライアント内パスだけではbytesを受け取れない。inline base64は両transportでdecoded 2MiB、ホスト取得は20MiB、添付JSON本体は4MiBである。HTTPS/443・operatorの完全一致host allowlist・公開IPへのDNS固定・redirect禁止・取得timeoutを適用し、任意URL fetchにはしない。既定hostは `files.oaiusercontent.com` で、実ホスト適合は未検証である。[catalog](../../crates/server/src/remote_mcp/attachments.rs#L5-L29)・[入力と取得検証](../../crates/server/src/routes/attachment_transfer.rs#L23-L175)・[運用契約](../../docs/development/mcp-attachments.md)

### 許可されていない添付ホストを診断する

host allowlistで拒否した場合、Remote MCPとupload APIは、安全に表現できる正規化済みhostnameだけを `Host file domain is not enabled (hostname: …)` として返す。専用の非公開error型が保持するのはhostnameのみで、ASCII英数字・ハイフン・ドット、全体253 bytes以内、各label 1〜63 bytesに限定する。URL parseの元errorは保持せず、不正なURL、userinfo・fragment・不許可scheme/port、表現できないhostnameには静的な拒否を返す。[hostnameの抽出と拒否](../../crates/server/src/routes/attachment_transfer.rs#L106-L161)・[upload API](../../crates/server/src/routes/attachment_transfer.rs#L534-L543)

公開error境界が動的に表示するのはこの専用型だけで、一般のdomain/IO/SQL errorやcontext chainは引き続き隠す。許可リストや取得時のセキュリティ条件を変更する機能ではない。隔離fixtureはhostname以外のURL・path・query・file_idが応答や監査に現れず、拒否で添付・operation receiptが作られないことを検査する。実ChatGPTのhostnameや実搬送の成功は未確認であり、配備後の再試行で確認する。[公開境界](../../crates/server/src/remote_mcp/errors.rs#L1-L12)・[回帰fixture](../../crates/server/src/remote_mcp/attachment_tests.rs#L322-L341)・[安全でない入力の回帰](../../crates/server/src/routes/attachment_transfer.rs#L596-L650)

### 読めることを確認してから実行する

新規startと既存続行のattachment_idsは、すべて利用可能なIDで、配置先のサイズ・SHA-256が一致することを実行受付前に確認する。欠落・改変・symlinkや部分失敗を無視せず、部分配置は各結果とall_verifiedを返し、一部失敗なら実行しない。改変された既存先を勝手に上書きしない。Remoteのcontinue/steerは既存の最新Run・状態・lock契約を保つ。stdio添付付きstartはrequest_id、続行はproject_id/request_idを必要とする。[Remote受付](../../crates/server/src/remote_mcp/runtime_write.rs#L432-L460)・[stdio](../../crates/mcp/src/task_server/tools/task_attempts.rs#L210-L230)・[配置の正本](../operations/workspace-inspection.md#session-の添付)

保存receiptのidentityは取得したbytesのSHA-256とProject・名前から作り、期限付きURLを含めない。URL変更後も同じ内容なら再利用するが、成功済みrequestでも再送時は再取得して内容を比較する。取得不能ならRemoteのget_operation、stdioのget_attachment_operationで元request_idのreceiptを回収する。request_id/file_idだけからbytes同一性を推定しない。URL・base64・file_idをreceiptに残さない。[保存とreceipt](../../crates/server/src/routes/attachment_transfer.rs#L175-L270)

Project参照がある未配置ファイルは既存孤立掃除から保護され、参照がある限り無期限保持となる。TTL・定期GC・総保存quotaは追加していない。per-file上限を総ディスク容量の保証と取り違えない。[保持と清掃](../../crates/services/src/services/file.rs)・[参照migration](../../crates/db/migrations/20261029160000_project_attachments.sql)

隔離HTTP/stdio E2Eは実bytes・権限・所属・再送・部分失敗から初回/続行までを確認し、native実行境界をmockする。実ChatGPT/Library fileParams、署名URL期限・host/proxy制約、実Codexは公開後ゲートであり、この成功で完成と呼ばない。配備後にtools/listと配布Skillを更新して実搬送を確認する。[検証・公開後手順](../../docs/development/mcp-attachments.md#検証と公開後ゲート)・[隔離fixture](../../crates/server/src/remote_mcp/attachment_tests.rs)

## MCPからREVIEWを指定・観測する

Codexのstart_workspaceは `execution_mode: review` を受け取り、続行では保存済みmodeと権限を維持する。Remoteのget_executionは指定Runのattemptのmodeを返す。REVIEWは会話で調査結果を返して止まり、製品実装への別指示を必要とする。承認回避や外部承認改善を保証するmodeではない。[開始](../../crates/server/src/remote_mcp/launch.rs#L336-L348)・[読取](../../crates/server/src/remote_mcp/runtime_read.rs#L137-L183)・[実行境界](agent-providers.md#review-の実行境界)

## 全MCP共通の失敗・結果不明・復旧契約

HTTP MCPとstdio MCPは共通の `Diagnostic` でcategory、outcome、recovery、message、correlation_idを返す。validation/conflict/permission/not_found/busy/unavailable/internal等の分類は、副作用の有無とは独立する。HTTPの即時tool失敗は既存isErrorを保ち `structuredContent.error` を加え、stdioは `success:false` / `error` / `diagnostic` をcontentとstructuredContentに返す。認証の標準JSON-RPC/OAuth challenge、未知toolとschema validationの標準契約は維持する。[共通型](../../crates/utils/src/mcp_error.rs)・[HTTP dispatch](../../crates/server/src/remote_mcp/mod.rs)・[stdio helper](../../crates/mcp/src/task_server/tools/mod.rs)

副作用前の拒否やtransaction rollbackの証拠があればfailed、commit・delivery・実行結果を確定できなければunknown/uncertainとなる。汎用HTTPのmutation失敗は401/403以外のstatusだけで未commitと推定しない。POST読取のrepository query/provider usageとintegration inspectは読取として扱う。結果不明では元requestと同じ入力でreceiptを回収し、対象状態を読戻してから判断する。新requestによる自動リトライは追加しない。[stdio確定性](../../crates/mcp/src/task_server/tools/mod.rs)・[transaction分類](../../crates/server/src/remote_mcp/todos.rs)・[元requestの保持](../../crates/server/src/remote_mcp/operations.rs)

receipt保存失敗やpanic後もdomain成功の可能性を維持し、同process内の不確実性と再起動後の旧pending回復を分ける。破損diagnosticは許可した型から安全に再構成し、uncertainならunknownを優先する。SQL・stack・credential・入力値・内部pathの生文は出さず、失敗のtarget/部分結果に含まれるpath/cwd/command/本文等を秘匿する。ID・状態・revision・部分作用は残し、成功時に要求された内容の応答は変えない。[耐久受付](../../crates/server/src/remote_mcp/operations.rs)・[receipt投影](../../crates/server/src/remote_mcp/issue_write.rs)・[秘匿](../../crates/utils/src/mcp_error.rs)

横断回帰はHTTPの `remote_mcp`、stdioの `error_contract_tests` / `stdio_tests`、DBのTODO/設計メモtransaction fixtureを入口とする。実行済み検証のSHA・結果と、fixture定義の存在は区別する。[契約の対応表](../../docs/development/mcp-error-contract.md)
