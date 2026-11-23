---
type: concept
title: Kanban board
description: Project の Issue を状態別に表示するボード。
tags: [kanban, board, project]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T02:42:04.517Z
sources:
  - id: openwiki-source-f4e0f9b2ce513d3f8b8b070e
    resource: repo://crates/executors/src/executors/provider_adapter.rs
  - id: openwiki-source-3d5de3bbc96079ee456ed0f5
    resource: repo://crates/server/src/routes/board_run_events.rs
  - id: openwiki-source-699012344cbfd472d165f098
    resource: repo://crates/server/src/routes/local_remote/board_api/mod.rs
  - id: openwiki-source-92cb09ac7279f8310b0dd21e
    resource: repo://packages/local-web/src/routes/_app.projects.%24projectId.tsx
  - id: openwiki-source-fd4f99568f51e2e5788058d5
    resource: repo://packages/web-core/src/features/kanban/model/runObservationPolling.ts
  - id: openwiki-source-fa9be7983cc21002d2346ef7
    resource: repo://packages/web-core/src/features/kanban/model/runSpeechQueue.ts
  - id: openwiki-source-e0c2de9e01617a8949b78360
    resource: repo://packages/web-core/src/features/kanban/ui/KanbanContainer.tsx
  - id: openwiki-source-461fe232dcc20677a51fbe76
    resource: repo://packages/web-core/src/features/kanban/ui/RunSpeechToggle.tsx
generated: { by: "codex", at: "2026-11-23T02:42:04.517Z" }
---

# Kanban board

Kanban board は [Project](project.md) の [Issue](issue.md) を [Column](column.md) に並べる主画面である。仕事を選び、状態と順序を確認するときに使う。[操作の段階](../quickstart.md#操作の段階)

[Card](card.md) の移動は共有 Issue の状態や順位を変更する。表示 filter と実行状態は別であり、表示列だけから処理の完了や統合を判定しない。自動状態同期や archive の詳細は [Project と Issue](project-and-issue.md) にある。

Project の route は Kanban 画面を開き、Board API はその Project の Issue と status を取得する。[画面入口](../../packages/local-web/src/routes/_app.projects.$projectId.tsx)・[Board API](../../crates/server/src/routes/local_remote/board_api/mod.rs)

## Run の進捗を音声で聞く

音声切替は、複数の [Workspace](workspace.md) で動く [Run](agent-run.md) を個々の画面で見張らずに把握するための機能である。ボード右上のスピーカーをオンにすると、その Project に属する新着 Run イベントを Issue 番号付きで読む。表示中の Workspace だけに対象を絞らない。ボードを開いた時点と Project 切替時はオフで、設定は保存しない。[画面への接続](../../packages/web-core/src/features/kanban/ui/KanbanContainer.tsx)

1. `speechSynthesis` を利用できるブラウザで対象 Project のボードを開く。非対応の場合は切替が無効になる。
2. スピーカーを押す。新着の基準位置を取得する間は接続中となり、取得できてからオンになる。接続に失敗した場合はオフのままなので、接続状態を確認して再操作する。
3. 開始、成功・失敗・停止、回答待ちの短い定型文と、agent が人へ送った commentary を聞く。現在の発話言語は `ja-JP` である。
4. オフにすると再生中の発話と待ち行列を即時に止める。再度オンにしても停止中の履歴は読み返さない。ボードを離れた場合も購読と発話を停止する。

[音声切替](../../packages/web-core/src/features/kanban/ui/RunSpeechToggle.tsx)はブラウザの発話機能を呼び出す。サーバーに音声キューや別モデルによる要約は設けない。

### 読み上げ順序と古いイベント

ブラウザの待ち行列は同時に一つだけ発話する。優先度は次のとおりで、同じ優先度では到着順を保つ。

| 優先度 | 内容 | 未再生分の扱い |
| --- | --- | --- |
| 高 | 回答待ち・終了 | 回答済みの待機通知は除去する。終了時は同じ Run の古い開始・中間・待機通知を除去する |
| 中 | 開始 | 未再生のまま終了した場合は終了だけを読む |
| 低 | commentary | Run ごとに最新一件を残す。終了済み Run の未再生分を除去する |

再生直前にも開始・中間は Run の活動状態、回答待ちは未回答の存在を問い合わせる。問い合わせ失敗時はその発話を見送る。別 Run の優先イベントが到着しても、既に読み始めた文は割り込まない。明示的なオフは即時停止する。[待ち行列](../../packages/web-core/src/features/kanban/model/runSpeechQueue.ts)

### Canonical イベントからの取得

`GET /api/projects/{project_id}/run-observations` は既存の [Agent Runtime](../architecture/agent-runtime.md) の canonical イベントを Project と Issue の関連で絞り込む。`after` のない初回取得は同一 SQLite snapshot の最新位置を基準とし、過去の本文を送らない。以後は cursor から最大256行を走査し、未回答と活動中 Run の snapshot も返す。[取得 API](../../crates/server/src/routes/board_run_events.rs)

ブラウザは応答の後に約1秒間隔で次の取得を行う。接続エラー後は cursor を捨てて新しい基準位置を取得するため、切断中の履歴を読み返さない。[ポーリング](../../packages/web-core/src/features/kanban/model/runObservationPolling.ts)

commentary の分類は provider が明示した情報に基づく。Codex は `phase=commentary`、Claude Code は同じ root メッセージに対応する `tool_use` 終了理由を使う。Claude の未確定本文は候補として保持し、対応する確定通知が来るまで読まない。旧 mapper は旧版の再生契約を維持する。[provider adapter](../../crates/executors/src/executors/provider_adapter.rs)

注意点:

- 思考、ツール入出力、最終応答本文は音声対象に含めない。終了通知は成果の内容受入や Git 公開を意味しない。
- Claude の終了理由を受け取る前に切断した場合、誤読を避けるため未確定本文は読み上げない。
- 再接続後は新着から再開する。切断中の報告を取り戻す機能ではない。

[キュー試験](../../packages/web-core/src/features/kanban/model/runSpeechQueue.test.ts)、[接続試験](../../packages/web-core/src/features/kanban/model/runObservationPolling.test.ts)、[切替試験](../../packages/web-core/src/features/kanban/ui/RunSpeechToggle.test.tsx)と API・adapter の試験は分類、順序、再接続、Project 分離を扱う。これらは Chrome の別ウィンドウでの実音声を証明せず、その実機確認は未実施として残る。
