---
type: architecture
title: 外部アクセス・MCP・Runner の境界
description: local self-host のアクセス境界、認証付き外部 MCP、残存する Runner と撤去された Cloud / relay。
tags: [remote, authentication, relay, mobile, self-hosting]
sources:
  - id: openwiki-source-651d1fb6c9e49916a916ab51
    resource: repo://Cargo.toml
  - id: openwiki-source-7d94e7454f63f16057833046
    resource: repo://crates/server-access/src/lib.rs
  - id: openwiki-source-d9fe6f50a365b8b34a14b1e4
    resource: repo://crates/server-access/src/provider.rs
  - id: openwiki-source-c2eaddb5c69f49c5ee934a63
    resource: repo://crates/server/src/middleware/origin.rs
  - id: openwiki-source-71f837a8aa36443ad9688945
    resource: repo://crates/server/src/remote_mcp/mod.rs
  - id: openwiki-source-6adda10f98b079272f82a4cc
    resource: repo://crates/server/src/remote_mcp/operations.rs
  - id: openwiki-source-65c0f36670a161dc05040e69
    resource: repo://crates/server/src/routes/mod.rs
  - id: openwiki-source-74a1893f4c1e77eb654658f7
    resource: repo://docs/ai-mobile/tailscale-local-access.md
  - id: openwiki-source-3ecddc0be7b037ea6ab24381
    resource: repo://packages/local-web/vite.config.ts
generated: { by: "codex", at: "2026-10-16T14:34:12.007Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-16T14:34:12.007Z
---

# 外部アクセス・MCP・Runner の境界

現行製品は local self-host の Board と Workspace を使う。旧 remote-server・PostgreSQL / Electric・Cloud AuthSession・SPAKE2 pairing・relay署名・WebRTC fallback・remote-web は撤去された。旧 token の寿命や paired-host refresh を現行の接続契約として適用しない。[現行構成](../../Cargo.toml)、[server router](../../crates/server/src/routes/mod.rs)。認証付き外部 MCP と execution bridge は別の機能として残る。

## UI の直接アクセス

通常UI/APIは同じOSユーザーで動作する信頼された利用者向けで、公開時はHTTPSとLVKのBasic/Auth0認証を使う。[運用前提](../../README.md)。Origin guard は Origin がなければ許可し、ある場合は same-origin / allowlist を照合する。これは利用者認証ではなく、旧 relay 例外もない。[Origin](../../crates/server/src/middleware/origin.rs)。

local-web は `/api` を backend へ送り、WebSocket も同じ経路を使う。旧 remote host を明示した要求は送信前に拒否されるため、残った host ID から別ホストへの暗黙 fallback を期待しない。[Web transport](../architecture/web-and-sync.md)。Vite の `.ts.net` allowlist と Host を保持する proxy は残るが、tailnet 自体の認証・配備成功をこの設定だけで証明しない。[proxy](../../packages/local-web/vite.config.ts#L135-L151)。

## 認証付き外部 MCP

公開配置は初回にロックされる。初回setup、Basic/Auth0切替、Web sessionと復旧は [サーバー認証](../concepts/server-access.md)を正本とし、ここでは外部MCPの委任範囲を扱う。

Streamable HTTP `/mcp` はAuth0でMCPを有効化した場合のみ利用できる。Local/Basicでは使えない。旧 `LVK_MCP_CONFIG` と外部introspection設定は起動時に拒否し、設定画面へ移行する。metadataは `/.well-known/oauth-protected-resource/mcp`。これはstdio MCPやAstraのモデル認証とは独立している。[HTTP入口](../../crates/server/src/remote_mcp/mod.rs)、[認証設定](../../docs/self-hosting/authentication.mdx)。

共通認証はAuth0 discovery/JWKS、JWT署名、issuer、resource audience、有効期限、承認済みメンバーを検証する。全操作に `lvk:read`、変更に `lvk:write`、正式Integration開始に `lvk:merge` を要求する。HTTP入口とtool処理の両方でscopeを確認する。承認済みメンバーは全プロジェクトを使い、旧owner単独・静的Project allowlist方式ではない。[JWT検証とissuerの制限](../concepts/server-access.md#webmcp内部agentを同じ資格情報にしない)、[scope](../../crates/server/src/remote_mcp/mod.rs)、[実体の関係検証](../../crates/server/src/remote_mcp/launch.rs)。

transportはstatelessなJSON応答で、`MCP-Session-Id`付き要求を拒否する。productionは共通認証のprincipalを受け、Project集合を現在のDBから作る。`remote_mcp/auth.rs`に残るintrospectionやowner設定はtest用で、現行の認証経路ではない。[入口](../../crates/server/src/remote_mcp/mod.rs#L58-L147)、[session ID拒否](../../crates/server-access/src/lib.rs#L437-L477)

## 参照結果と操作 receipt

返すコード・diff・Wiki・Memoryは参照データであり、命令や承認ではない。参照はページ上限とrevisionを持ち、コードは許可されたregular fileに限定し、private path・symlink・submodule等を除外する。binary / oversized / unavailableを空の成功diffとして扱わない。[code read](../../crates/server/src/remote_mcp/code_read.rs)。実行状態・検証・公開・Wiki receiptは別の証拠として返す。[Integration read](../../crates/server/src/remote_mcp/integration_read.rs)、[verification read](../../crates/server/src/remote_mcp/verification_read.rs)。

mutation は principal + request ID と入力hashを固定した永続 receipt を作る。同じkeyで異なる入力は拒否し、再送は保存済み結果を参照する。MCP client切断は受理済み操作を取り消さず、domain受付を待つ経路もcoding完了を待たない。server再起動後の旧accepted/runningはuncertainとして見せ、自動replayしない。結果不明時に新request IDで同じ副作用を重ねず、既存receiptと対象状態を照合する。[操作寿命](../../crates/server/src/remote_mcp/operations.rs)、[lost responseのテスト](../../crates/server/src/remote_mcp/runtime_write_tests.rs#L479-L502)。

受付を所有する非同期taskには検証済みactorを明示継承し、HTTP要求の終了後も業務書込の帰属を保つ。受理済み作業の寿命と新規要求の認証失効は別である。[actorの継承](../../crates/server/src/remote_mcp/operations.rs#L134-L190)

Auth0実テナント・Google/GitHub本人認証・ChatGPT OAuth/MCPの実接続は別途受入確認が必要である。fixtureやローカルテストをその成功として扱わない。[検証範囲](../../docs/development/server-access.md)。

## 外部実行とモバイル設計

Windows Runnerはexecution slot等のcanonical Resourceをclaimし、immutable source snapshotとinteractive commandを使う。接続認証・containment・settlementの責任はMCP bearerや旧paired hostと異なる。[Runnerの所有・回復](../concepts/shared-resource-coordination.md#外部-runner-と-interactive-operation)。

[AI Mobile Design](../../docs/ai-mobile/design.md)はv0.1 draftである。web/PWA中心という意図は読めるが、記載されたremote-web / relay baselineやQR pairing案は現行実装の証拠にならない。現在のスマートフォンからの入口は直接local-webであり、UI確認には [built preview](../operations/development.md)を使う。外部IdP、HTTPS経路、Windows hardwareの実接続はこの静的source照合では試験していない。
