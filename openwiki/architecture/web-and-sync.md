---
type: architecture
title: Web の実行環境と同期
description: ローカル Web、Board snapshot、Agent Run stream と表示状態の異なる同期契約を説明する。
tags: [frontend, synchronization, transport, persistence]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-22T05:31:45.518Z
sources:
  - id: openwiki-source-162bcfe1ee47c1047ccf7d52
    resource: repo://crates/server/src/routes/agent_runs.rs
  - id: openwiki-source-71d9a385f4b03b8ca71765b0
    resource: repo://packages/web-core/src/features/wiki/model/wikiViewerController.ts
  - id: openwiki-source-e665e0fe8e478ec542e0ace4
    resource: repo://packages/web-core/src/shared/lib/electric/collections.ts
generated: { by: "codex", at: "2026-11-22T05:31:45.518Z" }
---

# Web の実行環境と同期

Local Web はブラウザーの shell と共有画面を組み合わせ、[Project と Issue](../concepts/project-and-issue.md) や [Session](../concepts/session-and-agent-run.md) を表示する。画面の取得、保存、実行状態を変更するときに、各データ経路の鮮度と所有者を区別する。受付から内容受入までの区別は[操作の段階](../quickstart.md#操作の段階)を参照する。

## Shell と認証

`packages/local-web` は router、設定と認証 gate を組み立て、`packages/web-core` は共有画面と hook、`packages/ui` は再利用部品を持つ。ブラウザーは現行のサーバーへ接続する。旧 native shell や Cloud relay の名前が残っていても、それらが現在の実行環境を意味しない。[App](../../packages/local-web/src/app/entry/App.tsx)・[システム境界](system.md)

Bootstrap は fetch 補助を登録し、製品 App を `ServerAccessGate` の内側で遅延読込する。fetch 補助は同一 origin の `/api/` に対する変更要求だけへ CSRF を付け、業務 API の 401/423 で認証状態の再確認を促す。最終的な認証は backend の責任である。[Bootstrap](../../packages/local-web/src/app/entry/Bootstrap.tsx)・[fetch 補助](../../packages/local-web/src/app/auth/serverAccess.ts)・[サーバー認証](../concepts/server-access.md)

`remoteApi` は歴史的な名前だが、現在は `/api/local/` を使う。local transport は古い remote host 指定を HTTP と WebSocket の送信前に拒否する。未知の `/api` は SPA の HTML 成功応答にせず 404 とし、通常画面の fallback と区別する。[API](../../packages/web-core/src/shared/lib/remoteApi.ts)・[transport](../../packages/web-core/src/shared/lib/localApiTransport.ts)・[fallback](../../crates/server/src/routes/frontend.rs)

## Board snapshot と書込み

Board collection はローカル REST snapshot を読む。`electric` というディレクトリ名は Electric transport の使用を意味しない。cache があれば先に表示し、取得を開始する。購読中は30秒ごとに更新し、購読者がいない間は周期取得を止め、再購読時に取得する。初回エラーでも Ready にする経路があるため、Ready は取得成功の証拠ではない。[collection](../../packages/web-core/src/shared/lib/electric/collections.ts)

Mutation 成功後は cache を無効化し、進行中の旧取得を待った後に新しい snapshot を取得する。古い応答で楽観更新を巻き戻さないための順序である。Issue 本文の互換書込みは元の `updated_at` を期待値として渡す。[mutation と refresh](../../packages/web-core/src/shared/lib/electric/collections.ts)・[競合試験](../../packages/web-core/src/shared/lib/electric/collections.test.ts)

Kanban の drag 保存は `KanbanDropSync` が直列化する。HTTP 成功は希望順位の成立と同義ではなく、保存後の読取と新鮮な snapshot を待つ。保存失敗、読取失敗、競合を区別し、同期の再試行は書込みを再送しない。[drop 同期](../../packages/web-core/src/features/kanban/ui/kanbanDropSync.ts)・[Issue の順位](../concepts/project-and-issue.md)

表示設定と未送信 draft は業務データの mutation と別の Scratch 状態である。初回取得失敗を空の保存済み draft とみなさない。[Scratch](../../packages/web-core/src/shared/hooks/useScratch.ts)・[Session の draft](../concepts/session-and-agent-run.md)

## Agent Run の会話

Agent Run stream は Board snapshot と別の WebSocket 経路を使う。サーバーは Live を先に購読して replay 中の到着を受け止め、永続履歴を attempt 番号と canonical sequence の cursor で送る。Live は耐久 cursor を進めない。履歴の後に state と usage snapshot を送り、通知を逃した場合は周期的な再読取で補う。[stream](../../crates/server/src/routes/agent_runs.rs)

クライアントは cursor から再接続し、重複 event を除く。切断時には一時的な Live 表示を落とし、確定 message の到着で同じ message の差分を収束させる。会話の表示順を Native Audit の sequence と混ぜない。[購読](../../packages/web-core/src/features/agent-runtime/model/useAgentRunCanonicalStream.ts)・[timeline](../../packages/web-core/src/features/agent-runtime/model/canonicalAgentTimeline.ts)・[監査と投影](agent-runtime.md)

Session の画面は最近の Run と活動中 Run を先に表示し、古い履歴を段階的に読む。履歴の取得状態は初回表示と別であり、先頭への移動は履歴取得を待つ。閲覧位置と末尾追従も別に保持し、履歴を読んでいる人を新しい出力で強制的に末尾へ移さない。[Session context](../../packages/web-core/src/features/agent-runtime/model/AgentRunSessionContext.tsx)・[閲覧位置](../../packages/web-core/src/features/workspace-chat/model/useConversationVirtualizer.ts)

承認待ちの表示は元の tool 内容を保持し、質問の ID と選択肢も投影する。画面表示や回答受付は provider 側の実行成功を証明しない。[会話投影](../../packages/web-core/src/features/agent-runtime/model/canonicalAgentConversation.ts)・[質問](../../packages/web-core/src/features/agent-runtime/model/nativeAgentQuestions.ts)・[承認の所有者](agent-runtime.md#承認と指示配送)

## Wiki と画面の寿命

Wiki controller は Workspace と Repository の identity と要求 sequence を検査する。Repository 切替では選択・検索・scroll をリセットし、古い応答が現在の対象を上書きしない。Wiki を開いても chat の composer と timeline は保持され、表示の切替は Agent Run の停止を意味しない。[controller](../../packages/web-core/src/features/wiki/model/wikiViewerController.ts)・[chat 保持](../../packages/web-core/src/pages/workspaces/PreservedChatPanel.tsx)・[表示元の確認](../operations/workspace-inspection.md)

ログの表示・コピーは選択 process の stream を共有する一方、DevServer URL 検出は独立している。ログ画面を離れることは DevServer の停止ではない。[ログ](../../packages/web-core/src/shared/providers/LogsPanelProvider.tsx)・[DevServer](../../packages/web-core/src/pages/workspaces/DevServerControlsContainer.tsx)

Markdown の閲覧装飾は保存本文を変更しない。引用の明示 marker、目次、ファイルリンクと copy はそれぞれの表示・操作境界で扱う。移送済み Project の本文は旧参照を実行しない literal 表示を使う。[Markdown](../../packages/ui/src/components/MarkdownReadingSurface.tsx)・[ファイルリンク](../../packages/web-core/src/features/workspace-files/model/useChatFileLink.ts)・[移送](../concepts/project-transfer.md)

Collection の競合試験、会話 projection、履歴と Markdown の fixture はそれぞれ対象を絞った回帰である。合成 HTTP/WS や component fixture の成功は実 provider、認証済み製品全体、物理端末の受入を証明しない。[collection tests](../../packages/web-core/src/shared/lib/electric/collections.test.ts)・[chat tests](../../tests/chat/isolated.spec.ts)・[Markdown tests](../../tests/markdown/README.md)
