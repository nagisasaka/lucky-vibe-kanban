---
type: architecture
title: Web の実行環境と同期
description: Local shell と共有画面の責任、ローカル snapshot と AgentRun stream の異なる同期契約。
tags: [frontend, synchronization, electric, transport]
sources:
  - id: openwiki-source-162bcfe1ee47c1047ccf7d52
    resource: repo://crates/server/src/routes/agent_runs.rs
  - id: openwiki-source-4f90d37fb90b7f5198b7b293
    resource: repo://crates/server/src/routes/frontend.rs
  - id: openwiki-source-a623a548ee805a854d7d5e18
    resource: repo://packages/local-web/src/app/entry/App.tsx
  - id: openwiki-source-71d9a385f4b03b8ca71765b0
    resource: repo://packages/web-core/src/features/wiki/model/wikiViewerController.ts
  - id: openwiki-source-202a77c218ffbc8ac2871a04
    resource: repo://packages/web-core/src/features/wiki/ui/WorkspaceWikiProvider.tsx
  - id: openwiki-source-a045ed78b54878edd07211c7
    resource: repo://packages/web-core/src/features/workspace-chat/ui/ExecutionInspectionPanel.tsx
  - id: openwiki-source-2fb74c0fc81d817149606557
    resource: repo://packages/web-core/src/pages/workspaces/PreservedChatPanel.tsx
  - id: openwiki-source-0894249b732085a9800940bd
    resource: repo://packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx
  - id: openwiki-source-a7cac6e6f3abafbcf9390ddf
    resource: repo://packages/web-core/src/shared/components/RouteLoadState.tsx
  - id: openwiki-source-a1a870cf32aedb317f9ea946
    resource: repo://packages/web-core/src/shared/dialogs/settings/settings/SettingsHostContext.tsx
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
generated: { by: "codex", at: "2026-10-16T12:21:28.631Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-16T12:21:28.631Z
---

# Web の実行環境と同期

## 画面と実行環境を分ける

Local App は TanStack Router と Hotkeys、設定、ローカル identity、Tauri listener を組み立てる。共有画面と hooks は web-core にある。Remote Web shell は削除済みである。[Local App](../../packages/local-web/src/app/entry/App.tsx)

UI package は既に Kanban や chat、preview などの再利用部品を持つ。[Kanban の例](../../packages/ui/src/components/KanbanBoard.tsx#L1-L58)。`packages/ui/README.md` の「initial scaffold」と [抽出前の監査](../../docs/frontend-ui-library-refactor-audit.md)は現行配置の全体図ではない。監査は責任分割の経緯を知る資料として読み、古い `local-web/src/components` の位置を変更先にしない。

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

通知用 hook と group 表示の型が残っていても Cloud 配信の存在を意味しない。[現在の通知境界](../concepts/issue-collaboration.md)を確認する。

## Agent の会話は別の同期経路

AgentRun stream は Electric ではない。サーバーは Live を先に購読し、その間の到着を受け止めてから永続履歴を replay する。cursor は RunAttempt 番号と canonical sequence で進め、Live 通知で進めない。再接続時に usage の最新 snapshot も送る。[stream](../../crates/server/src/routes/agent_runs.rs#L428-L500)

会話 projection は表示用 entry に変換しても AgentRun、RunAttempt、event ID を保持する。この識別を外すと、別 attempt の表示と制御が混ざり得る。[表示 identity](../../packages/web-core/src/features/agent-runtime/model/canonicalAgentConversation.ts#L25-L73)。永続化・一時差分・劣化時の詳細は [Agent Runtime](agent-runtime.md)に集約する。

## State と履歴を同期させる際の注意

AgentRun stream は新しい event がゼロでも State が変われば送信する。履歴が複数ページに跨る場合は、残りの event より先に terminal State を表示させない。DB commit 後の通知は再読取のヒントであり、取りこぼしは周期 poll で補う。catch-up を有界ページで進めて Live と切断処理を挟み、WebSocket の送信にも有限 timeout を設ける。[履歴と State](../../crates/server/src/routes/agent_runs.rs#L450-L585)

JSON Patch stream は接続成立と初回 snapshot の Ready を分ける。再接続時は空 accumulator へ replay しつつ同じ endpoint の最後の表示を保持し、別 endpoint へ切り替えた場合は引き継がない。接続失敗を「保存済み draft は空」の証明に使わない。[snapshot の境界](../../packages/web-core/src/shared/hooks/useJsonPatchWsStream.ts#L90-L166)

## Wiki のナビゲーションと本文を一つの状態で動かす

Workspace の Wiki は本文をメイン領域、ナビゲーションを右側へ分けて表示するが、二つの独立した reader ではない。`WorkspaceWikiProvider` が Workspace ID ごとに一つの controller を所有し、repository、選択ページ、検索、読取結果を両 surface で共有する。モバイルでは本文とナビゲーションを切り替える。[共有 controller](../../packages/web-core/src/features/wiki/ui/WorkspaceWikiProvider.tsx#L39-L91)、[画面への配置](../../packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx#L509-L628)

表示対象は canonical OpenWiki 一種類であり、legacy 形式の切替状態は持たない。repository を切り替えると snapshot・選択・検索・scroll 状態をリセットし、再取得する。controller はリクエスト sequence と応答の Workspace / repository identity を検査し、遅れて返った別対象の結果で現在の画面を上書きしない。[scope と応答の扱い](../../packages/web-core/src/features/wiki/model/wikiViewerController.ts#L11-L119)。実際に読む checkout と reader の範囲は [Wiki inspection](../operations/workspace-inspection.md#wiki-の本文ツリーと表示元を確認する)を参照する。

Wiki を開くことはチャットや AgentRun の終了ではない。`PreservedChatPanel` は composer と timeline を unmount せず、非表示時に `inert` / `aria-hidden` を付ける。最後の表示幅を保つのは、幅ゼロによる仮想化 timeline の再計算を避けるためである。Wiki の表示設定と実行 lifecycle を結び付けない。[chat の保持](../../packages/web-core/src/pages/workspaces/PreservedChatPanel.tsx#L4-L52)。操作と表示元の確認は [Workspace の検査](../operations/workspace-inspection.md)を参照する。

## 統一一覧と実行予約

Workspace 一覧は execution_only を恒久的な別カテゴリへ分離しない。stream の用途変更を詳細 cache に反映し、実行予約解放後に通常の操作へ戻れる。running 表示では最新一件だけでなく summary の実行集計を優先する。[一覧](../../packages/web-core/src/shared/hooks/useWorkspaces.ts)。予約と解放条件の正本は [Workspace](../concepts/workspace.md#操作用途と実行所有者)である。

予約中は inspection panel が owner view と Stop を扱い、現在実行中の Wiki agent には追加指示を送れる。owner query は host / Workspace をキーにし、取得失敗を空の成功へ変換しない。[owner view](../../packages/web-core/src/shared/hooks/useWorkspaceOwner.ts)・[指示 UI](../../packages/web-core/src/features/workspace-chat/ui/ExecutionInspectionPanel.tsx#L159-L175)。backend も所有者を再検証するため、古い画面の操作権限を信頼しない。

Session 一覧の再取得では利用者が選んだ古いログや new-session draft を維持する。host / Workspace または明示 Session link の変更時だけ選択を再解決し、入口に使った URL がその後の手動選択を奪わない。[選択規則](../../packages/web-core/src/shared/hooks/workspaceSessionSelection.ts#L3-L25)、[scope の更新](../../packages/web-core/src/shared/hooks/useWorkspaceSessions.ts#L51-L87)、[回帰テスト](../../packages/web-core/src/shared/hooks/workspaceSessionSelection.test.ts#L4-L36)

## モバイルと文書の適用範囲

モバイル shell は `visualViewport.height` を CSS 変数 `--app-vh` に反映する。キーボード表示時の composer 隠れを避け、重い shell を viewport のたびに React render しない意図がコードに記録されている。[実装と理由](../../packages/web-core/src/shared/hooks/useVisualViewportHeightVar.ts#L3-L50)

[モバイル設計](../../docs/ai-mobile/design.md)は draft、[390px 監査](../../docs/ai-mobile/audit-2026-07-07.md)は過去時点のギャップ調査である。QR、PWA、native shell の構想を現行保証へ置き換えず、実画面・hooks・transport の実装を確認する。画布の見た目や操作設計は [Workflow UI 設計](../../docs/future/ai-workflow/spec-ui.md)に委ね、実行順序の意味は [Workflow Attempt](../concepts/workflow-attempt.md)で確認する。
