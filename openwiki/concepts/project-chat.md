---
type: concept
title: Astra のプロジェクト会話
description: 永続会話、送信ごとの権限上限、Codex ChatGPT 推論と LVK の操作 receipt の境界。
tags: [astra, project, conversation, authorization, recovery]
verified:
  - by: openwiki/0.5.1
    at: 2026-10-16T11:50:57.361Z
sources:
  - id: openwiki-source-278e88bfb0c3bdf2a2bd8662
    resource: repo://crates/server/src/routes/project_chat/mod.rs
  - id: openwiki-source-0f681e0f4222c5e8c1d6dd48
    resource: repo://crates/server/src/routes/project_chat/model.rs
  - id: openwiki-source-113eb4ece03270cd6eee1479
    resource: repo://crates/server/src/routes/project_chat/model/codex.rs
  - id: openwiki-source-e85c722eb17c6ac827c791a4
    resource: repo://crates/server/src/routes/project_chat/settings.rs
  - id: openwiki-source-b35093d0057e18ad85394a51
    resource: repo://crates/server/src/routes/project_chat/store.rs
  - id: openwiki-source-7c7954a1a4ada7c9b5b72849
    resource: repo://crates/server/src/routes/project_chat/tests.rs
  - id: openwiki-source-424a0051ff596878bb2d8a34
    resource: repo://crates/server/src/routes/project_chat/tools.rs
generated: { by: "codex", at: "2026-10-16T11:50:57.361Z" }
---

# Astra のプロジェクト会話

Astra は Project 内で相談を続け、明示依頼を通常の Issue・Workspace・Session・正式 Integration に渡す LVK client である。coding agent 自体でも、会話を自動的に仕事へ変換する scheduler でもない。意図は会話、合意 scope は [Issue](project-and-issue.md)、実行状態は [AgentRun](session-and-agent-run.md)、公開済み知識は [OpenWiki](repository-memory.md)が保持する。[会話契約](../../crates/server/src/routes/project_chat/model.rs)、[既存操作の adapter](../../crates/server/src/routes/project_chat/tools.rs)。

## 相談と操作可

送信ごとに保存する mode は `discuss`（相談／RO）または `operate`（操作可）。省略時は RO。操作可は権限の上限であって、操作の指示ではない。現在の直接依頼だけが mutation の根拠で、過去の RO 依頼を後の mode 切替で復活させない。旧保存値 issue / implement / change / answer / merge は、それぞれの狭い権限を維持する。server も tool ごとの allowed 判定を行う。[mode と受付](../../crates/server/src/routes/project_chat/mod.rs)、[権限表](../../crates/server/src/routes/project_chat/tools.rs#L38-L49)。

「Issue に残す」は作成だけ、「実装する」は必要なら自己完結した Issue を作り Workspace を開始する。「実装を変える」は対象 Session への指示で、Issue 編集を暗黙に伴わない。「merge」は [正式 Integration](formal-integration.md)へ渡す。これは model に与える routing 契約であり、曖昧な自然言語が必ず正しく解釈されるという保証ではない。Project 所属、revision、pending question などは操作 adapter が別に照合する。[指示と実装](../../crates/server/src/routes/project_chat/model.rs)、[操作境界](../../crates/server/src/routes/project_chat/tools.rs)。

Workspace の仕様質問への回答は original input / run / session の識別を保ち、直接回答との二重投入を防ぐ。実行 tool の承認は代行せず Workspace で判断する。任意 shell・HTTP・設定編集・秘密ファイル取得の権能を Astra に与えない。[質問と競合テスト](../../crates/server/src/routes/project_chat/tests.rs#L298-L497)。

## 永続化と再開

Project と固定 local owner ごとの Astra conversation に message を保存する。同一 request ID の再送は内容・conversation・mode が同じ場合だけ受け付ける。worker は pending **user message** のある会話だけを処理し、会話内の順序を保つ。Workspace event は情報として残っても推論を自動起動しない。restart は旧 event の pending / running を done に収束させ、user の running を pending に戻す。[store](../../crates/server/src/routes/project_chat/store.rs)、[worker](../../crates/server/src/routes/project_chat/mod.rs)、[event 非実行テスト](../../crates/server/src/routes/project_chat/tests.rs#L730-L801)。

turn の checkpoint は model が提案した call を副作用より先に保存する。operation は message / call ID・名前・引数を固定し、保存済み receipt を再利用する。中断した mutation は既存 LVK identity を照合し、不確実なら新しい call ID で再実行せず止める。model 接続障害では保存した user message を backoff 付きで再試行する。過去会話は直近60 message の参照、現在状態は各 model round 前に再取得し、16 round で停止する。[checkpoint と invoke](../../crates/server/src/routes/project_chat/mod.rs#L294-L472)、[再接続・副作用後障害のテスト](../../crates/server/src/routes/project_chat/tests.rs#L631-L718)。

受理済み command は実装完了ではなく、turn 成功は Goal 完了ではない。実装・検証・merge・Wiki 更新をそれぞれの状態と receipt で説明し、partial success と不確実な結果を分ける。coding の完了を待って会話を塞がない。[応答契約](../../crates/server/src/routes/project_chat/model.rs)。

## 推論と認証の境界

現行は server の Codex ChatGPT login を用いる。Codex app-server の version は0.154.0以上、account type は chatgpt を検査する。API key login は受け付けず、旧 saved key は保持可能でも推論に使わず、新規 key 登録は拒否する。model / reasoning effort は saved preference、次に `LVK_ASTRA_MODEL` / `LVK_ASTRA_REASONING_EFFORT` を使う。model 未設定または認証不可なら user queue を保存したまま待つ。[認証と version](../../crates/server/src/routes/project_chat/model/codex.rs#L177-L209)、[設定・競合保存](../../crates/server/src/routes/project_chat/settings.rs)。

推論ごとに temporary cwd・ephemeral thread・空 environments を用い、shell、apps、plugins、skills、memory、継承 MCP などを無効化する。schema-constrained 出力は文章または LVK function call の**提案**で、実行は既存 checkpoint loop に戻す。server から native tool / approval request が来ても実行させない。認証 refresh は Codex、会話と receipt は LVK の責務であり、API key による別 Responses client ではない。[隔離設定と schema](../../crates/server/src/routes/project_chat/model/codex.rs#L210-L273)。

推論 timeout は180秒、認証 probe は10秒、結果 cache は15秒。foreground child は成功・失敗・timeout 後に kill / wait する。[process の寿命](../../crates/server/src/routes/project_chat/model/codex.rs#L274-L333)。これは実装と mock protocol test から確認した契約で、実アカウントへの接続試験結果ではない。`docs/features/project-chat.mdx` に残る event batching の説明は、現在の user-only worker と一致しない。
