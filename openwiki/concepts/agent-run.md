---
type: concept
title: Agent Run
description: Session 内の実行依頼と試行、状態の関係。
tags: [agent-run, session, lifecycle]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-24T14:16:53.361Z
sources:
  - id: openwiki-source-d20a82e2192a07c687b838cb
    resource: repo://crates/db/src/models/agent_runtime.rs
  - id: openwiki-source-379c8cd3e1e9932f9adda9ee
    resource: repo://crates/services/src/services/container.rs
  - id: openwiki-source-f2fdb07edb2a73ea40538a98
    resource: repo://packages/web-core/src/shared/components/ModelSelectorContainer.tsx
  - id: openwiki-source-bfcca8eb0acec96215c9bf83
    resource: repo://packages/web-core/src/shared/lib/executorOverrides.ts
  - id: openwiki-source-f66eb76c24833219ccd50c84
    resource: repo://packages/web-core/src/shared/lib/modelSelector.ts
  - id: openwiki-source-540291fed63f1f0f52cb1e86
    resource: repo://tests/run-selection/demand.spec.ts
generated: { by: "codex", at: "2026-11-24T14:16:53.361Z" }
---

# Agent Run

Agent Run は [Session](session.md) 内の一つの実行依頼である。実行を追跡し、再試行や終了を判断するときに読む。Run と実プロセスの寿命は同じではない。[操作の段階](../quickstart.md#操作の段階)

Run の各試行は Attempt として識別される。Retry は同じ Run の新しい試行であり、Follow-up は同じ Session の新しい Run である。Resume と Restart の違いや要求 ID の扱いは [Session と Agent Run](session-and-agent-run.md) を参照する。

Run は待機、起動、実行、入力・承認待ち、取消を経て終端状態に至る。終端表示だけでは実プロセスの終了や成果の統合を証明しない。永続イベント、監査と回復の経路は [Agent Runtime](../architecture/agent-runtime.md) にある。

[状態契約](../../crates/executors/src/runtime/contracts.rs)・[永続モデル](../../crates/db/src/models/agent_runtime.rs)

## 次の Run の設定を読む・変更する

入力欄のモデル・推論深さ・Codex の承認表示は、継承した値と今回の明示上書きを分ける。継承値は通常色、継承値と異なる明示上書きはその項目だけアクセント色で表示する。ツールチップとアクセシブル名にも出典を付ける。表示した値を見ただけでは Run の明示 override に変換しない。[入力欄](../../packages/web-core/src/shared/components/ModelSelectorContainer.tsx)・[送信値の解決](../../packages/web-core/src/shared/lib/executorOverrides.ts)

| 表示 | 意味 |
| --- | --- |
| 継承: プリセットの明示設定 | 選択した profile の値を表示している |
| 継承: 設定済みの値／プロバイダーの既定値 | 読み取りで解決した次 Run の設定または provider 既定値であり、実行後の観測値ではない |
| 明示上書き | その項目について継承値と異なる値を選んでいる |
| 未確認 | 安全に値を解決できていない。候補一覧から実行値を推測しない |
| 非対応 | provider が推論深さ非対応と確認したモデル。入力欄の推論深さ表示を隠す。候補の取得失敗や空配列だけでは非対応としない |

継承値と同じ値を選ぶと、その項目の override を外す。「上書きを解除」は上書きがある場合だけ表示する。モデルを別のものへ変えると古い推論深さを解除する。同じ実効モデルの冗長な override だけを外す場合は、独立した明示の推論深さを保持する。[選択処理](../../packages/web-core/src/shared/components/ModelSelectorContainer.tsx)・[変更の合成](../../packages/web-core/src/shared/lib/executorOverrides.ts)

再開 Session でも discovery が解決した次 Run のモデル・推論深さを表示する。承認は再開後の実値を確認していないため、discovery の承認値を隠す。解決できない項目は未確認のまま表示し、他の取得済み項目を消さない。空の native 設定だけを理由にモデルの設定を必須にするものではない。Codex と Claude の解決方法や承認の境界は [provider の設定](../integrations/agent-providers.md#run-の継承値と承認)を参照する。

Claude で継承モデルと異なるモデルを明示選択すると、そのモデルだけを追加照会する。候補一覧を開くだけでは候補ごとの照会を始めない。取得中は未解決の推論深さに loading を表示し、失敗時は未確認とする。解除や別モデルへの変更後に旧応答が到着しても、現在の選択へ適用しない。[選択時照会の回帰](../../tests/run-selection/demand.spec.ts)

[Run-selection ブラウザ fixture](../../tests/run-selection/selection.spec.ts)と[選択単体試験](../../packages/web-core/src/shared/lib/modelSelector.test.ts)は表示・送信分離、同値解除、モデル変更、未確認と非対応を検証する。fixture は実 provider の応答や通常／Goal の end-to-end 成功を証明しない。
