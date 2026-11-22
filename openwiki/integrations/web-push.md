---
type: integration
title: Web Push と通知
description: browser ごとの同意、配送、失敗と会話への復帰。
tags: [notifications, web-push]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-22T05:31:45.518Z
sources:
  - id: openwiki-source-2f63ca1582805a46da98d522
    resource: repo://crates/db/migrations/20261018180000_web_push.sql
generated: { by: "codex", at: "2026-11-22T05:31:45.518Z" }
---

# Web Push と通知

Web Push は認証された利用者が Project と browser ごとに登録し、Codex の blocking question、承認待ち、Run 終端から会話に戻る任意機能である。通知は [Agent Run](../concepts/session-and-agent-run.md) の状態そのものではなく、到達を回答、承認、検証、統合や公開の成功と扱わない。[trigger](../../crates/db/migrations/20261018180000_web_push.sql)・[配送](../../crates/server/src/routes/web_push.rs)

## 設定と同意

server は永続的な P-256 VAPID 秘密鍵の path を `LVK_WEB_PUSH_PRIVATE_KEY_FILE`、連絡先を `LVK_WEB_PUSH_SUBJECT` に設定する。HTTPS と outbound HTTPS が必要で、不正・欠損なら登録できない。鍵は秘密として保護し、rotation 後は browser を再登録する。Settings → 通知で Project と browser を登録し、browser／OS の許可を与える。登録解除とイベントを全て OFF にする設定は別である。[設定と登録](../../crates/server/src/routes/web_push.rs)・[画面](../../packages/web-core/src/shared/components/web-push/WebPushSettings.tsx)

DB の封印された subscription と server-access 鍵は整合した組で保全する。OFF は待機中の配送を停止するが、既に push service や OS が受け取った通知は撤回できない。Web Push の browser 別設定と server 端末の OS 通知設定は別である。[server access](../concepts/server-access.md)・[OS 通知](../../crates/services/src/services/notification.rs)

## 配送と復帰

canonical event から outbox を作り、送信前に現在の待機状態、最新 attempt、同意と event mask を再確認する。期限切れ endpoint は削除し、network error や一時エラーは限度付きで再試行する。応答喪失のため exactly-once は保証しない。lock screen は汎用文面で、質問や command の本文を露出しない。[配送処理](../../crates/server/src/routes/web_push.rs)

通知クリックは同一 origin の画面を開き、認証を経て対象 Session／Run へ戻る。実 browser／OS で許可、配送、解除、鍵変更後の再登録を確認する必要があり、fixture の成功だけで実配送を保証しない。[service worker](../../packages/public/lvk-push-sw.js)
