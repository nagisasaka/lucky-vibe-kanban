---
type: architecture
title: Web の実行環境と同期
description: Local shell と共有画面の責任、ローカル snapshot と AgentRun stream の異なる同期契約。
tags: [frontend, synchronization, electric, transport]
sources:
  - id: openwiki-source-2f63ca1582805a46da98d522
    resource: repo://crates/db/migrations/20261018180000_web_push.sql
  - id: openwiki-source-d20a82e2192a07c687b838cb
    resource: repo://crates/db/src/models/agent_runtime.rs
  - id: openwiki-source-788fa2f6f1a3d449a1a9efe4
    resource: repo://crates/local-deployment/src/agent_run_port.rs
  - id: openwiki-source-162bcfe1ee47c1047ccf7d52
    resource: repo://crates/server/src/routes/agent_runs.rs
  - id: openwiki-source-4f90d37fb90b7f5198b7b293
    resource: repo://crates/server/src/routes/frontend.rs
  - id: openwiki-source-42c7cffe07ec2d2c0ed4cc9d
    resource: repo://crates/server/src/routes/workspaces/workspace_summary.rs
  - id: openwiki-source-7151ac14cb4570c5c04781dd
    resource: repo://crates/services/src/services/agent_runtime.rs
  - id: openwiki-source-10897c55284ee23ee2d4e388
    resource: repo://packages/local-web/src/app/auth/pushReturn.ts
  - id: openwiki-source-a4deecf361e70bd3b2703d13
    resource: repo://packages/local-web/src/app/auth/serverAccess.ts
  - id: openwiki-source-5d3aad6c180293a86853f6ec
    resource: repo://packages/local-web/src/app/auth/ServerAccessGate.tsx
  - id: openwiki-source-a623a548ee805a854d7d5e18
    resource: repo://packages/local-web/src/app/entry/App.tsx
  - id: openwiki-source-f5c8f8cc45fdf36d98b214ce
    resource: repo://packages/local-web/src/app/entry/Bootstrap.tsx
  - id: openwiki-source-2bc26d0169842f4566548104
    resource: repo://packages/local-web/src/app/providers/ConfigProvider.tsx
  - id: openwiki-source-5345acc0f5e557e263435988
    resource: repo://packages/ui/src/components/AppBar.tsx
  - id: openwiki-source-b2fc2a264ac3c8fd4fc0ac15
    resource: repo://packages/ui/src/components/AppBarSocialLink.tsx
  - id: openwiki-source-0538c702500041c84ca85af7
    resource: repo://packages/ui/src/components/ChatAggregatedToolEntries.tsx
  - id: openwiki-source-9fa3c3bb81a61c7549d33839
    resource: repo://packages/ui/src/components/ChatCode.tsx
  - id: openwiki-source-7178b2bafadec69965c4e1cd
    resource: repo://packages/ui/src/components/ChatScriptEntry.tsx
  - id: openwiki-source-d89c4480040eee7fa41a6000
    resource: repo://packages/ui/src/components/CodeHighlightPlugin.tsx
  - id: openwiki-source-5e4c1802d2c1ee81cad7ac45
    resource: repo://packages/ui/src/components/IssueWorkspaceCard.tsx
  - id: openwiki-source-bb23b715172e6051c10e1afc
    resource: repo://packages/ui/src/components/ReadOnlyLinkPlugin.tsx
  - id: openwiki-source-1383d6dc4e3d9d2ea475e735
    resource: repo://packages/ui/src/lib/chat-code-highlighting.ts
  - id: openwiki-source-2a1a06abf88738697abb7c30
    resource: repo://packages/ui/src/lib/repository.ts
  - id: openwiki-source-397291e029ec7b98f2f46af2
    resource: repo://packages/web-core/src/features/agent-runtime/model/AgentRunSessionContext.tsx
  - id: openwiki-source-a46d1ceab41257b518aa5a4a
    resource: repo://packages/web-core/src/features/agent-runtime/model/canonicalAgentConversation.test.ts
  - id: openwiki-source-6f68befb805cadf03e4b569b
    resource: repo://packages/web-core/src/features/agent-runtime/model/canonicalAgentConversation.ts
  - id: openwiki-source-fe9d810dccb0b777dfcb7ae4
    resource: repo://packages/web-core/src/features/agent-runtime/model/canonicalAgentTimeline.ts
  - id: openwiki-source-f6872c1e7e07221eda6db545
    resource: repo://packages/web-core/src/features/onboarding/ui/LandingPage.tsx
  - id: openwiki-source-71d9a385f4b03b8ca71765b0
    resource: repo://packages/web-core/src/features/wiki/model/wikiViewerController.ts
  - id: openwiki-source-202a77c218ffbc8ac2871a04
    resource: repo://packages/web-core/src/features/wiki/ui/WorkspaceWikiProvider.tsx
  - id: openwiki-source-b58899ddf6c6173903fabe61
    resource: repo://packages/web-core/src/features/workspace-chat/model/contexts/ApprovalFeedbackContext.tsx
  - id: openwiki-source-3e72be92e5674f5611856ac3
    resource: repo://packages/web-core/src/features/workspace-chat/model/conversation-scroll-commands.ts
  - id: openwiki-source-952ab4470659b6dc518fab38
    resource: repo://packages/web-core/src/features/workspace-chat/model/useConversationVirtualizer.ts
  - id: openwiki-source-d32b3d54fb9362eee85118ce
    resource: repo://packages/web-core/src/features/workspace-chat/ui/AppChatMarkdown.tsx
  - id: openwiki-source-cd106619cfd7af68eb9464fc
    resource: repo://packages/web-core/src/features/workspace-chat/ui/ConversationListContainer.tsx
  - id: openwiki-source-a045ed78b54878edd07211c7
    resource: repo://packages/web-core/src/features/workspace-chat/ui/ExecutionInspectionPanel.tsx
  - id: openwiki-source-b20f21ce6e1fdbde2fcde6ed
    resource: repo://packages/web-core/src/features/workspace-files/model/useChatFileLink.ts
  - id: openwiki-source-89a716f39968e2030cf296c0
    resource: repo://packages/web-core/src/pages/root/RootRedirectPage.tsx
  - id: openwiki-source-2fb74c0fc81d817149606557
    resource: repo://packages/web-core/src/pages/workspaces/PreservedChatPanel.tsx
  - id: openwiki-source-27129bb783a0bcb3964399f2
    resource: repo://packages/web-core/src/pages/workspaces/WorkspacesLanding.tsx
  - id: openwiki-source-0894249b732085a9800940bd
    resource: repo://packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx
  - id: openwiki-source-8e27f9b63333a62bc1b246ff
    resource: repo://packages/web-core/src/shared/actions/index.ts
  - id: openwiki-source-caba6442d1d02b97bf406c6a
    resource: repo://packages/web-core/src/shared/command-bar/actions/pages.ts
  - id: openwiki-source-649882ab70d5bba1d79459a5
    resource: repo://packages/web-core/src/shared/components/CreateChatBoxContainer.tsx
  - id: openwiki-source-a7cac6e6f3abafbcf9390ddf
    resource: repo://packages/web-core/src/shared/components/RouteLoadState.tsx
  - id: openwiki-source-986a5914454c708e7540f1e6
    resource: repo://packages/web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx
  - id: openwiki-source-4f5acbe99dd0d72515b7e82b
    resource: repo://packages/web-core/src/shared/components/ui-new/containers/SharedAppLayout.tsx
  - id: openwiki-source-10caf2340da819f813f5ea62
    resource: repo://packages/web-core/src/shared/components/WYSIWYGEditor.tsx
  - id: openwiki-source-a1a870cf32aedb317f9ea946
    resource: repo://packages/web-core/src/shared/dialogs/settings/settings/SettingsHostContext.tsx
  - id: openwiki-source-5cead200f725b8a7ed11e3cd
    resource: repo://packages/web-core/src/shared/hooks/useGitHubStars.ts
  - id: openwiki-source-70a9c5225d8ef0472482d401
    resource: repo://packages/web-core/src/shared/hooks/useJsonPatchWsStream.ts
  - id: openwiki-source-43d7a2f6bd06c556e7d48e3c
    resource: repo://packages/web-core/src/shared/hooks/useScratch.ts
  - id: openwiki-source-b885bc17dfe85c4ca45223e9
    resource: repo://packages/web-core/src/shared/hooks/useUiPreferencesScratch.ts
  - id: openwiki-source-ffa337e3881da0103a10e223
    resource: repo://packages/web-core/src/shared/hooks/useVisualViewportHeightVar.ts
  - id: openwiki-source-012d640135a03bd20065dc93
    resource: repo://packages/web-core/src/shared/hooks/useWorkspaceOwner.ts
  - id: openwiki-source-97cd8794805cef30e1c2f047
    resource: repo://packages/web-core/src/shared/hooks/useWorkspaces.ts
  - id: openwiki-source-8860c99d38e029d2bcc112c2
    resource: repo://packages/web-core/src/shared/hooks/useWorkspaceSessions.ts
  - id: openwiki-source-29a47cc7d1a22c6d0ddf4b6c
    resource: repo://packages/web-core/src/shared/hooks/workspaceSessionSelection.test.ts
  - id: openwiki-source-cf7deb2cd54542f4c2fd7515
    resource: repo://packages/web-core/src/shared/hooks/workspaceSessionSelection.ts
  - id: openwiki-source-e665e0fe8e478ec542e0ace4
    resource: repo://packages/web-core/src/shared/lib/electric/collections.ts
  - id: openwiki-source-9ca1dca04d93c4fed8664d25
    resource: repo://packages/web-core/src/shared/lib/localApiTransport.ts
  - id: openwiki-source-d4726fdd33da0f7c2f1ae1ee
    resource: repo://packages/web-core/src/shared/lib/remoteApi.ts
  - id: openwiki-source-3b6f5885b9d50925c82267dd
    resource: repo://packages/web-core/src/shared/providers/WorkspaceProvider.tsx
  - id: openwiki-source-3744cbc03f96140414e0b945
    resource: repo://tests/chat/conversation-layout.test.mjs
  - id: openwiki-source-adbcc4477d532869e93682ad
    resource: repo://tests/chat/isolated.spec.ts
  - id: openwiki-source-0a43f232c5a84b7f94378d02
    resource: repo://tests/chat/syntax-highlight.test.mjs
  - id: openwiki-source-7c49efe20c19a1848e432191
    resource: repo://tests/chat/syntax-isolated-e2e.mjs
  - id: openwiki-source-b8dd26804205bb3bb293d01e
    resource: repo://tests/header/isolated.mjs
  - id: openwiki-source-21c2ff0d1558af038f1aaa2e
    resource: repo://tests/header/README.md
  - id: openwiki-source-823cc976bdb2bdcb610e746f
    resource: repo://tests/integration-skip/history.spec.ts
  - id: openwiki-source-4bfe8064f685e5073ef06513
    resource: repo://tests/workspace-navigation/playwright.config.ts
  - id: openwiki-source-f6a73bd9a093b3a0004194f2
    resource: repo://tests/workspace-navigation/run-isolated.py
  - id: openwiki-source-b87e3253bd1458a0091f8587
    resource: repo://tests/workspace-navigation/workspace-navigation.spec.ts
generated: { by: "codex", at: "2026-10-21T05:39:12.277Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-21T05:39:12.277Z
---

# Web の実行環境と同期

## 画面と実行環境を分ける

Local App は TanStack Router と Hotkeys、設定、ローカル identity、Tauri listener を組み立てる。共有画面と hooks は web-core にある。Remote Web shell は削除済みである。[Local App](../../packages/local-web/src/app/entry/App.tsx)

UI package は既に Kanban や chat、preview などの再利用部品を持つ。[Kanban の例](../../packages/ui/src/components/KanbanBoard.tsx#L1-L58)。`packages/ui/README.md` の「initial scaffold」と [抽出前の監査](../../docs/frontend-ui-library-refactor-audit.md)は現行配置の全体図ではない。監査は責任分割の経緯を知る資料として読み、古い `local-web/src/components` の位置を変更先にしない。

## 共有ナビゲーションと外部リポジトリ表示

`SharedAppLayout` は Project 一覧、Workspace への遷移、モバイル drawer と外部 GitHub 件数の取得を接続し、UI package の `AppBar` は渡された値と callback を描画する。単独の `Local` 見出し、Project navbar の Organization 名、モバイル drawer の Organization 名は表示しない。ただし Project 一覧は引き続き選択 Organization ID で取得し、Workspace 入口と Project の選択・作成を維持する。Workspace navbar はパス、または `direct-folder` 以外のブランチ名を表示する。[一覧の scope](../../packages/web-core/src/shared/components/ui-new/containers/SharedAppLayout.tsx#L87-L127)・[drawer の遷移](../../packages/web-core/src/shared/components/ui-new/containers/SharedAppLayout.tsx#L337-L412)・[Workspace 見出し](../../packages/web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx#L243-L260)・[AppBar の入口](../../packages/ui/src/components/AppBar.tsx#L148-L180)。見出しの整理は [Organization の identity・権限](../concepts/organization-and-membership.md)や [Workspace の実行単位](../concepts/workspace.md)の廃止を意味しない。

外部リンクと stars API は `packages/ui/src/lib/repository.ts` の公式リポジトリ `nagisasaka/lucky-vibe-kanban` を共通参照する。AppBar と LandingPage の GitHub リンク、web-core の `useGitHubStars` の API URL と query key が同じ identity を使う。定数のコメントは README / README.ja.md と origin を選定根拠として記録し、両 README の source link も一致する。[共通定義](../../packages/ui/src/lib/repository.ts#L1-L3)・[AppBar](../../packages/ui/src/components/AppBar.tsx#L321-L337)・[LandingPage](../../packages/web-core/src/features/onboarding/ui/LandingPage.tsx#L88-L101)・[英語 README](../../README.md)・[日本語 README](../../README.ja.md)

stars はブラウザーから GitHub API を直接 `cache: 'no-store'` で読み、非負の safe integer だけを採用する。HTTP エラー、通信・JSON 解析失敗、欠落・不正値は `null` にする。初回 loading や `null` ではバッジを出さず、実数 `0` は表示する。リンク自体は件数取得に依存しない。query はリポジトリ単位で、stale time と定期再取得は10分、retry と mount 時の再取得は無効である。[取得・query lifecycle](../../packages/web-core/src/shared/hooks/useGitHubStars.ts#L1-L41)・[バッジの条件](../../packages/ui/src/components/AppBar.tsx#L325-L336)・[リンクとバッジの描画](../../packages/ui/src/components/AppBarSocialLink.tsx#L18-L46)。この表示用 API は Board の snapshot 同期や [GitHub 認証・PR 操作](../integrations/github-review.md)とは別の経路である。

旧 `Give Feedback` survey action は navbar と command bar の General に存在せず、AppBar と LandingPage の Discord リンクもない。共有 shell は Discord 人数 hook を呼ばない。これは外部サービスへの UI 導線の整理であり、Agent approval の feedback を除去する契約ではない。[navbar actions](../../packages/web-core/src/shared/actions/index.ts#L1555-L1569)・[command bar](../../packages/web-core/src/shared/command-bar/actions/pages.ts#L35-L44)・[AppBar の外部領域](../../packages/ui/src/components/AppBar.tsx#L321-L337)・[shell の取得](../../packages/web-core/src/shared/components/ui-new/containers/SharedAppLayout.tsx#L60-L69)・[approval feedback](../../packages/web-core/src/features/workspace-chat/model/contexts/ApprovalFeedbackContext.tsx#L41-L101)

### ヘッダー検証の入口と限界

`tests/header/isolated.mjs` は checkout の build 済み server / frontend と専用の一時 instance を使い、実 API で Project と独立 Workspace を作る。GitHub API だけを mock し、loading → 0、正数、HTTP 失敗、負数、欠落、通信失敗と、desktop / 390px drawer の往復遷移、旧導線の不在を検査する。終了時は browser を閉じ、所有 server process group を停止して待ち、一時 instance を削除する。[隔離と API 操作](../../tests/header/isolated.mjs#L11-L120)・[stars と desktop](../../tests/header/isolated.mjs#L128-L241)・[mobile](../../tests/header/isolated.mjs#L242-L276)・[cleanup](../../tests/header/isolated.mjs#L321-L341)。再実行は [header 試験手順](../../tests/header/README.md)と [開発環境](../operations/development.md)を参照する。mock の成功から live GitHub の到達性や件数を保証しない。

local shell の ConfigProvider は onboarding を acknowledged として公開し、LandingPage はその値を見て root へ戻る。このため隔離 browser 試験が検査するのは onboarding bypass であり、LandingPage 内の social link の操作ではない。後者はソース照合の範囲である。[config の上書き](../../packages/local-web/src/app/providers/ConfigProvider.tsx#L49-L59)・[redirect](../../packages/web-core/src/features/onboarding/ui/LandingPage.tsx#L172-L179)・[bypass 試験](../../tests/header/isolated.mjs#L277-L300)。[開発時の検証記録](../../tests/header/VALIDATION.md)と、統合後の検証成功は区別する。今回のヘッダー Sync は凍結 source `db5fe7976ca152829ad46d498342a8f3fe889b27` の実装・呼出側・試験定義を静的に照合した。供給された正式 Integration は検証省略であり、今回も製品テストや live GitHub 確認は実施していない。無関係な既存節の再検証を意味しない。

## Workspace の一覧遷移と作成要求

`/workspaces` の `WorkspacesLanding` は `Workspaces` → `WorkspacesLayout` を描画する。通常の Workspace 入口は一覧を開き、作成 route への転送や最後の Workspace の自動選択を行わない。一方、`/` の `RootRedirectPage` は別の入口で、Project を解決できない場合などに作成 route へ進む。両者の挙動を一括して変更しない。[一覧入口](../../packages/web-core/src/pages/workspaces/WorkspacesLanding.tsx#L1-L5)・[root の分岐](../../packages/web-core/src/pages/root/RootRedirectPage.tsx#L18-L50)

sidebar の「＋」は `navigateToCreate` を呼ぶ。この処理は route 遷移に加えて provider の `workspaceTargetRequest` を増やすため、既に create route にいる場合も新しい選択要求を伝えられる。layout の desktop / mobile 両方が同じ値を composer へ渡す。composer は draft と既定値の準備を待ち、最後に扱った request と異なる場合に chooser を開く。dialog-open ref は重複表示を防ぐ。これは作成回数や Workspace ID ではなく UI 内の再選択要求である。[要求の発行](../../packages/web-core/src/shared/providers/WorkspaceProvider.tsx#L244-L251)・[mobile](../../packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx#L338-L350)・[desktop](../../packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx#L522-L531)・[受信](../../packages/web-core/src/shared/components/CreateChatBoxContainer.tsx#L284-L300)

chooser の取消は選択値を適用せず戻り、再表示のために composer を作り直したり message を消去したりしない。確定した作成先を初期値として再び渡す。作成先確認と directory 参照の詳細は [Workspace の作成先選択](../concepts/workspace.md#一覧から作成先を選ぶ)を正本とし、永続 draft の scope は [Scratch の契約](../concepts/session-and-agent-run.md#未送信ドラフトと-scratch)と区別する。[結果の適用](../../packages/web-core/src/shared/components/CreateChatBoxContainer.tsx#L210-L250)

### Workspace 導線の検証範囲

`WorkspacesLanding.test.tsx` は作成転送用 navigation hook を使わず一覧を描画することを確認する。`tests/workspace-navigation/workspace-navigation.spec.ts` は実 API で設定と空 Workspace を作り、一覧への通常遷移、＋の再選択、取消、確定パスの復元、存在しないパスのエラー、server directory の選択、日本語・dark・390px の表示を検査する。Agent 起動・実 worktree 作成・message draft の保存復元をこの E2E が証明するわけではない。[unit test](../../packages/web-core/src/pages/workspaces/WorkspacesLanding.test.tsx#L1-L21)・[browser 試験](../../tests/workspace-navigation/workspace-navigation.spec.ts#L1-L207)

`run-isolated.py` は checkout の build 済み server / agent-process-host と frontend を前提に、一時 instance と loopback で試験を実行する。終了時は試験 process group と残存 browser、port、一時 instance を確認する。Playwright 設定は loopback URL と既知の常用 port の除外を要求する。[隔離と cleanup](../../tests/workspace-navigation/run-isolated.py#L64-L138)・[接続先制約](../../tests/workspace-navigation/playwright.config.ts#L1-L25)。今回の Sync は凍結 source `1458145b8b2db8b00be6d002996a2c64d7a3494d` を静的に照合した。正式 Integration の入力記録は検証省略であり、開発時の成功報告を統合後の成功証拠へ読み替えない。製品テストは再実行せず、無関係な既存節を再検証したとも扱わない。

## 認証が製品画面より先に確定する

Bootstrap は `installAccessFetch` を登録し、`ServerAccessGate` 内に App を置く。gate が access status を取得して本人IDを設定してから製品の providers を mount する。認証不成立や初期設定中は製品画面を表示しない。[初期化順](../../packages/local-web/src/app/entry/Bootstrap.tsx#L79-L99)、[gate](../../packages/local-web/src/app/auth/ServerAccessGate.tsx#L88-L148)

fetch wrapper は同じ origin の `/api/` への変更要求だけに CSRF を補う。業務APIの401/423では状態再取得イベントを送り、外部URLへCSRFを転送しない。これはWebの接続補助で、backendが最終認証を行う。セッション・失効・配置の詳細は [共通認証](../concepts/server-access.md)、歴史的ownerと現在actorの区別は [メンバー](../concepts/organization-and-membership.md)を参照する。[wrapper](../../packages/local-web/src/app/auth/serverAccess.ts#L17-L74)

通知リンクから OAuth login に移る場合、gate は許可された Workspace route と `session_id` / `agent_run_id` だけを sessionStorage に保持する。復帰値は10分以内の一度限りで、認証後の `/` から戻す。storage が使えなくても login を妨げず、通知の再オープンが fallback となる。[復帰経路](../../packages/local-web/src/app/auth/pushReturn.ts)・[gate の接続](../../packages/local-web/src/app/auth/ServerAccessGate.tsx#L101-L164)。通知の意味と安全なクリック先は [Web Push](../integrations/web-push.md)を参照する。

## 同じ API 呼び出しでも対象ホストが変わる

Local の route は TanStack Router の code splitting を使う。読み込み中と chunk 読み込み失敗には共通表示を置き、失敗を無表示のままにしない。この route 単位の遅延読込は、Workspace 内の Wiki / chat 切替で composer を unmount する設計とは異なる。[router](../../packages/local-web/src/app/router/index.ts)、[loading / error](../../packages/web-core/src/shared/components/RouteLoadState.tsx)

API の未定義 path が SPA の `index.html` を成功応答として返すと、client は API 応答として解釈できない HTML を受け取ってしまう。backend は `/api` 以下の未定義 path を 404 とし、通常の画面 route の SPA fallback と分ける。この拒否は JSON 本文を保証する契約ではない。[HTTP fallback](../../crates/server/src/routes/frontend.rs#L15-L33)、[非 HTML・非 redirect の回帰テスト](../../crates/server/src/routes/frontend.rs#L94-L118)

`localApiTransport` は HTTP / WebSocket の両方で、明示 hostId・relayHostId や現在の remote host を送信前に拒否する。古い remote 対象を現在の LVK server へ誤って向けないための境界である。[transport](../../packages/web-core/src/shared/lib/localApiTransport.ts#L35-L61)・[拒否テスト](../../packages/web-core/src/shared/lib/localApiTransport.test.ts)

`remoteApi` は旧名を残すが、現在は常に `/api/local/` へ送り、host scope は none とする。Cloud Bearer refresh や外部 URL への切替はない。[API](../../packages/web-core/src/shared/lib/remoteApi.ts#L8-L24)。認証付き外部 MCP は[別の入口](../integrations/remote-access.md)である。

## Issue データの読み取りと書き込み

[Project・Issue](../concepts/project-and-issue.md)の collection はローカル REST snapshot を取得する。`electric/collections.ts` という名前でも Electric transport は使わない。cached snapshot を表示した後に再取得し、30秒周期でも更新する。取得エラーでは初回 Ready を解除する経路があるため、Ready は取得成功の証明ではない。[snapshot の処理](../../packages/web-core/src/shared/lib/electric/collections.ts#L295-L382)

mutation は REST 成功後に cache を破棄し、進行中の古い取得を待ってから**mutation 後に開始した新しい snapshot**を待つ。これにより楽観更新を古い snapshot で巻き戻さない。txid を Electric へ渡す契約はなくなった。[refresh と mutation](../../packages/web-core/src/shared/lib/electric/collections.ts#L345-L434)・[競合回帰テスト](../../packages/web-core/src/shared/lib/electric/collections.test.ts)

## ドラフト・表示設定・通知の保存境界

設定画面は現在 local LVK server だけを提供し、local 以外の initialHostId を拒否する。旧 discovery/relay の選択規則を現在の接続機能と誤認しない。[設定 scope](../../packages/web-core/src/shared/dialogs/settings/settings/SettingsHostContext.tsx#L20-L48)

Scratch は Board の業務データとは別の UI 状態で、server SQLite と JSON Patch WebSocket を使う。初回取得失敗を空ドラフトと扱わず、ローカル入力を許したまま authoritative snapshot を待つ。composer のキー、保存・復元・消去、送信 queue の違いは [Session の未送信ドラフト](../concepts/session-and-agent-run.md#未送信ドラフトと-scratch)に集約する。[Scratch](../../packages/web-core/src/shared/hooks/useScratch.ts#L28-L82)

Kanban の Project / view 別表示設定も UI preferences の Scratch に入る。同じ画面でも、表示設定を保存する処理と Issue を変更する REST mutation では共有範囲が異なる。Team / Personal、列操作、blocked 判定の意味は [ボード表示と共有状態](../concepts/project-and-issue.md#ボード表示と共有状態)で確認する。[preferences の保存形式](../../packages/web-core/src/shared/hooks/useUiPreferencesScratch.ts#L27-L98)

通知用 hook と group 表示の型が残っていても Cloud 配信の存在を意味しない。[Issue 通知の境界](../concepts/issue-collaboration.md)を確認する。Codex Run の blocking input・approval・終端を対象とする [Web Push](../integrations/web-push.md)は別の opt-in 経路である。

## Agent の会話は別の同期経路

AgentRun stream は Electric ではない。サーバーは Live を先に購読し、その間の到着を受け止めてから永続履歴を replay する。cursor は RunAttempt 番号と canonical sequence で進め、Live 通知で進めない。再接続時に usage の最新 snapshot も送る。[stream](../../crates/server/src/routes/agent_runs.rs#L428-L500)

会話 projection は表示用 entry に変換しても AgentRun、RunAttempt、event ID を保持する。この識別を外すと、別 attempt の表示と制御が混ざり得る。[表示 identity](../../packages/web-core/src/features/agent-runtime/model/canonicalAgentConversation.ts#L25-L73)。永続化・一時差分・劣化時の詳細は [Agent Runtime](agent-runtime.md)に集約する。

### 永続会話と Live の順序

会話内の永続イベントは `run_attempt_number`、canonical `sequence` の順で並べる。native audit の番号とは大きさを比較しない。backend の Steer は provider へ制御を書き込んだ後、command ID を message ID とする User message を native refs なしで保存する。この host 作成イベントを native 番号付きの応答と一律にソートすると、追加指示が古い応答より前へ移動してしまう。[steering の保存](../../crates/local-deployment/src/agent_run_port.rs#L3360-L3400)・[会話順の組立て](../../packages/web-core/src/features/agent-runtime/model/canonicalAgentConversation.ts#L29-L80)

Live delta はまず整列済みの永続列へ挿入位置を求める。同じ attempt の先行候補に native refs があれば native sequence、なければ timestamp を比較し、最後の先行候補の後ろへ置く。先行 attempt も候補となり、候補がなければ列の前に置く。これは一時表示の配置規則であり、永続列の順序や replay cursor を置き換えない。受信側は event ID の重複を除き、確定 message を受け取ると同じ message ID の一時表示を取り除く。[配置](../../packages/web-core/src/features/agent-runtime/model/canonicalAgentConversation.ts#L54-L79)・[確定時の置換](../../packages/web-core/src/features/agent-runtime/model/canonicalAgentTimeline.ts#L175-L204)。入力の受理・queue・停止の意味は [Session の実行状態](../concepts/session-and-agent-run.md#状態停止送信待ち)に置く。

### Run の時間表示を同期状態から分ける

Board の Workspace summary は、Workspace ごとに作成日時と ID の降順で選んだ最新 AgentRun の `created_at` を `latest_process_started_at` に返す。`latest_process_completed_at` はその Run が終端のときだけ現在 attempt の保存済み `finished_at` を返す。`latest_process_*` という互換名は旧 ExecutionProcess の時計を意味しない。[最新 Run の選択](../../crates/db/src/models/agent_runtime.rs#L323-L357)・[API の組立て](../../crates/server/src/routes/workspaces/workspace_summary.rs#L170-L192)

`useWorkspaces` は一覧 stream に summary を重ね、初期化後に15秒周期でも再取得する。Kanban と Issue の Workspace / task-attempt 一覧は同じ時刻を `IssueWorkspaceCard` へ渡す。時計の active 判定は最新 Run の status を使い、Workspace 全体の `is_running` 集計とは別である。別 Run・script・Integration が活動中でも、最新 Run の時計が止まっている場合がある。時計は summary の到着に依存するため、Board と chat の確定表示が同時に切り替わる保証ではない。[summary の合成と poll](../../packages/web-core/src/shared/hooks/useWorkspaces.ts#L56-L89)・[周期取得](../../packages/web-core/src/shared/hooks/useWorkspaces.ts#L168-L194)・[カード](../../packages/ui/src/components/IssueWorkspaceCard.tsx#L249-L275)

Session 側は Run 一覧の `completed_at` を受け、会話 projection が Run ごとに一つの `runTiming` footer を生成し、`ConversationListContainer` が共通の `ChatElapsedTime` を描画する。Goal 専用表示に依存しない。[Run 一覧](../../crates/services/src/services/agent_runtime.rs#L201-L237)・[footer](../../packages/web-core/src/features/agent-runtime/model/canonicalAgentConversation.ts#L648-L679)・[描画](../../packages/web-core/src/features/workspace-chat/ui/ConversationListContainer.tsx#L76-L82)。待機・retry、終了根拠の欠損、再接続時の新旧比較、Goal 会計との違いは [Run の経過時間](../concepts/session-and-agent-run.md#run-の経過時間と-goal-会計)を正本とする。表示のための新旧比較を、stream cursor や実行制御の変更と混同しない。

### 最近の会話を先に表示する

Session の購読は新しい2件と全 active run を foreground とし、その初期化を待って会話を公開する。残る履歴は foreground の準備後、前 batch の初期化またはエラーを待って3件ずつ追加する。既に開始した stream は新しい run の到着でも保持し、Session が変わると keyed component ごとに状態をリセットする。`isLoadingHistory` は初期表示の loading と別であり、古いログの遅延だけで最近の会話や制御を待たせない。[購読と公開](../../packages/web-core/src/features/agent-runtime/model/AgentRunSessionContext.tsx#L80-L278)、[遅延履歴・active run・Session 分離のテスト](../../tests/integration-skip/history.spec.ts)

Project Todo は Board collection や会話 stream とは別の REST query を使う。Project と Workspace の表示は同じ Project のメモを参照し、保存と競合、画面内 draft の寿命は [Todo の契約](../concepts/project-todos.md)にまとめる。

### 履歴の閲覧位置と末尾追従

購読の段階化と、画面上の位置保持は別の責任を持つ。`ConversationList` は Workspace ID と Session scope を React key にし、保留中の rAF・測定値・初回表示状態を会話ごとに分離する。通知を rAF でまとめても、最初に非空の行を適用した一回だけ初回スクロールを行う。遅れて来た `initial` 通知を再度の初回ジャンプと扱わない。[scope と cleanup](../../packages/web-core/src/features/workspace-chat/ui/ConversationListContainer.tsx#L273-L302)・[非空の初回適用](../../packages/web-core/src/features/workspace-chat/ui/ConversationListContainer.tsx#L379-L469)・[key](../../packages/web-core/src/features/workspace-chat/ui/ConversationListContainer.tsx#L1052-L1065)

リストは仮想化された前方と、通常 DOM として描画する末尾を組み合わせる。末尾付近の判定は64pxで、末尾 lock 中は内容領域の `ResizeObserver` でも追従する。画像・Markdown・streamed text の高さだけが変わり、行数や仮想領域の測定サイズが変わらない場合にも必要な補正である。上向き wheel・touch・キー入力などで lock を解放し、末尾付近へ戻れば再取得する。smooth scroll の猶予中は即時補正を抑える。[閾値](../../packages/web-core/src/features/workspace-chat/model/conversation-scroll-commands.ts#L11-L17)・[入力と実寸補正](../../packages/web-core/src/features/workspace-chat/model/useConversationVirtualizer.ts#L259-L378)

過去を読んでいる間に行を更新すると、最初の可視行の semantic key と viewport 内の相対位置を記憶する。prepend 後は同じ行の位置差で scrollTop を補正し、DOM にまだなければ virtualizer でその行へ移動する。行が消えたら anchor を破棄する。末尾の非仮想化境界も数値 index を固定せず、以前の境界行の key から再解決するため、prepend のたびに非仮想 DOM 範囲が広がらない。[anchor と境界の捕捉](../../packages/web-core/src/features/workspace-chat/ui/ConversationListContainer.tsx#L404-L436)・[復元](../../packages/web-core/src/features/workspace-chat/ui/ConversationListContainer.tsx#L558-L601)

anchor は React 更新後だけでなく、末尾 lock がないときの同じ content ResizeObserver からも復元する。prepend 後に viewport より上の非仮想行が DOM だけで遅れて伸びる場合を取りこぼさないためである。anchor 保持中は virtualizer 側のサイズ補正を抑えて二重補正を避ける。wheel / touchstart / pointerdown / keydown や明示 jump は anchor を解放し、利用者の次の閲覧操作を固定し続けない。[補正の調停](../../packages/web-core/src/features/workspace-chat/ui/ConversationListContainer.tsx#L315-L329)・[resize](../../packages/web-core/src/features/workspace-chat/model/useConversationVirtualizer.ts#L360-L378)・[操作による解除](../../packages/web-core/src/features/workspace-chat/ui/ConversationListContainer.tsx#L901-L920)

### チャット表示の検証境界

変更時は layout の Chromium fixture で prepend・遅い initial・DOM の遅延 resize 後の同一行座標と scope 切替を確認し、canonical projection の unit test で live と replay の `before → steer → after`、重複、Run identity を確認する。順序 assertion は user / assistant message を対象にし、時間 footer を会話本文と混同しない。[layout 試験](../../tests/chat/conversation-layout.test.mjs#L87-L172)・[順序試験](../../packages/web-core/src/features/agent-runtime/model/canonicalAgentConversation.test.ts#L100-L166)。composer の試験対象は [送信 snapshot の保護](../concepts/session-and-agent-run.md#送信中の編集と-markdown-正規化)を参照する。

built-app の隔離試験は、実 backend と合成履歴、実 Scratch と制御した follow-up 応答、実 Codex steering の3経路を区別する。[試験定義](../../tests/chat/isolated.spec.ts)と [再実行手順・過去の結果](../../docs/development/chat-stability-local21.md)を参照し、component fixture の成功を provider 全体の保証にしない。特に遅延 DOM resize の追加検証は実画像ダウンロードや全体 E2E の再実行ではない。

今回の Sync は凍結 source `a044769b1dfdf1972c6e10937fa0dfb98faf31db` のチャット変更を、購読・投影・backend の steering 保存・リスト・editor・試験定義という独立した入口で静的に照合した。凍結入力の正式 Integration は検証省略であり、開発時の成功記録を統合後の成功証拠へ読み替えない。この Sync で製品テストを再実行したことや、無関係な過去の Wiki 全体を再検証したことは意味しない。

## チャットのコード装飾と本文の境界

コードの色付けには二つの表示経路がある。command と script は UI の `ChatCode` が Prism token を React の文字列と span に変換し、Markdown fence は `AppChatMarkdown` → disabled の `WYSIWYGEditor` → Lexical `CodeHighlightPlugin` を通る。両方が既存の `CODE_HIGHLIGHT_CLASSES` と syntax CSS 変数を使う。UI package は Prism を直接依存に持ち、共有 helper が bash / JSON grammar を登録するため、command 側だけでなく Lexical 側の対応言語にも関わる。[文字列の描画](../../packages/ui/src/components/ChatCode.tsx#L1-L31)・[grammar と上限](../../packages/ui/src/lib/chat-code-highlighting.ts#L1-L23)・[Markdown の接続](../../packages/web-core/src/features/workspace-chat/ui/AppChatMarkdown.tsx#L22-L39)・[共通テーマ](../../packages/ui/src/lib/code-highlight-theme.ts#L1-L36)

ツールの見出しは実 command と表示 summary が等しい場合だけ装飾し、説明文を shell code として扱わない。展開詳細は空白だけの command を除外する判定に trim を使うが、描画には前後空白を含む元の文字列を渡す。script の command も同じ描画部品を使い、ツール出力本文は別の plain pre に残る。ここでの `language="bash"` は表示側が固定した字句装飾であり、provider が実際に選んだ shell を証明しない。[見出しと詳細](../../packages/ui/src/components/ChatAggregatedToolEntries.tsx#L46-L77)・[単一見出し](../../packages/ui/src/components/ChatAggregatedToolEntries.tsx#L153-L163)・[集約内の見出し](../../packages/ui/src/components/ChatAggregatedToolEntries.tsx#L264-L279)・[script](../../packages/ui/src/components/ChatScriptEntry.tsx#L78-L89)。実行と event の責任は [Agent Runtime](agent-runtime.md)にあり、色付けから実行分類を逆算しない。

同期 tokenization は文字列の `length` が16,384以下で、指定言語が登録済みの場合に限る。これは UTF-16 code unit 数の上限であり、byte 数や表示上の文字数ではない。長文・未指定・未対応言語、tokenizer の例外では全文を plain 表示し、切り捨てない。read-only Lexical は既定言語を `text` にし、fallback 時も改行・tab node を作り直す。editable editor は従来の tokenizer を使うため、この上限を composer 全体の入力制限と解釈しない。コードコメントは streaming 中の同期処理を有界にする目的を記録している。[guard と fallback](../../packages/ui/src/lib/chat-code-highlighting.ts#L5-L23)・[read-only tokenizer](../../packages/ui/src/components/CodeHighlightPlugin.tsx#L11-L46)・[disabled の接続](../../packages/web-core/src/shared/components/WYSIWYGEditor.tsx#L641-L650)

`ChatCode` は装飾のみを所有し、コピー、空白のレイアウト、折返しは呼出側の責任である。Markdown コピーは装飾 DOM を読み戻さず editor の `value` から行う。ただし既存処理として escaped underscore を unescape するため、全入力について byte 単位の同一性を保証する説明は誤りになる。[装飾の責任](../../packages/ui/src/components/ChatCode.tsx#L18-L31)・[Markdown コピー](../../packages/web-core/src/shared/components/WYSIWYGEditor.tsx#L447-L459)。入力の保存・送信は [Session の composer 契約](../concepts/session-and-agent-run.md#送信中の編集と-markdown-正規化)、高さ変化への追従は本ページの[履歴の閲覧位置](#履歴の閲覧位置と末尾追従)を参照する。

検証の入口は `syntax-highlight.test.mjs` の Chromium fixture と `syntax-isolated-e2e.mjs` の built-app 試験である。前者は部分文字列・未閉鎖 fence、bash/sh/JSON などの装飾、未知・未指定・長文の fallback、文字列と focus の保持を検査する。後者は build 済み frontend/server と一時 SQLite の Workspace / Session を使い、AgentRun 一覧と WebSocket event を fixture に置き換えて command コピー、Markdown コピー、入力保持、light/dark、長文を検査する。provider の生成そのものを検証する試験ではない。終了時には browser/server、残存 PID、port、一時DBの cleanup を確認する。[component 試験](../../tests/chat/syntax-highlight.test.mjs#L7-L108)・[隔離条件](../../tests/chat/syntax-isolated-e2e.mjs#L1-L81)・[fixture と操作](../../tests/chat/syntax-isolated-e2e.mjs#L146-L325)・[cleanup](../../tests/chat/syntax-isolated-e2e.mjs#L350-L414)。再実行の環境準備は [開発と検証](../operations/development.md)を参照する。今回のコード装飾 Sync は凍結 source `777488ca3a430b56d19e2df9437de5f848dc00e2` と試験定義を静的に照合したもので、製品試験を再実行していない。供給された正式 Integration の記録も validation skipped であり、開発時の成功報告を統合後の検証成功へ読み替えない。

## チャットのファイルリンクを既存 panel へ渡す

`AppChatMarkdown` は read-only の `WYSIWYGEditor` に `useChatFileLink` の resolver を渡す。UI package の `ReadOnlyLinkPlugin` は Repo や API を所有せず、解決された callback だけを実行する。既存の inline code → Changes 操作は別 callback のまま残る。Workspace / Repo metadata と preview action が揃わない間はローカルリンクを有効化しない。[チャット側の接続](../../packages/web-core/src/features/workspace-chat/ui/AppChatMarkdown.tsx#L19-L39)、[metadata と action の依存](../../packages/web-core/src/features/workspace-files/model/useChatFileLink.ts#L7-L33)

リンク DOM の `href` は無効化・sanitization 済みの場合があるため、plugin は Lexical node の元 URL から再評価する。resolver が変わると既存 node も処理し直すので、後から Workspace metadata が到着した場合にも有効状態を更新できる。ローカルリンクは DOM の href を除去し、click / Enter で callback を呼ぶ。HTTPS は通常の新規タブと `noopener noreferrer` を保つ。[DOM 更新と再評価](../../packages/ui/src/components/ReadOnlyLinkPlugin.tsx#L15-L77)

通常 layout の bridge は解決済み target を `openTarget` へ渡し、right main panel を `FILES` へ切り替える。これにより会話からも既存の file selection / viewer を使う。ファイル名の形式、行指定、曖昧さと拒否、backend の最終検査は [チャットから Repo 内ファイルを開く契約](../operations/workspace-inspection.md#チャットから-repo-内ファイルを開く)を正本とする。[panel への接続](../../packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx#L650-L677)

## State と履歴を同期させる際の注意

AgentRun stream は新しい event がゼロでも State が変われば送信する。履歴が複数ページに跨る場合は、残りの event より先に terminal State を表示させない。DB commit 後の通知は再読取のヒントであり、取りこぼしは周期 poll で補う。catch-up を有界ページで進めて Live と切断処理を挟み、WebSocket の送信にも有限 timeout を設ける。[履歴と State](../../crates/server/src/routes/agent_runs.rs#L450-L585)

JSON Patch stream は接続成立と初回 snapshot の Ready を分ける。再接続時は空 accumulator へ replay しつつ同じ endpoint の最後の表示を保持し、別 endpoint へ切り替えた場合は引き継がない。接続失敗を「保存済み draft は空」の証明に使わない。[snapshot の境界](../../packages/web-core/src/shared/hooks/useJsonPatchWsStream.ts#L90-L166)

## Wiki のナビゲーションと本文を一つの状態で動かす

Workspace の Wiki は本文をメイン領域、ナビゲーションを右側へ分けて表示するが、二つの独立した reader ではない。`WorkspaceWikiProvider` が Workspace ID ごとに一つの controller を所有し、repository、選択ページ、検索、読取結果を両 surface で共有する。モバイルでは本文とナビゲーションを切り替える。[共有 controller](../../packages/web-core/src/features/wiki/ui/WorkspaceWikiProvider.tsx#L39-L91)、[画面への配置](../../packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx#L509-L628)

表示対象は canonical OpenWiki 一種類であり、legacy 形式の切替状態は持たない。repository を切り替えると snapshot・選択・検索・scroll 状態をリセットし、再取得する。controller はリクエスト sequence と応答の Workspace / repository identity を検査し、遅れて返った別対象の結果で現在の画面を上書きしない。[scope と応答の扱い](../../packages/web-core/src/features/wiki/model/wikiViewerController.ts#L11-L119)。実際に読む checkout と reader の範囲は [Wiki inspection](../operations/workspace-inspection.md#wiki-の本文ツリーと表示元を確認する)を参照する。

Wiki を開くことはチャットや AgentRun の終了ではない。`PreservedChatPanel` は composer と timeline を unmount せず、非表示時に `inert` / `aria-hidden` を付ける。最後の表示幅を保つのは、幅ゼロによる仮想化 timeline の再計算を避けるためである。Wiki の表示設定と実行 lifecycle を結び付けない。[chat の保持](../../packages/web-core/src/pages/workspaces/PreservedChatPanel.tsx#L4-L52)。操作と表示元の確認は [Workspace の検査](../operations/workspace-inspection.md)を参照する。

## 統一一覧と実行予約

Workspace 一覧は execution_only を恒久的な別カテゴリへ分離しない。stream の用途変更を詳細 cache に反映し、実行予約解放後に通常の操作へ戻れる。running 表示では最新一件だけでなく summary の実行集計を優先する。[一覧](../../packages/web-core/src/shared/hooks/useWorkspaces.ts)・[用途変更時の詳細 cache 無効化](../../packages/web-core/src/shared/providers/WorkspaceProvider.tsx#L72-L97)。予約と解放条件の正本は [Workspace](../concepts/workspace.md#操作用途と実行所有者)である。

予約中は inspection panel が owner view と Stop を扱い、現在実行中の Wiki agent には追加指示を送れる。owner query は host / Workspace をキーにし、取得失敗を空の成功へ変換しない。[owner view](../../packages/web-core/src/shared/hooks/useWorkspaceOwner.ts)・[指示 UI](../../packages/web-core/src/features/workspace-chat/ui/ExecutionInspectionPanel.tsx#L159-L175)。backend も所有者を再検証するため、古い画面の操作権限を信頼しない。

Session 一覧の再取得では利用者が選んだ古いログや new-session draft を維持する。host / Workspace または明示 Session link の変更時だけ選択を再解決し、入口に使った URL がその後の手動選択を奪わない。[選択規則](../../packages/web-core/src/shared/hooks/workspaceSessionSelection.ts#L3-L25)、[scope の更新](../../packages/web-core/src/shared/hooks/useWorkspaceSessions.ts#L51-L87)、[回帰テスト](../../packages/web-core/src/shared/hooks/workspaceSessionSelection.test.ts#L4-L36)

## モバイルと文書の適用範囲

モバイル shell は `visualViewport.height` を CSS 変数 `--app-vh` に反映する。キーボード表示時の composer 隠れを避け、重い shell を viewport のたびに React render しない意図がコードに記録されている。[実装と理由](../../packages/web-core/src/shared/hooks/useVisualViewportHeightVar.ts#L3-L50)

[モバイル設計](../../docs/ai-mobile/design.md)は draft、[390px 監査](../../docs/ai-mobile/audit-2026-07-07.md)は過去時点のギャップ調査である。QR、PWA、native shell の構想を現行保証へ置き換えず、実画面・hooks・transport の実装を確認する。画布の見た目や操作設計は [Workflow UI 設計](../../docs/future/ai-workflow/spec-ui.md)に委ね、実行順序の意味は [Workflow Attempt](../concepts/workflow-attempt.md)で確認する。
