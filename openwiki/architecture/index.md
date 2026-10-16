# ファイル

- [Agent Runtime の監査・投影・回復](agent-runtime.md) - AgentRun を独立プロセスで実行し、Native Audit から永続状態と画面表示へ変換する境界。再接続、DB 競合、投影劣化時の契約を説明する。
- [システム境界と起動順序](system.md) - ローカル実行基盤、Web、Git と永続化の所有境界、外部接続と二つの起動経路の回復順序。
- [Web の実行環境と同期](web-and-sync.md) - Local shell と共有画面の責任、ローカル snapshot と AgentRun stream の異なる同期契約。
- [Workflow の実行と耐久オーケストレーション](workflow-runtime.md) - 凍結グラフから AgentRun を起動する経路、outbox・inbox・lease の役割、分岐検証と取消・再起動時の所有境界。
