---
type: concept
title: 共有リソースの所有権と調停
description: 実機、DB、配備先を安全に共有するための操作と復旧。
tags: [resources, coordination, runner, recovery]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-22T05:31:45.518Z
sources:
  - id: openwiki-source-555b9ada58c3b29a4f83535e
    resource: repo://crates/db/src/models/resource_coordination.rs
generated: { by: "codex", at: "2026-11-22T05:31:45.518Z" }
---

# 共有リソースの所有権と調停

Worktree はソース編集を分離し、[正式 Integration](formal-integration.md) は選択した変更を統合する。共有資源調停はその間に使う実機、DB、配備先などの排他操作を扱う。Runner は実行機と installation の identity であり、Resource 自体でも所有権でもない。[要求型と保存](../../crates/db/src/models/resource_coordination.rs)・[Runner API](../../crates/server/src/routes/execution_bridge/mod.rs)

## 操作と所有権

同じ実体に一つの canonical key を使う。操作前に snapshot（`list_shared_resources` または `GET /api/resource-coordination/snapshot`）を読み、必要な全資源と観測 revision を一つの要求に含める。待機中は一部だけを占有せず、全 grant は原子的に得る。critical section は操作、cleanup、独立した idle／状態検証まで含む。安定した `request_id`、実行元 Session、目的、全 claims、script、verification、相対 working directory、timeout を指定する。秘密を目的や状態に書かない。[モデルと検査](../../crates/db/src/models/resource_coordination.rs)

要求の応答が不明なら同じ UUID と完全に同じ内容で再送し、operation と events を確認する。queued は完了ではない。成功時に宣言した変更後状態と revision を保存し解放する。古い revision の待機要求は互換性を再確認する必要がある。grant の fence と状態 revision は別で、外部対象が fence を検証しなければ任意の外部操作を防げない。[所有と完了](../../crates/db/src/models/resource_coordination.rs)・[API](../../crates/server/src/routes/resource_coordination/mod.rs)

調停は競合時の提案であり、現所有者を奪ったり前提や検証を免除したりしない。別 DB の LVK 間ではロックを共有せず、異なる key が同じ物理対象か自動発見しない。raw shell の迂回を OS レベルで防ぐ仕組みでもない。観測した負荷は予約や将来の安全の保証ではない。

## Runner と interactive 操作

選択した Runner を既存 execution-slot Resource に結び、desktop や device を含む必要資源を同じ bundle で取得する。Session／Workspace と Git HEAD に結び付く source snapshot は tracked と非 ignored untracked を含むため、送信前に秘密を確認する。危険な path、`.git`、実行時の共有領域、symlink 等を検査し、working directory は snapshot 相対とする。[source 検証](../../crates/server/src/routes/execution_bridge/source.rs)

Windows Runner はログイン済み user desktop から outbound 接続し、専用 account と保護した token、永続 data root を使う。HTTP は loopback、遠隔接続は HTTPS または SSH tunnel を用いる。heartbeat は過去の処理停止の証拠ではない。interactive operation は開始後も bundle を保持し、安定 UUID の step を逐次送り、receipt と artifact を確認する。最後に finish し、保存済み cleanup、子孫停止、独立 verification を完了する。各 step は別 PowerShell process で、変数は引き継がれない。[operator client](../../scripts/local-runner/control.py)・[Runner](../../scripts/local-runner/runner.py)・[Windows containment](../../scripts/local-runner/windows_job.py)

## 不明状態の復旧

切断、timeout、cancel、失敗、host 再起動は処理終了や解放の証拠ではない。実行中の cancel は停止要求であり、不確実なら `recovery_required` のまま所有する。TTL 自動解放や不明な副作用の自動再実行はしない。

DB と Runner data root 全体（identity、journal、結果、artifact）を保全し、同じ authority、Runner ID、installation と root で再接続する。別 root や clone は旧 installation の代用にならない。journal に受付済みで結果がない command は再実行せず、元の containment と子孫停止を確認して settlement する。disable は新規 dispatch を止めるが既存の cleanup と結果配送を許し、token rotation は所有権を解放しない。[Runner の journal](../../scripts/local-runner/runner.py)・[settlement guard](../../crates/server/src/routes/execution_bridge/mod.rs)

settlement の後も operator が外部実機・DB の状態を確認し、全 held resource の現在 revision、確認済み状態と停止の根拠を recovery に記録する。containment が未確認なら解放は拒否され、recovery は元の操作の成功を意味しない。installation/journal 喪失時の安易な force-unlock を前提にしない。[復旧](../../crates/db/src/models/resource_coordination.rs)・[外部実行の回復条件](../../crates/server/src/routes/execution_bridge/mod.rs)

通常の作業時には配置された on-demand guide に操作例と認証の詳細がある。local stdio MCP は内部 capability を使い、raw loopback HTTP の内部 token は同じ data directory から読み、URL やログへ出さない。[ガイド](../../crates/utils/src/agent_guides.rs)・[client](../../scripts/local-runner/control.py)
