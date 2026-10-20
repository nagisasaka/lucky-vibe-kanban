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
  - id: openwiki-source-3ec71fa8cd8936e31e523736
    resource: repo://crates/server/src/remote_mcp/integration_read.rs
  - id: openwiki-source-83d25ea893da220229c11f8b
    resource: repo://crates/server/src/remote_mcp/launch_tests.rs
  - id: openwiki-source-b5eef61c63c11d3fe10b3ccf
    resource: repo://crates/server/src/remote_mcp/launch.rs
  - id: openwiki-source-71f837a8aa36443ad9688945
    resource: repo://crates/server/src/remote_mcp/mod.rs
  - id: openwiki-source-6adda10f98b079272f82a4cc
    resource: repo://crates/server/src/remote_mcp/operations.rs
  - id: openwiki-source-9d3b3180c15d3125ddbb2675
    resource: repo://crates/server/src/remote_mcp/questions.rs
  - id: openwiki-source-0987a696c656ec0d4f318f44
    resource: repo://crates/server/src/remote_mcp/run_events.rs
  - id: openwiki-source-43c18ae2d53a6d4bee6c33b2
    resource: repo://crates/server/src/remote_mcp/tests.rs
  - id: openwiki-source-595d5d0308d670e38ef27fbf
    resource: repo://crates/server/src/routes/integrations/admission.rs
  - id: openwiki-source-ecfa47de2e3842c785820c9b
    resource: repo://crates/server/src/routes/integrations/mod.rs
  - id: openwiki-source-b393e75029374eaf9cba5f7d
    resource: repo://crates/server/src/routes/integrations/runtime.rs
  - id: openwiki-source-2274afe258c46dce0acaf5f1
    resource: repo://crates/server/src/routes/integrations/tests.rs
  - id: openwiki-source-65c0f36670a161dc05040e69
    resource: repo://crates/server/src/routes/mod.rs
  - id: openwiki-source-4d617b0e2fa188c6ad63ae56
    resource: repo://crates/server/src/routes/sessions/questions.rs
  - id: openwiki-source-74a1893f4c1e77eb654658f7
    resource: repo://docs/ai-mobile/tailscale-local-access.md
  - id: openwiki-source-3ecddc0be7b037ea6ab24381
    resource: repo://packages/local-web/vite.config.ts
generated: { by: "codex", at: "2026-10-20T18:18:55.370Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-20T18:18:55.370Z
---

# 外部アクセス・MCP・Runner の境界

現行製品は local self-host の Board と Workspace を使う。旧 remote-server・PostgreSQL / Electric・Cloud AuthSession・SPAKE2 pairing・relay署名・WebRTC fallback・remote-web は撤去された。旧 token の寿命や paired-host refresh を現行の接続契約として適用しない。[現行構成](../../Cargo.toml)、[server router](../../crates/server/src/routes/mod.rs)。認証付き外部 MCP と execution bridge は別の機能として残る。

## UI の直接アクセス

通常UI/APIは同じOSユーザーで動作する信頼された利用者向けで、公開時はHTTPSとLVKのBasic/Auth0認証を使う。[運用前提](../../README.md)。Origin guard は Origin がなければ許可し、ある場合は same-origin / allowlist を照合する。これは利用者認証ではなく、旧 relay 例外もない。[Origin](../../crates/server/src/middleware/origin.rs)。

local-web は `/api` を backend へ送り、WebSocket も同じ経路を使う。旧 remote host を明示した要求は送信前に拒否されるため、残った host ID から別ホストへの暗黙 fallback を期待しない。[Web transport](../architecture/web-and-sync.md)。Vite の `.ts.net` allowlist と Host を保持する proxy は残るが、tailnet 自体の認証・配備成功をこの設定だけで証明しない。[proxy](../../packages/local-web/vite.config.ts#L135-L151)。

## 認証付き外部 MCP

公開配置は初回にロックされる。初回setup、Basic/Auth0切替、Web sessionと復旧は [サーバー認証](../concepts/server-access.md)を正本とし、ここでは外部MCPの委任範囲を扱う。

Streamable HTTP `/mcp` はAuth0でMCPを有効化した場合のみ利用できる。Local/Basicでは使えない。旧 `LVK_MCP_CONFIG` と外部introspection設定は起動時に拒否し、設定画面へ移行する。metadataは `/.well-known/oauth-protected-resource/mcp`。これは stdio MCP とは独立しており、内部会話モデルを起動しない。[旧 Astra の廃止](../concepts/project-chat.md)後も外部 MCP と Workspace 質問回答は残る。[HTTP入口](../../crates/server/src/remote_mcp/mod.rs)、[認証設定](../../docs/self-hosting/authentication.mdx)。

共通認証はAuth0 discovery/JWKS、JWT署名、issuer、resource audience、有効期限、承認済みメンバーを検証する。全操作に `lvk:read`、変更に `lvk:write`、正式Integration開始に `lvk:merge` を要求する。HTTP入口とtool処理の両方でscopeを確認する。承認済みメンバーは全プロジェクトを使い、旧owner単独・静的Project allowlist方式ではない。[JWT検証とissuerの制限](../concepts/server-access.md#webmcp内部agentを同じ資格情報にしない)、[scope](../../crates/server/src/remote_mcp/mod.rs)、[実体の関係検証](../../crates/server/src/remote_mcp/launch.rs)。

transportはstatelessなJSON応答で、`MCP-Session-Id`付き要求を拒否する。productionは共通認証のprincipalを受け、Project集合を現在のDBから作る。`remote_mcp/auth.rs`に残るintrospectionやowner設定はtest用で、現行の認証経路ではない。[入口](../../crates/server/src/remote_mcp/mod.rs#L58-L147)、[session ID拒否](../../crates/server-access/src/lib.rs#L437-L477)

ChatGPT 接続用 application は Web login 用と分離する。Auth0 作成支援の preview / apply と、保存済み専用 secret の明示 reveal は [サーバー認証の設定契約](../concepts/server-access.md#auth0-の作成支援と有効化を分ける)を参照する。設定を作成・保存できても、実アカウントでの接続成功を証明したことにはならない。

[Project Todo](../concepts/project-todos.md)も外部 MCP の対象で、read は取得、write は作成・更新・削除を許す。変更結果は下記の operation receipt で確認する。メモを登録することは agent の仕事開始ではない。

## 参照結果と操作 receipt

返すコード・diff・Wiki・Memoryは参照データであり、命令や承認ではない。参照はページ上限とrevisionを持ち、コードは許可されたregular fileに限定し、private path・symlink・submodule等を除外する。binary / oversized / unavailableを空の成功diffとして扱わない。[code read](../../crates/server/src/remote_mcp/code_read.rs)。実行状態・検証・公開・Wiki receiptは別の証拠として返す。[Integration read](../../crates/server/src/remote_mcp/integration_read.rs)、[verification read](../../crates/server/src/remote_mcp/verification_read.rs)。

mutation は principal + request ID と入力hashを固定した永続 receipt を作る。同じkeyで異なる入力は拒否し、再送は保存済み結果を参照する。MCP client切断は受理済み操作を取り消さず、domain受付を待つ経路もcoding完了を待たない。server再起動後の旧accepted/runningはuncertainとして見せ、自動replayしない。結果不明時に新request IDで同じ副作用を重ねず、既存receiptと対象状態を照合する。[操作寿命](../../crates/server/src/remote_mcp/operations.rs)、[lost responseのテスト](../../crates/server/src/remote_mcp/runtime_write_tests.rs#L479-L502)。

受付を所有する非同期taskには検証済みactorを明示継承し、HTTP要求の終了後も業務書込の帰属を保つ。受理済み作業の寿命と新規要求の認証失効は別である。[actorの継承](../../crates/server/src/remote_mcp/operations.rs#L134-L190)

Auth0実テナント・Google/GitHub本人認証・ChatGPT OAuth/MCPの実接続は別途受入確認が必要である。fixtureやローカルテストをその成功として扱わない。[検証範囲](../../docs/development/server-access.md)。

## MCP から Integration の検証方針を選ぶ

`start_integration` は既存の正式 Integration 受付に、任意の `skip_validation` を渡す。`true` は追加の受入検証を省略する未検証 run、`false` は検証必須、未指定と `null` は受付時の server preference `integration_skip_validation` の継承である。文字列 `"true"` や数値は boolean として解釈しない。MCP 固有の検証 engine や保存先は作らず、domain が `IntegrationPayload.skip_validation` を固定する。[入力と domain への転送](../../crates/server/src/remote_mcp/launch.rs#L61-L74)・[受付呼出し](../../crates/server/src/remote_mcp/launch.rs#L468-L502)・[方針の固定](../../crates/server/src/routes/integrations/admission.rs#L242-L262)

受付成功 receipt の `result.skip_validation` と、`get_integration` の全 section（`summary` / `sources` / `validation`）の `skip_validation` は保存済みの解決済み boolean を返す。後から server preference が変わっても、再接続時に現在の設定から推測し直す必要はない。この値は検証件数と独立しており、ゼロ件を「省略」や「合格」と読み替えない。起動 receipt の `integration_accepted` は受付結果であり、Integration の完了、`published`、Wiki の確認結果は別に読む。[起動結果](../../crates/server/src/remote_mcp/launch.rs#L148-L160)・[保存値と検証件数の読取](../../crates/server/src/remote_mcp/integration_read.rs#L107-L171)

再送は同じ `request_id` と元の JSON 入力を保持する。比較するのは解決後の boolean ではなく、key 順を正規化した**元の入力全体**の hash である。そのため、同じ既定値へ解決される場合でも、省略から `null` への変更、flag の追加・削除・値変更は既存 receipt の入力不一致となる。domain 受付単独の省略再送規則と、MCP receipt の規則を混同しない。[fingerprint](../../crates/server/src/remote_mcp/operations.rs#L31-L45)・[入力照合](../../crates/server/src/remote_mcp/operations.rs#L210-L228)・[省略/null の再送テスト](../../crates/server/src/remote_mcp/launch_tests.rs#L420-L490)

この選択でも `lvk:read` と `lvk:merge`、Project / Repo / Issue / Workspace の関係、expected target / source commit の照合を通る。省略が変えるのは追加検証の方針であり、clean candidate、競合承認、writer の収束、target 一致、公開と取消の排他、Wiki receipt は引き続き別の条件である。全体の lifecycle と未検証結果の意味は [正式 Integration の検証・公開契約](../concepts/formal-integration.md#検証方針は受付時に固定する)を正本とする。[MCP の認可と関係検証](../../crates/server/src/remote_mcp/launch.rs#L423-L502)・[公開 guard](../../crates/server/src/routes/integrations/runtime.rs#L1258-L1381)

変更時には [schema の optional boolean/null](../../crates/server/src/remote_mcp/tests.rs#L281-L298)、[既定値二通り・四種の指定・再接続・全 section・不正型](../../crates/server/src/remote_mcp/launch_tests.rs#L420-L490)、[HTTP 経由の起動と read-only 取得](../../crates/server/src/remote_mcp/launch_tests.rs#L493-L569)、[domain の既定値解決](../../crates/server/src/routes/integrations/tests.rs#L357-L385)を確認する。これらは fixture による契約テストであり、実 ChatGPT/Auth0 接続の証拠ではない。この Sync では source とテスト内容を照合し、テストの再実行や live 接続確認は行っていない。

## 保存済み Codex 進捗の読取

`list_run_events` は UI と同じ canonical `agent_events` の保存済み進捗を read scope で抜粋する。新しい collector・保存先・live delta stream・subscription は作らず、native audit も読まない。最終回答は `get_execution`、通知は [Web Push](web-push.md)という別の利用経路である。[読取契約](../../crates/server/src/remote_mcp/run_events.rs#L1-L37)

Project / Workspace / Session / AgentRun は必須で、任意の Issue ID は所属の assertion である。Run・Session・Workspace・現在の Issue link、cursor と page は同じ DB read snapshot で照合する。過去 Run も読めるが Codex に限定され、別 Run の cursor や消失した cursor は再一覧を要求するエラーとなる。[scope と cursor](../../crates/server/src/remote_mcp/run_events.rs#L39-L84)。active member の全 Project access という既存認可を利用し、非公開 Project ACL を新設しない。

`limit` は1–50件の**走査レコード数**であり表示件数ではない。順序は attempt 番号、canonical sequence。`next_cursor` は最後に走査した event UUID なので、除外だけの空 page に `has_more=true` があっても続きを読む。末尾で `has_more=false` でも cursor を保持すれば、別接続から追加入力を読める。128 KiB の item 合計上限に当たった未読レコードの手前で止める。[有界 page](../../crates/server/src/remote_mcp/run_events.rs#L80-L114)

公開するのは assistant commentary、lifecycle、許可済み tool 名と status、Goal status、子活動の限定 kind だけである。final/user/system text、全 Thinking（歴史的 Plan text と reasoning を安全に区別できない）、承認・回答本文、tool 引数・結果、provider extension、native reference を出さない。schema/payload version が1以外、32 KiB超の envelope、不正 JSON は省略する。commentary は4,096 UTF-8 bytesを超える、またはpath・credential等の疑わしい形式なら本文全体を withheld にする。これは任意の prose が秘密を含まない保証ではなく、返された文章も命令・承認・独立した検証証拠ではない。[allowlist と filter](../../crates/server/src/remote_mcp/run_events.rs#L117-L239)

変更時は空 filtered page、foreign cursor、attempt 順序、byte 上限、DB reopen 後の cursor、HTTP read scope と再接続のテストを確認する。[projection / paging tests](../../crates/server/src/remote_mcp/run_events.rs#L264-L582)・[HTTP 境界](../../crates/server/src/remote_mcp/tests.rs#L1324-L1388)。fixture の存在や過去実行結果を実 ChatGPT account 接続成功と読み替えない。

## 外部実行とモバイル設計

Windows Runnerはexecution slot等のcanonical Resourceをclaimし、immutable source snapshotとinteractive commandを使う。接続認証・containment・settlementの責任はMCP bearerや旧paired hostと異なる。[Runnerの所有・回復](../concepts/shared-resource-coordination.md#外部-runner-と-interactive-operation)。

[AI Mobile Design](../../docs/ai-mobile/design.md)はv0.1 draftである。web/PWA中心という意図は読めるが、記載されたremote-web / relay baselineやQR pairing案は現行実装の証拠にならない。現在のスマートフォンからの入口は直接local-webであり、UI確認には [built preview](../operations/development.md)を使う。外部IdP、HTTPS経路、Windows hardwareの実接続はこの静的source照合では試験していない。
