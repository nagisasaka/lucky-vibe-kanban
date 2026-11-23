---
type: architecture
title: Agent Runtime
description: Agent Run の監査、投影と回復の責任を説明する。
tags: [agent-runtime, audit, persistence, recovery]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T19:12:52.102Z
sources:
  - id: openwiki-source-d20a82e2192a07c687b838cb
    resource: repo://crates/db/src/models/agent_runtime.rs
  - id: openwiki-source-c88a7c6fabdac15166554032
    resource: repo://crates/executors/src/executors/codex/client.rs
generated: { by: "codex", at: "2026-11-23T19:12:52.102Z" }
---

# Agent Runtime の監査・投影・回復

Agent Runtime は [Session 内の Agent Run](../concepts/session-and-agent-run.md) を実行し、その観測を製品の状態へ変換する。実行中の接続断、画面表示、プロセスの終了は別の事実である。実行や回復の変更時にこのページを読む。操作の受付から内容受入までの区別は[クイックスタート](../quickstart.md#操作の段階)を参照する。

## 起動と所有者

`LocalAgentRunPort` は Workspace を検証し、Run と Attempt の予約を永続化する。Setup 待ちでは予約と起動が分かれる。Starting の保存が失敗した場合、provider を先に起動せず耐久状態を再照合する。既存 host の予約が見つかった場合は起動応答の喪失もあり得るため、その host の観測へ戻る。[起動](../../crates/local-deployment/src/agent_run_port.rs)

`agent-process-host` は一つの provider process と Native Audit を所有する独立ホストである。アプリは認証付き loopback に再接続し、保存済み cursor 以降を取り込む。サーバーとの接続断だけでは provider の終了を証明できない。[process host](../../crates/local-deployment/src/process_host.rs)

未起動を確認した予約の `not_started` と、実プロセスの `exited` は別である。terminal 投影時、host identity、PID、cursor、監査などが存在しない予約だけが同じ transaction で `not_started` になる。PID が空という条件だけでは不十分であり、古い terminal 予約を migration が自動修復するわけでもない。[予約制約と trigger](../../crates/db/migrations/20261027010000_unstarted_reservation_terminal.sql)

## 監査と表示の経路

| 経路 | 用途と寿命 |
| --- | --- |
| Native Audit | provider との生の入出力と manifest。再解釈や終了証拠の原資料 |
| Host journal | attempt ごとの意味的イベントと snapshot の再接続用記録。raw delta の複製ではない |
| Canonical | SQLite の event と状態投影。製品が状態や完了を判断するための永続記録 |
| Live | 一時的な表示差分。再接続時に全差分が再送される保証はない |
| Snapshot | usage などの最新値。通知回数をそのまま累積しない |

Host journal は checksum、attempt と host の identity、連番を検証する。保存済みの cursor のみを公開し、配信と一時表示に上限を設ける。journal の offset 索引は実行に伴って増えるので、全メモリが一定という保証ではない。[journal](../../crates/local-deployment/src/process_host/journal.rs)

DB は host batch の identity を照合し、event、provider session、投影、usage と cursor を transaction 内で更新する。後続の照合や保存に失敗すれば cursor も rollback する。Board の同期も適用された canonical event の transaction に属し、OS の exit を別途 Board 更新に変えてはならない。[永続化](../../crates/db/src/models/agent_runtime.rs)・[Board 投影](../../crates/db/src/models/agent_runtime_kanban.rs)

監査の本体は SQLite 外の asset directory にある。`manifest.json` と `frames.jsonl` は本文を含む生の証拠であり、DB だけのバックアップでは保全できない。エクスポートや共有前に内容とアクセス権を確認する。Workspace 削除は関連する監査の cleanup を伴うため、必要な証拠は削除前に保全する。[保存先](../../crates/utils/src/native_audit.rs)・[bundle 検証](../../crates/executors/src/runtime/native_audit.rs)・[削除](../../crates/workspace-manager/src/workspace_manager.rs)

## 失敗、取消、終端

一時的な DB 競合と、決定的な投影失敗は別に扱う。後者は失敗観測、`projection_degraded`、cursor を原子的に記録し、後続の観測を取り込める。劣化を自動修復の成功と扱わない。限定された startup collision の明示 Retry は、終了済み process と完全な journal・監査などの証拠を確認してから再構築する。[失敗台帳](../../crates/db/src/models/agent_runtime.rs)・[回復](../../crates/local-deployment/src/agent_run_port.rs)

初回 launch の入力は起動前に監査へ記録する。一方、活動中の制御は provider に送った後で送信 bytes を監査へ追加する。したがって、監査失敗やフレーム不在は「未送信」や「副作用なし」の証拠にならない。再試行前に実行と作業木の状態を確認する。[制御送信](../../crates/local-deployment/src/process_host.rs)

取消は native interrupt と実プロセスの cleanup を伴う。Linux では所有 identity と捕捉した子を照合して停止を確認する。捕捉前に逃れた daemon や強制的な runtime 破棄までの完全な回収保証ではない。[process cleanup](../../crates/utils/src/process/owned_group.rs)・[検証範囲](../../tests/codex-process-cleanup/README.md)

Provider の turn 完了、OS exit、Host Terminal は同じではない。Host は cleanup 後に出力を drain し、監査を閉じて terminal を記録する。exit 0 だけから provider の成功を作らず、後着の成功で取消、監査失敗や非ゼロ終了を上書きしない。通信失敗だけで死亡と決めたり、代替 provider を勝手に起動したりしない。[終端処理](../../crates/local-deployment/src/process_host.rs)

### Codex の子エージェントと親の完了

親の turn や Goal が完了しても、子の終了証拠が不足する場合は Run を成功にしない。Codex client は表示用の投影とは別に子の thread と turn を追跡する。新しい turn の開始で終了証拠を無効にし、別の古い turn の完了では新しい turn を終了扱いにしない。[終了ガード](../../crates/executors/src/executors/codex/client.rs)

`wait` は子を観測する操作である。V1 の grouped wait が A/B を receiver に列挙し、結果には B の状態しか返さなくても、既に終了した A の証拠を保つ。初めて見る子は未確定として登録する。明示された running・未知状態は以前の終了証拠より優先し、wait の呼出し自体の完了を子の完了とは扱わない。

`resumeAgent`、`sendInput`、`spawnAgent`、未知・欠損 tool は子を再稼働させ得るため、receiver の古い終了証拠を無効にする。V2 の `subAgentActivity` でも開始・対話は終了証明にならない。receiver/state が空の mailbox wait は、既知の子を終了させることも再稼働扱いへ戻すこともしない。[観測と表示](../../crates/executors/src/executors/codex/agent_scope.rs)

注意点: 子の終了確認と、その成果が正しいかは別である。この修正は終了ガードを弱めず、観測のみの wait で既存証拠を失う経路を直す。過去の失敗 Run の実原因を確定したり、既存 Run を書き換えたりするものではない。

## 承認と指示配送

承認や質問の waiter は provider を所有する host に置く。回答受付、元の native request への配送、実際の操作結果は別の証拠である。取消や接続終了では待機を解放する。[承認](../../crates/local-deployment/src/process_host/approvals.rs)・[Codex client](../../crates/executors/src/executors/codex/client.rs)

通常起動は Run と Repository の identity を使い、必要時の詳細ガイドを準備する。継続時の固定指示は Session ごとの受付 receipt と契約の一致により省略される場合があり、不明時や契約変更時は全文へ戻る。CLI の受付はモデルが内容を読んで従った証明ではない。[環境](../../crates/executors/src/env.rs)・[指示配送](../../crates/executors/src/instruction_delivery.rs)・[ガイド](../../crates/utils/src/agent_guides.rs)

Codex と Claude の mapper は native frame を canonical event に変換し、過去の mapper version を replay のために区別する。質問、承認、usage、最終メッセージを一つの会話本文へ潰さない。Provider 固有の設定と利用可能性は[provider の設定](../integrations/agent-providers.md)を参照する。[adapter](../../crates/executors/src/executors/provider_adapter.rs)

## 検証の入口

DB の batch・reservation 回帰、process host の drain と terminal の回帰、Linux の所有 process cleanup を変更範囲に応じて確認する。これらの fixture は実 provider、全 OS、サーバー強制終了を越える回復の受入を証明しない。[DB tests](../../crates/db/src/models/agent_runtime_reservation_tests.rs)・[host tests](../../crates/local-deployment/src/process_host.rs)

Codex client の `version_check_tests` は V1 grouped wait、V2 mailbox wait、再開・明示非終端・古い turn 完了を合成通知で検証する。実 Codex の fan-out → wait → 親完了と LVK の成功・成果保存は、候補の server/host を隔離 instance で動かして別に照合する。合成試験だけでは実モデルの動作を証明しない。[対象回帰](../../crates/executors/src/executors/codex/client.rs)・[隔離手順](../../deploy/developer/STAGING.md)
