---
type: integration
title: Web Push・通知同意・ChatGPT 相談
description: Codex の待機・Run 終端を browser に通知する同意、原子的 outbox、配送失敗、認証復帰と明示的な ChatGPT 相談の契約。
tags: [web-push, notifications, consent, codex, lifecycle, authentication]
verified:
  - by: openwiki/0.5.1
    at: 2026-10-20T17:16:45.724Z
sources:
  - id: openwiki-source-2f63ca1582805a46da98d522
    resource: repo://crates/db/migrations/20261018180000_web_push.sql
  - id: openwiki-source-7d94e7454f63f16057833046
    resource: repo://crates/server-access/src/lib.rs
  - id: openwiki-source-fb97b52f99c4e6667bfbcfa7
    resource: repo://crates/server/src/routes/web_push.rs
  - id: openwiki-source-67674da85aa6dbcd2c2a493d
    resource: repo://docs/development/web-push.md
  - id: openwiki-source-555aad5cd76b2126b533c7d0
    resource: repo://packages/public/lvk-push-sw.js
  - id: openwiki-source-f0616990372eebb94bc69e6f
    resource: repo://packages/web-core/src/features/workspace-chat/ui/SessionChatBoxContainer.tsx
  - id: openwiki-source-aa003fdafd77c971b217ad41
    resource: repo://packages/web-core/src/shared/components/web-push/ConsultChatGPT.tsx
  - id: openwiki-source-9cea28cc3d20cd3fcde94697
    resource: repo://packages/web-core/src/shared/components/web-push/WebPushSettings.tsx
generated: { by: "codex", at: "2026-10-20T17:16:45.724Z" }
---

# Web Push・通知同意・ChatGPT 相談

Web Push は、認証された利用者が Project ごとに browser を登録し、Codex の blocking question / execution approval / Run 終端へ戻るための任意機能である。[Issue Follower や OS notifier](../concepts/issue-collaboration.md)とは別で、タブの会話 stream が通知の生成主体ではない。状態の正本は [AgentRun](../concepts/session-and-agent-run.md)にあり、通知到達は人の回答・承認・テスト・Integration・公開の成功を意味しない。[trigger](../../crates/db/migrations/20261018180000_web_push.sql)・[固定通知本文](../../crates/server/src/routes/web_push.rs#L308-L317)

## 通知同意と所有権

Settings → General の Web Push は secure context、Service Worker / Push / Notification API と利用者の許可を要求する。ON は Project と endpoint を登録する。owner は入力値ではなく local/basic/web の認証 actor から取り、内部 agent / MCP channel は拒否する。既存 local Board の active member は全 Project を利用し、通知だけに private Project ACL を設けない。[browser 登録](../../packages/web-core/src/shared/components/web-push/WebPushSettings.tsx#L36-L104)・[actor](../../crates/server/src/routes/web_push.rs#L83-L93)

endpoint と暗号鍵は既存 server-access 鍵で封印し、DB には検索用 endpoint hash を別に置く。登録は owner・Project・browser に属し、同じ endpoint を別 owner へ黙って移せない。owner 当たり64登録が上限で、同じ登録の更新は可能。DB と access key は整合した組で保全する。[登録 transaction](../../crates/server/src/routes/web_push.rs#L187-L254)・[保存 schema](../../crates/db/migrations/20261018180000_web_push.sql#L1-L25)

同意 envelope は owner・用途・認証設定 revision を固定し、配送時に membership / revision を再検証する。browser の idle/absolute expiry や logout を越えて残すのは、夜間の仕事も通知するためという記録された設計である。API/login credential ではなく、詳細表示は引き続き認証を要する。[認証境界](../concepts/server-access.md#失効は接続と受理済み作業で異なる)・[seal/open](../../crates/server-access/src/lib.rs#L394-L470)

OFF は owner の登録と cascade で配送待ちを消し、一回の有界 send と同じ gate を使う。OFF 応答時に該当 send は残らないが、既に push service / OS が受け取った通知は撤回できない。現在 browser の最後の登録を削除したら client も unsubscribe する。90日で同意は期限切れになり、member 無効化・設定変更でも配送を失効させる。account 切替や VAPID key 変更では UI が既存 subscription を置き換える。[OFF](../../crates/server/src/routes/web_push.rs#L256-L270)・[dispatcher](../../crates/server/src/routes/web_push.rs#L342-L385)・[browser 更新・削除](../../packages/web-core/src/shared/components/web-push/WebPushSettings.tsx#L76-L130)

## Canonical 状態から配送待ちへ

SQLite trigger は canonical `input_requested` / `approval_requested` の INSERT と、Codex AgentRun の succeeded / failed / cancelled / crashed / audit_failed への状態変化を同じ transaction 内で outbox に入れる。Workspace が登録 Project に link されている browser ごとに配送を作り、登録前の履歴を一括再送しない。async_questions、assistant prose、provider turn 完了、script/test、Integration/publication は trigger ではない。[admission](../../crates/db/migrations/20261018180000_web_push.sql#L26-L49)

control は attempt・種別・reference、terminal は Run・attempt と browser 登録で重複排除する。worker は送信前に共通 `pending_blocking` と照合し、回答済み・期限切れ・再試行で古くなった待機を捨てる。terminal も現在 status と最新 attempt を照合する。通知用に新しい質問 resolver や approval control を作らない。[actionable 判定](../../crates/server/src/routes/web_push.rs#L289-L307)。特に [現在の Codex Noop 承認配線](../concepts/session-and-agent-run.md#承認質問と現在の接続状態)の制限を、この通知機能が解消したと解釈しない。

## 配送と失敗の境界

共通 router が worker を起動する。ネットワーク request は15秒 timeout、redirect 無効。送信先は HTTPS の Google FCM（`/wp/` と `/fcm/send/`）、Mozilla autopush、Apple、Microsoft notify の明示 allowlist だけであり、任意 relay URL を受け付けない。新しい vendor 対応はコード変更の境界となる。[endpoint 検証](../../crates/server/src/routes/web_push.rs#L36-L67)・[worker](../../crates/server/src/routes/web_push.rs#L95-L127)

| 条件 | 結果 |
| --- | --- |
| network error / 429 / 5xx | 最大5回、30–480秒の指数 backoff |
| send 前の予約後に restart | 120秒の予約期限を待って再試行可能 |
| 404 / 410 | 期限切れ endpoint の登録を削除 |
| その他の2xx–4xx | 成功または恒久失敗として job を終了（redirect は追わない） |
| job 作成から1時間経過 | 終了扱いとし、長期停止後の通知を再生しない |
| 保存から7日経過 | 配送 receipt を削除 |

一回に最大32 jobを読み、設定ありは5秒周期、なしは60秒周期で維持処理する。push TTL は5分。予約を network I/O より前に保存するが、service 受付後の応答消失は区別できないため exactly-once ではない。固定 Topic / tag と Service Worker receipt が再送の重複を抑える。[配送処理](../../crates/server/src/routes/web_push.rs#L319-L430)

lock screen 本文は LVK・イベント分類・汎用案内のみで、Issue title・質問・command・例外本文を含めない。IDs と相対 navigation URL は暗号化 payload 内に置く。endpoint を含み得る network error 自体はログへ出さない。[payload](../../crates/server/src/routes/web_push.rs#L398-L407)・[警告](../../crates/server/src/routes/web_push.rs#L107-L117)

## 通知から会話へ戻る

root scope の `/lvk-push-sw.js` に fetch handler はなく、認証済み画面/APIを cache しない。receipt cache は直近256件を保持し、browser storage 消去で失われる。通知クリックは同一 origin の参照先と完全一致する既存 tab を focus し、なければ開く。[Service Worker](../../packages/public/lvk-push-sw.js)

URL の Session / Run 参照は [認証 gate の復帰処理](../architecture/web-and-sync.md#認証が製品画面より先に確定する)で login を越えて引き継ぐ。composer は通知対象 Run を示し、新しい Run と混同しない。見つからなければ Session ID と履歴の確認を促す。[通知対象表示](../../packages/web-core/src/features/workspace-chat/ui/SessionChatBoxContainer.tsx#L1535-L1570)

ChatGPT 相談は IDs・参照 URL を依頼文としてコピーし、通常の ChatGPT URL を開いて利用者が貼り付ける。clipboard 失敗時は選択可能な textarea を使う。質問本文・code・logs をコピーせず、自動送信・回答・承認・merge は行わない。Project link の取得失敗時も Workspace ID からの確認を案内する。[相談 UI](../../packages/web-core/src/shared/components/web-push/ConsultChatGPT.tsx#L14-L124)。[外部 MCP の認証・scope](remote-access.md#認証付き外部-mcp)は別途必要で、保存済み progress は [list_run_events](remote-access.md#保存済み-codex-進捗の読取)から読む。Web Push はこの read tool の実装に依存しない。

## 設定と検証限界

server は `LVK_WEB_PUSH_PRIVATE_KEY_FILE` の PEM と `LVK_WEB_PUSH_SUBJECT`（mailto または HTTPS）から VAPID を構成する。欠損・不正設定では登録が unavailable になり、配送を行わない。公開鍵だけを browser へ渡し、秘密鍵を restart 後も維持する。key rotation 後は browser で再登録する。[設定読取](../../crates/server/src/routes/web_push.rs#L68-L81)・[運用手順](../../docs/development/web-push.md#server-configuration)

変更時は trigger の rollback/dedup、Project fanout、古い待機・retry、owner/CSRF と同意失効、Service Worker の URL/重複、相談文と login 復帰のテストを照合する。[server tests](../../crates/server/src/routes/web_push.rs#L546-L683)・[consent tests](../../crates/server-access/src/tests.rs#L1803-L1886)・[browser tests](../../tests/web-push/service-worker.test.mjs)・[相談と復帰 tests](../../packages/web-core/src/shared/components/web-push/pushReturn.test.ts)

実 OS/browser の配送、HTTPS deployment、実 ChatGPT account の接続は fixture や source 調査では確定しない。OS の休止・通知権限・network policy により遅延や未着があり、完全停止した browser の起動を保証しない。[手動受入項目](../../docs/development/web-push.md#manual-acceptance-checks)を別に実施する。実装記録の過去 test 件数を、この統合 checkout の acceptance 成功と読み替えない。
