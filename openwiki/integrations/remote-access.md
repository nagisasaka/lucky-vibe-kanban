---
type: architecture
title: 外部アクセス・MCP・Runner の境界
description: local self-host のアクセス境界、認証付き外部 MCP、残存する Runner と撤去された Cloud / relay。
tags: [remote, authentication, relay, mobile, self-hosting]
sources:
  - id: openwiki-source-651d1fb6c9e49916a916ab51
    resource: repo://Cargo.toml
  - id: openwiki-source-c2eaddb5c69f49c5ee934a63
    resource: repo://crates/server/src/middleware/origin.rs
  - id: openwiki-source-0efe022da63bfbe33cd95498
    resource: repo://crates/server/src/remote_mcp/auth.rs
  - id: openwiki-source-e8ffcc68baa749ea9aa59c4d
    resource: repo://crates/server/src/remote_mcp/config.rs
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
generated: { by: "codex", at: "2026-10-16T12:21:28.631Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-16T12:21:28.631Z
---

# 外部アクセス・MCP・Runner の境界

現行製品は local self-host の Board と Workspace を使う。旧 remote-server・PostgreSQL / Electric・Cloud AuthSession・SPAKE2 pairing・relay署名・WebRTC fallback・remote-web は撤去された。旧 token の寿命や paired-host refresh を現行の接続契約として適用しない。[現行構成](../../Cargo.toml)、[server router](../../crates/server/src/routes/mod.rs)。認証付き外部 MCP と execution bridge は別の機能として残る。

## UI の直接アクセス

通常 UI / API は信頼された単一利用者を前提とし、公開時は HTTPS と Basic 認証などの運用境界を設ける。[運用前提](../../README.md)。Origin guard は Origin がなければ許可し、ある場合は same-origin / allowlist を照合する。これは利用者認証ではなく、旧 relay 例外もない。[Origin](../../crates/server/src/middleware/origin.rs)。

local-web は `/api` を backend へ送り、WebSocket も同じ経路を使う。旧 remote host を明示した要求は送信前に拒否されるため、残った host ID から別ホストへの暗黙 fallback を期待しない。[Web transport](../architecture/web-and-sync.md)。Vite の `.ts.net` allowlist と Host を保持する proxy は残るが、tailnet 自体の認証・配備成功をこの設定だけで証明しない。[proxy](../../packages/local-web/vite.config.ts#L135-L151)。

## 認証付き外部 MCP

`LVK_MCP_CONFIG` がある場合だけ Streamable HTTP `/mcp` を提供する。未設定なら入口なし、不正設定は起動時エラー。metadata は `/.well-known/oauth-protected-resource/mcp`。stateful session を持たない JSON 応答の transport で、設定 public URL の Host / Origin を制限する。[router](../../crates/server/src/remote_mcp/mod.rs#L42-L85)。これは [stdio MCP](agent-providers.md#外部-mcp-client--lvk)や [Astra](../concepts/project-chat.md)のモデル会話と独立し、自動監視・通知や model session を作らない。

operator 設定は HTTPS の public base URL・issuer・introspection endpoint、client ID、secret を読む環境変数名、単一 owner subject、1〜500 Project の明示 allowlist を要求する。token を tool caller の申告だけで信用せず、認可 server の RFC 7662 introspection に照会する。[設定](../../crates/server/src/remote_mcp/config.rs)。

有効条件は active、bearer、owner subject と issuer の一致、resource audience、必須 exp が未来、任意 nbf が現在以前である。署名と失効の検証は認可 server、resource / owner / scope の照合は LVK が担う。認証失敗401、read不足403、introspection通信・非成功・不正応答は503。照会は redirect禁止・5秒・32KiB上限で、provider本文やtokenをログに出さず、受理後 Authorization header を downstream から除く。[認証](../../crates/server/src/remote_mcp/auth.rs)。

全操作に `lvk:read`、Issue作成更新・Workspace開始・steer/continue・仕様回答に `lvk:write`、正式 Integration 開始に `lvk:merge` を要求する。scope不足は MCP error と再接続 challenge で返す。Project allowlistと実際のIssue/Workspace/Session関係も照合するので、UUIDを知るだけでは別Projectへ越境できない。[scope dispatch](../../crates/server/src/remote_mcp/mod.rs#L190-L221)、[所属とrevisionのテスト](../../crates/server/src/remote_mcp/tests.rs)。

## 参照結果と操作 receipt

返すコード・diff・Wiki・Memoryは参照データであり、命令や承認ではない。参照はページ上限とrevisionを持ち、コードは許可されたregular fileに限定し、private path・symlink・submodule等を除外する。binary / oversized / unavailableを空の成功diffとして扱わない。[code read](../../crates/server/src/remote_mcp/code_read.rs)。実行状態・検証・公開・Wiki receiptは別の証拠として返す。[Integration read](../../crates/server/src/remote_mcp/integration_read.rs)、[verification read](../../crates/server/src/remote_mcp/verification_read.rs)。

mutation は principal + request ID と入力hashを固定した永続 receipt を作る。同じkeyで異なる入力は拒否し、再送は保存済み結果を参照する。MCP client切断は受理済み操作を取り消さず、domain受付を待つ経路もcoding完了を待たない。server再起動後の旧accepted/runningはuncertainとして見せ、自動replayしない。結果不明時に新request IDで同じ副作用を重ねず、既存receiptと対象状態を照合する。[操作寿命](../../crates/server/src/remote_mcp/operations.rs)、[lost responseのテスト](../../crates/server/src/remote_mcp/runtime_write_tests.rs#L479-L502)。

このcheckoutは上記introspection方式である。Auth0 discovery / JWKS / owner bootstrap等を、未統合の設計やWorkspace Memoryだけから現行機能へ昇格させない。

## 外部実行とモバイル設計

Windows Runnerはexecution slot等のcanonical Resourceをclaimし、immutable source snapshotとinteractive commandを使う。接続認証・containment・settlementの責任はMCP bearerや旧paired hostと異なる。[Runnerの所有・回復](../concepts/shared-resource-coordination.md#外部-runner-と-interactive-operation)。

[AI Mobile Design](../../docs/ai-mobile/design.md)はv0.1 draftである。web/PWA中心という意図は読めるが、記載されたremote-web / relay baselineやQR pairing案は現行実装の証拠にならない。現在のスマートフォンからの入口は直接local-webであり、UI確認には [built preview](../operations/development.md)を使う。外部IdP、HTTPS経路、Windows hardwareの実接続はこの静的source照合では試験していない。
