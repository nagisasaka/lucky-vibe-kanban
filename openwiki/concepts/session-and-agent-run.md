---
type: concept
title: Session と Agent Run
description: 会話、実行依頼、試行と継続の境界。
tags: [session, agent-run, lifecycle]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-351583963abfe37bbf137f7e
    resource: repo://crates/executors/src/runtime/contracts.rs
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# Session・Agent Run・Attempt

**Session** は [Workspace](workspace.md) 内の会話単位で、同じ Workspace の別 Session は会話履歴を分けてもファイルを共有する。**Agent Run** は一つの実行依頼、**Run Attempt** はその実行試行であり、Session や Run を OS プロセスの寿命と同一視しない。provider の native session ID も LVK の Session ID とは別である。[操作の段階](../quickstart.md#操作の段階)を区別する。[Session](../../crates/db/src/models/session.rs)・[実行契約](../../crates/executors/src/runtime/contracts.rs)

依頼は Session、Run、turn、相関 ID、冪等キー、provider/profile、Workspace と入力を保持する。Attempt はさらに試行番号、transport、capability snapshot、設定、選択 Skill、provider session を固定する。検証はこれらの identity と実行環境の整合性を要求し、Review intent の selected Skills や Follow-up 以外の reset を拒否する。起動・永続化と投影の仕組みは [Agent Runtime](../architecture/agent-runtime.md) を参照する。

## 継続と再試行

Follow-up は同じ Session に新しい Run を作り、provider binding があればその会話を継続する。Retry は終端 Run の同じ turn に新しい attempt を作る。Resume は対応能力と provider session を必要とし、Restart はその参照を引き継がない。通常の follow-up は非終端 Run があると拒否されるため、実行中の入力には queue や steer の適切な経路を使う。応答不明時の冪等要求は同じ request ID と同じ入力で確認し、別 ID で重複起動しない。[Session 経路](../../crates/server/src/routes/sessions/mod.rs)・[実行 host](../../crates/local-deployment/src/agent_run_port.rs)

Session の実行設定は最新 attempt の保存済み launch request から復元する。保存値と provider が実際に解決した設定は別であり、壊れた最新設定を古い設定で黙って代用しない。provider を変える場合は同じ Workspace に新 Session を作り、目的と残件を渡す。native 会話は引き継がれない。[設定復元](../../crates/server/src/routes/sessions/executor_config.rs)・[provider](../integrations/agent-providers.md)

## 状態と観測

Run は pending、starting、running、入力・承認待ち、cancelling を経て succeeded、failed、cancelled、crashed または audit_failed に至る。終端状態と実プロセスが確実に終了した証拠は別である。表示の経過時間、通知や会話の末尾だけから成功や統合を推定しない。[状態型](../../crates/executors/src/runtime/contracts.rs)・[投影](../../crates/executors/src/runtime/reducer.rs)

Goal は objective、状態、予算と使用量を持ち、Run の成功とは別の寿命を持つ。会話中の質問・承認や非同期回答にはそれぞれ対象 Run と期限があり、後続 Run へ無条件に流用しない。実装と停止・回復の詳細は [Agent Runtime](../architecture/agent-runtime.md) を参照する。[Goal 契約](../../crates/executors/src/runtime/contracts.rs)・[質問](../../crates/server/src/routes/sessions/questions.rs)

新しい Session は Workspace root を作業位置とし、既存 Session の保存位置は移行で書き換えない。Git の author/committer は Session や provider の identity とは別である。[Session モデル](../../crates/db/src/models/session.rs)・[Git identity](../../crates/git/src/identity.rs)
