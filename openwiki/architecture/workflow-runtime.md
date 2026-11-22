---
type: architecture
title: Workflow の実行と耐久オーケストレーション
description: 凍結グラフから Agent Run を配送し、待機、再起動、分岐と取消を扱う実行境界。
tags: [workflow, orchestration, recovery, lifecycle]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-22T05:31:45.518Z
sources:
  - id: openwiki-source-28ba6f0a94db33f3977b34d2
    resource: repo://crates/server/src/workflow_runtime/presets.rs
  - id: openwiki-source-424fb004ecd8c2fe0a368131
    resource: repo://crates/server/src/workflow_runtime/workspace.rs
  - id: openwiki-source-b19d02455156008b43c210c8
    resource: repo://crates/services/src/services/orchestration.rs
generated: { by: "codex", at: "2026-11-22T05:31:45.518Z" }
---

# Workflow の実行と耐久オーケストレーション

Workflow runtime は [Workflow Attempt](../concepts/workflow-attempt.md) のグラフを実行し、各工程と Agent Run の状態を結び付ける。グラフの意味、実際の副作用、製品固有の成果検証を変更するときに読む。受付、実行、完了、公開、内容受入は[別の段階](../quickstart.md#操作の段階)である。

## 計画と作業場所

`workflow` crate はグラフを検証し、実行可能な node を計画する。server runner は Workspace、Session、node execution と Agent Run を接続する。開始時の graph snapshot は Run に保存され、回復時に現在のテンプレートへ無条件で置き換えない。[planner](../../crates/workflow/src/planner.rs)・[runner](../../crates/server/src/workflow_runtime/runner.rs)

Workspace row の存在は物理 worktree の準備完了を意味しない。通常の Agent dispatch は Session の所属を確認し、予約 guard を通して作業場所を準備する。`execution_only` は所有者が準備した inspection root を使い、欠落時に通常 container を勝手に再作成しない。閲覧や graph 編集は作業木作成の契機ではない。[作業場所](../../crates/server/src/workflow_runtime/workspace.rs)・[Workspace の所有権](../concepts/workspace.md)

新規 Workflow Workspace の Repo は Project の作成プリセットと base branch を検査する。既存 Workspace の再利用はその構成を保つ。複数 Repo への適用を一つの原子的 rollback とみなさない。[resolver](../../crates/server/src/workflow_runtime/workspace.rs)・[Arena](../../crates/server/src/workflow_runtime/arena.rs)

## 耐久配送と回復

| 記録 | 責任 |
| --- | --- |
| Plan snapshot と node identity | dispatch 前に Run と工程を固定する |
| Outbox | Create、Retry、Cancel 等の command を耐久化する |
| Inbox と consumption | canonical event と下流による消費を識別する |
| Lease | dispatcher の一時的な処理所有権を表す |

[永続モデル](../../crates/db/src/models/orchestration.rs)と[サービス](../../crates/services/src/services/orchestration.rs)は配送の再試行と重複防止を扱う。Inbox の join は製品 planner の辺の意味そのものではなく、実際の Ready 判定は Workflow 側にある。

Create は outbox の配送済み表示だけで受理としない。親の状態、Run・node・Agent と冪等性の link、port query の Session・turn identity を照合する。受理された Agent Run が Pending でも Started を返し得るため、provider の準備完了や成果成立の証拠ではない。[受理確認](../../crates/services/src/services/orchestration.rs)・[配送競合テスト](../../crates/services/src/services/orchestration_dispatch_tests.rs)

Watcher や lease が失われただけでは実行を失敗や置換にしない。起動時は配送中 outbox、処理中 inbox、lease を再照合し、子状態の回復後に inbox を replay する。実プロセスの再接続は [Agent Runtime](agent-runtime.md) の責任である。[startup reconciliation](../../crates/services/src/services/orchestration.rs)

Driver の Pause はサーバー thread を待機させる意味ではなく、次の event 等による照合まで駆動を返す。Standalone と library 起動の completion watcher が終端を追い、Run GET や SSE 接続も reconciliation を行う。画面の活動中 GET は既定4秒周期だが、UI を開いていることが後続開始の前提ではない。[watcher](../../crates/server/src/workflow_runtime/runner.rs)・[画面取得](../../packages/web-core/src/shared/hooks/useWorkflowRun.ts)・[起動順](system.md#起動と回復)

## 成果、分岐、取消

固定 preset は同じ Workspace に工程ごとの Session を持つ。実装工程の引継ぎは通常 coding 完了と clean source の確立を待ち、report、固定 SHA と Change Manifest 参照を渡す。空 report や source 不成立は失敗であり、publication 中は同じ実行の完了を待つ。[preset 完了](../../crates/server/src/workflow_runtime/presets.rs)・[固定 graph](../../crates/workflow/src/presets.rs)

報告工程は事前の source 保護と開始・終了 snapshot の照合を使う。後者だけを事前保護の代わりにしない。多角調査の合流は必要な全入力の成功を待ち、一部失敗なら結果を保持して後続を開始しない。[planner](../../crates/workflow/src/planner.rs)・[provider の設定](../integrations/agent-providers.md)

通常 Condition は候補と上流情報に基づく decision block を検証する。形式不正、候補不一致、確信不足、作業木変更などでは人の判断を待ち、推測した分岐で進めない。read-only を求める prompt 自体は OS 隔離の証明ではない。[router](../../crates/server/src/workflow_runtime/condition_router.rs)・[runner](../../crates/server/src/workflow_runtime/runner.rs)

取消は親を Cancelling にし、子への取消と照合を続ける。配送時にも親状態を再確認し、取消後に残った Create/Retry は新しい Agent Run を起動せず、Cancel は配送可能に保つ。[配送 guard](../../crates/services/src/services/orchestration.rs)

## 製品所有の実行

Repository scope の OpenWiki Bootstrap は専用 Workspace の owner と Workflow Run を結び付け、phase ごとの Session と子 Run を記録する。provider の成功と製品の validation・公開は別の結果であり、同じ Workspace にいるだけで起動権限が得られるわけではない。[bootstrap](../../crates/server/src/workflow_runtime/bootstrap.rs)・[Wiki 保守](../operations/openwiki-maintenance.md)

Bootstrap の Review は host が JSON と evidence を検証し、完全な report を管理領域へ保存する。下流には identity、digest、判定と件数の参照を渡し、再読込時に照合する。これは任意ファイルを prompt に挿入する汎用機構ではない。[report](../../crates/services/src/services/openwiki/bootstrap/reports.rs)

[Auto-merge](../concepts/formal-integration.md) は製品固有の検証と monitor を持つ。通常 Workflow watcher や Wiki Bootstrap と同一の lifecycle として扱わない。

Outbox・lease の回復、重複配送、親取消、router の不正出力、製品 validation はそれぞれ別の境界で試験する。隔離 DB や mock の試験は実 provider での全 preset 完走を証明しない。[配送試験](../../crates/services/src/services/orchestration_dispatch_tests.rs)・[router tests](../../crates/server/src/workflow_runtime/condition_router.rs)
