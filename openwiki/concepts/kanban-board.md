---
type: concept
title: Kanban board
description: Project の Issue を状態別に表示するボード。
tags: [kanban, board, project]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-24T02:54:42.940Z
sources:
  - id: openwiki-source-f4e0f9b2ce513d3f8b8b070e
    resource: repo://crates/executors/src/executors/provider_adapter.rs
  - id: openwiki-source-3d5de3bbc96079ee456ed0f5
    resource: repo://crates/server/src/routes/board_run_events.rs
  - id: openwiki-source-699012344cbfd472d165f098
    resource: repo://crates/server/src/routes/local_remote/board_api/mod.rs
  - id: openwiki-source-f7410fb786e9a362721ca134
    resource: repo://packages/local-web/src/app/navigation/AppNavigation.ts
  - id: openwiki-source-92cb09ac7279f8310b0dd21e
    resource: repo://packages/local-web/src/routes/_app.projects.%24projectId.tsx
  - id: openwiki-source-fd4f99568f51e2e5788058d5
    resource: repo://packages/web-core/src/features/kanban/model/runObservationPolling.ts
  - id: openwiki-source-fa9be7983cc21002d2346ef7
    resource: repo://packages/web-core/src/features/kanban/model/runSpeechQueue.ts
  - id: openwiki-source-e0c2de9e01617a8949b78360
    resource: repo://packages/web-core/src/features/kanban/ui/KanbanContainer.tsx
  - id: openwiki-source-2706dbba10a20a516c29675e
    resource: repo://packages/web-core/src/features/kanban/ui/RunSpeechProvider.tsx
  - id: openwiki-source-4f5acbe99dd0d72515b7e82b
    resource: repo://packages/web-core/src/shared/components/ui-new/containers/SharedAppLayout.tsx
  - id: openwiki-source-06199fac94fe2f7f1f9bd1b2
    resource: repo://packages/web-core/src/shared/dialogs/settings/settings/SpeechSynthesisSettingsSection.tsx
  - id: openwiki-source-a7576c2af3663c39efa0974d
    resource: repo://packages/web-core/src/shared/lib/speechSynthesis.ts
  - id: openwiki-source-6393181e558dceee5ef998cc
    resource: repo://tests/kanban/run-speech-lifetime.mjs
  - id: openwiki-source-2ed526da66878b98eebfeeff
    resource: repo://tests/saved-filters/navigation.spec.ts
  - id: openwiki-source-eece9c51da74bcdb3fe07f45
    resource: repo://tests/speech-synthesis/settings.spec.ts
generated: { by: "codex", at: "2026-11-24T02:54:42.940Z" }
---

# Kanban board

Kanban board は [Project](project.md) の [Issue](issue.md) を [Column](column.md) に並べる主画面である。仕事を選び、状態と順序を確認するときに使う。[操作の段階](../quickstart.md#操作の段階)

[Card](card.md) の移動は共有 Issue の状態や順位を変更する。表示 filter と実行状態は別であり、表示列だけから処理の完了や統合を判定しない。自動状態同期や archive の詳細は [Project と Issue](project-and-issue.md) にある。

Project の route は Kanban 画面を開き、Board API はその Project の Issue と status を取得する。[画面入口](../../packages/local-web/src/routes/_app.projects.$projectId.tsx)・[Board API](../../crates/server/src/routes/local_remote/board_api/mod.rs)

## 保存条件で仕事を絞り込む

[保存フィルター](saved-filters.md)はProjectで共有する条件を選び、現行のIssueから対象集合を求める。設定の「カンバンボードのフィルター」で管理し、ボードの選択欄または安定ID付きリンクから適用する。名前や条件の変更後も同じIDを参照する。

選択中は保存条件の結果を表示し、一時検索・Team/Personal・タグ・archiveタブ・blocked非表示では追加の絞り込みを行わない。結果に含まれるhidden列のIssueと子Issueも表示する。通常条件へ戻るには保存フィルターを解除する。表示件数は取得できた対象Issueに基づき、一括操作は表示集合に対応する選択に限る。[表示と選択](../../packages/web-core/src/features/kanban/ui/KanbanContainer.tsx)

未評価・初回評価中・失敗を完了した0件と区別し、前回結果を表示する場合は古い結果であることを示す。評価結果のIssue本体がまだ読み込まれていない間も確定一覧にしない。[状態表示](../../packages/web-core/src/shared/components/saved-filters/SavedFilterStatus.tsx)・[評価hook](../../packages/web-core/src/shared/hooks/useSavedFilter.ts)

保存フィルターの選択と解除は現在のProject内のWorkspace・Sessionを保ってURLを更新する。同じProjectのIssueへ移動するとフィルターを保持し、別Projectへの移動では持ち越さない。[AppNavigation](../../packages/local-web/src/app/navigation/AppNavigation.ts)

保存条件の作成・編集は共有定義の変更であり、選択そのものはIssue本文や関係の書換えではない。機械条件の検索と通常のボード検索では対象項目や関連展開が異なるため、[条件の意味](saved-filters.md#機械条件の意味)を確認する。保存フィルター選択は以下のProject単位の音声購読の対象を絞り込まない。

[ブラウザfixture](../../tests/saved-filters/integration.spec.ts)と[ナビゲーション試験](../../tests/saved-filters/navigation.spec.ts)は条件編集・状態・リンク・選択解除を扱う。fixtureは実providerや実サーバーの全画面受入を証明しない。

## Run の進捗を音声で聞く

音声切替は、複数の [Workspace](workspace.md) で動く [Run](agent-run.md) を個々の画面で見張らずに把握するための機能である。ボード右上のスピーカーをオンにすると、その Project に属する新着 Run イベントを Issue 番号付きで読む。表示中の Workspace だけに対象を絞らない。ボードを開いた時点と Project 切替時はオフで、オン／オフは保存しない。音声・速度・読み上げる内容の選好は別途ブラウザに保存する。[画面への接続](../../packages/web-core/src/features/kanban/ui/KanbanContainer.tsx)

1. `speechSynthesis` を利用できるブラウザで対象 Project のボードを開く。非対応の場合は切替が無効になる。
2. スピーカーを押す。新着の基準位置を取得する間は接続中となり、取得できてからオンになる。接続に失敗した場合はオフのままなので、接続状態を確認して再操作する。
3. 開始、成功・失敗・停止、回答待ちの短い定型文と、agent が人へ送った commentary を聞く。「結果と要対応だけ」を選んだ場合は開始と commentary を除く。ボード発話の言語は、解決した音声の正規化済み言語、ブラウザ優先言語、`ja-JP` の順で決まる。
4. オフにすると再生中の発話と待ち行列を即時に止める。再度オンにしても停止中の履歴は読み返さない。ボードを離れた場合も購読と発話を停止する。

[音声切替](../../packages/web-core/src/features/kanban/ui/RunSpeechToggle.tsx)は Provider の状態を表示する。購読と発話キューは Project ごとの [RunSpeechProvider](../../packages/web-core/src/features/kanban/ui/RunSpeechProvider.tsx) が所有し、[SharedAppLayout](../../packages/web-core/src/shared/components/ui-new/containers/SharedAppLayout.tsx) が画面内の再マウントより上に配置する。詳細の開閉、画面幅変更、loading、同一ボード内のページ再マウントでオン状態を失わない。別 Project、ボード離脱、新しいタブは別の寿命であり、新規ボードはオフになる。サーバーに音声キューや別モデルによる要約は設けない。

### 音声・速度・読み上げる内容を選ぶ

設定の「音声合成」は、ブラウザ優先言語に合う公開音声を実際の名前と言語で表示する。`navigator.languages`、続いて `navigator.language` から最初の有効な言語を選び、同じ基本言語の音声を地域・提供元をまたいで残す。アプリの表示言語で候補を絞らない。Androidなどの `ja_JP` は `ja-JP` に正規化する。特定ベンダーの音声が存在するとは仮定しない。

候補が複数なら「ブラウザの既定」と各音声を選べる。一件なら選択欄の代わりに音声名を表示し、自動使用する。ゼロ件なら候補がないことを表示し、ブラウザ既定へフォールバックする。一覧は `voiceschanged` による遅延読み込みと `languagechange` に追従する。速度の初期値は1.0×、読み上げる内容は「すべて」であり、速度は0.5〜2.0×を0.1刻みで選べる。[設定画面](../../packages/web-core/src/shared/dialogs/settings/settings/SpeechSynthesisSettingsSection.tsx)

選好は `speech-synthesis-preferences` として localStorage に保存する。保存音声が一時的に存在しなくても選好を消さない。一件の候補は保存値より優先し、複数候補では保存値と一致する音声を使う。それ以外はブラウザ既定へ戻す。音声は URI・名前・言語の組で識別する。別タブの変更も次の発話生成時に読み直し、現在の音声と速度を適用する。速度の体感はブラウザと音声エンジンに依存する。[保存と発話生成](../../packages/web-core/src/shared/lib/speechSynthesis.ts)

「結果と要対応だけ」は終了・失敗・停止・回答待ちを残し、開始・中間報告を除く。イベントの状態更新と重複排除は継続し、キューの排出時と非同期の有効性確認後の発話直前に最新モードを確認する。除外した待機分は破棄し、後で「すべて」に戻しても過去分を再生しない。読み始めた発話はモード切替では中断しない。[内容の選別](../../packages/web-core/src/features/kanban/model/runSpeechQueue.ts)

試聴とボードは共通の発話生成処理で音声・速度を解決する。音声もブラウザ優先言語も得られない場合だけ、試聴はアプリ表示言語、ボードは `ja-JP` を使う。「試聴」は固定文を現在の音声・速度でブラウザの発話列へ追加し、ボードをオンにしない。試聴や設定画面を閉じる操作は、ボードの発話を cancel せず、ボード内の待ち行列も破棄しない。内容モードにかかわらず試聴できる。非対応ブラウザでは試聴とボード切替を無効にする。[試聴](../../packages/web-core/src/shared/lib/speechSynthesis.ts)

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

[キュー試験](../../packages/web-core/src/features/kanban/model/runSpeechQueue.test.ts)、[接続試験](../../packages/web-core/src/features/kanban/model/runObservationPolling.test.ts)、[切替試験](../../packages/web-core/src/features/kanban/ui/RunSpeechToggle.test.tsx)、[音声設定ブラウザ試験](../../tests/speech-synthesis/settings.spec.ts)、[ボード寿命ブラウザ試験](../../tests/kanban/run-speech-lifetime.mjs)と API・adapter の試験は分類、順序、再接続、Project 分離、選好のライブ反映、旧イベント非再生と試聴の独立を扱う。ブラウザ fixture の音声APIはモックであり、実音声の聴取や利用者環境の音声品質を証明しない。
