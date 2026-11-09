---
type: concept
title: 保存済み本文の保護と並列作業の参照
description: Card Contextと旧Pipeline供給の廃止後に残る通常本文、既知定型文5hashの送信時除去、実ガイドと公開peer参照の境界。
tags: [card-context, pipeline, parallel-context, knowledge]
sources:
  - id: openwiki-source-a3956271cbf62ae0786e7fc7
    resource: repo://crates/executors/src/env.rs
  - id: openwiki-source-f4e0f9b2ce513d3f8b8b070e
    resource: repo://crates/executors/src/executors/provider_adapter.rs
  - id: openwiki-source-788fa2f6f1a3d449a1a9efe4
    resource: repo://crates/local-deployment/src/agent_run_port.rs
  - id: openwiki-source-dc107dec6d46a1dcc08d0a9b
    resource: repo://crates/mcp/src/task_server/tools/context.rs
  - id: openwiki-source-391f34050223d13f679b3793
    resource: repo://crates/server/src/routes/parallel_context.rs
  - id: openwiki-source-cc3b0007b9399fe7dda21b8e
    resource: repo://crates/services/src/services/parallel_context.rs
  - id: openwiki-source-8dd059d50335f535b6d49e83
    resource: repo://crates/utils/src/agent_guides.rs
  - id: openwiki-source-19419edea1d312cc9f07ef2d
    resource: repo://crates/utils/src/path.rs
  - id: openwiki-source-7cc1d248ce00f0c5ef61c76e
    resource: repo://tests/chat/prompt-context.test.mjs
generated: { by: "codex", at: "2026-11-09T18:10:41.827Z" }
verified:
  - by: openwiki/0.7.2
    at: 2026-11-09T18:20:01.087Z
---

# 保存済み本文の保護と並列作業の参照

Card Context専用UIとそのPipeline供給経路はLOCAL-361で廃止した。このページは保存済み本文の扱い、実行時の限定除去、共有領域・公開peer参照の境界を調べる入口である。[Issue](project-and-issue.md)の要求は通常本文で扱い、[WorkflowAttempt](workflow-attempt.md)の実行graphとは区別する。

## 保存と fresh Session への継承

専用欄、既定文の新規複製、hidden部分の分離・再結合、saved_context抽出・再注入、旧ON/OFFに依存するruntime分岐は撤去した。独自文・編集済みの文・未知Pipelineは通常本文として閲覧・編集する。保存値の一括移行やDB・TOMLの物理削除は行わない。新たな共通prompt設定も作らない。[通常description保存](../../packages/web-core/src/pages/kanban/KanbanIssuePanelContainer.tsx)・[本文のUI](../../packages/ui/src/components/KanbanIssuePanel.tsx)

送信時には保存Issue由来の入力だけを共通helperで処理する。通常launchは現在関連するIssueのtitleとdescriptionの合成に入力が完全一致し、通常coding判定を満たす場合だけ対象とする。追記した新規promptやReview例文は加工しない。外部MCPは保存Issue本文を合成する箇所で同じhelperを使い、Review判定を尊重する。[通常起動の限定](../../crates/local-deployment/src/agent_run_port.rs#L875-L899)・[MCP合成](../../crates/server/src/remote_mcp/launch.rs)

除去対象はlvk markerの既知共有定型文5版で、CRLFをLFに正規化したSHA-256の完全一致を条件とする。markerだけで本文全体を捨てない。編集済み・未知・不完全・不整合のenvelopeや旧evk版は保持する。先頭空白を除いた行がbacktickまたはtildeのfenceらしければ、外側の既知文も含め入力全体を保持する。これは引用保護を優先した保守的な境界であり、未知旧文の完全除去を保証しない。[5hashと全文保持](../../crates/executors/src/executors/provider_adapter.rs#L2651-L2725)

## チャットの補助情報と元の全文

PromptContextと表示用marker parserは撤去した。chat本文全体を通常rendererへ渡し、専用の補助情報折畳みや全文コピーを追加しない。編集callbackには元contentを渡す。通常MarkdownがHTML comment自体を表示する保証とは別である。[会話表示](../architecture/web-and-sync.md#user-message-の補助情報を表示時だけ折り畳む)

### 表示変更の検証範囲

`tests/chat/prompt-context.test.mjs` はChromiumの隔離fixtureで、実KanbanIssuePanel/WYSIWYGEditorを使った独自本文の閲覧・編集・保持、専用checkbox不在とchatの未分割表示を検査する。chat rendererはfixture置換であり、実保存API・認証・実モデルの統合試験ではない。送信helperのpredicate境界試験と実hash照合も別の証拠として扱う。[試験](../../tests/chat/prompt-context.test.mjs)

## 共有案内と現在のidentity

共有storageの所在と基本境界は共通親AGENTSを責任元とし、親がないときだけExecutionEnvがfallback案内を送る。tmpはRun終了では消さず、受渡し完了・不要・非使用時だけ整理する。RepoごとのGit identity案内は保存本文と独立し、同一の正常案内はRun内で一度、Repo固有の不備案内は保持する。Manifestの現在identityはGit著者情報とは異なる。[合成](../../crates/executors/src/env.rs#L53-L90)・[Git identity](workspace.md#git-commit-identity)

Manifest・parallel・資源操作の詳細は通常起動で配置する実ガイドファイルにまとまり、必要ならstdio `get_context(guide=coding|parallel|resources)` からも同じ定義を読める。引数なしget_contextはmetadataのみを返す。初回・継続の現在identityと短い契約は保存Cardの旧設定に依存しない。[ガイド配置](../../crates/utils/src/agent_guides.rs)・[任意MCP](../../crates/mcp/src/task_server/tools/context.rs)

## 並列活動と固定したソースを読む

`/api/repos/{id}/parallel-context` は登録 Repo の interactiveなWorktree Workspace（DirectFolderを除く）、現在の Card link、実行数、観測時の branch HEAD と immutable Manifest の参照を返す。peer の checkout を復元せず common repository の branch を読む。活動と Manifest は別 snapshot であり、Manifest がないことは活動ゼロ、現在の Card link は過去 event の帰属、という意味にはならない。[discovery](../../crates/services/src/services/parallel_context.rs#L63-L131)

Workspace と Manifest は独立した cursor でページングする。既定25・最大100件で、memory 読取失敗は `memory_error` として残す。空結果と情報源の故障を区別し、他 Workspace の私的文脈や raw conversation を参照する仕組みにしない。Wiki の target / source / publication commit は maintenance の観測情報であり、今の作業木へ最新 Wiki を配達した証拠ではない。[情報の解釈契約](../../crates/services/src/services/parallel_context.rs#L192-L198)

`/api/repos/{id}/snapshot` は指定 commit の Git object を読む。path 未指定なら一覧を100件ずつ返し、本文は通常ファイルの UTF-8・128 KiB以内に限定する。symlink / submodule を辿らず、大きな blob は理由を返して同じ固定 object の分割読取へ案内する。[snapshot API](../../crates/server/src/routes/parallel_context.rs#L43-L79)。組合せを試すときは [detached preview の操作](../workflows/task-to-integration.md#並列変更を試してから正式統合する)を使い、peer の作業木や target を変えない。

DirectFolder には自動共有リンクや peer discovery がない旨を通知する。一方、通常codingのWiki読取専用規則は別に適用し、Git発見だけでDirectFolderにManifestを有効化しない。[DirectFolder 分岐](../../crates/services/src/services/parallel_context.rs#L168-L170)

## 旧 LLM Wiki からの移行境界

旧 `.llm-wiki` は歴史的な作業branch内知識の構想であり、現行OpenWikiではない。LOCAL-361で旧Wiki互換filter、通知、旧skill pathの特別除外を撤去した。旧Recall/Enrichだけを選別除去する処理は提供しない。既存ファイルや保存Issueを自動削除・移行する意味ではなく、通常本文と選択Skillを保持する。[起動境界](../../crates/server/src/routes/sessions/agent_run.rs)・[現在のprompt合成](../../crates/executors/src/executors/provider_adapter.rs)

旧GET `/api/pipelines`、frontendのlist API、TOML parse/validation/seed/load/sort、Pipeline専用型とrouter/service登録も撤去済みで、互換endpointや空loaderは残さない。現行WorkflowやOpenWiki設定で使うtoml依存は別責務として残る。[現行router](../../crates/server/src/routes/mod.rs)・[型生成](../../crates/server/src/bin/generate_types.rs)
