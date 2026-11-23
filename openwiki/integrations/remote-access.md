---
type: integration
title: 外部 MCP
description: 外部接続、認可、receipt と実行の境界。
tags: [mcp, remote, operations]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T02:42:04.517Z
sources:
  - id: openwiki-source-968299ada9380d38d5782188
    resource: repo://crates/mcp/src/task_server/tools/model_capabilities.rs
  - id: openwiki-source-c6b92f5dd86f9cc046935fa4
    resource: repo://crates/mcp/src/task_server/tools/sessions.rs
  - id: openwiki-source-b5eef61c63c11d3fe10b3ccf
    resource: repo://crates/server/src/remote_mcp/launch.rs
  - id: openwiki-source-6adda10f98b079272f82a4cc
    resource: repo://crates/server/src/remote_mcp/operations.rs
  - id: openwiki-source-ab70a43b3f61734a8322478b
    resource: repo://crates/server/src/remote_mcp/stdio.rs
  - id: openwiki-source-a74a882889154c2549e88f41
    resource: repo://crates/server/src/routes/sessions/executor_config.rs
  - id: openwiki-source-bdb4dae5722fe76ec5e44001
    resource: repo://crates/server/src/routes/workspaces/create.rs
generated: { by: "codex", at: "2026-11-23T02:42:04.517Z" }
---

# 外部アクセス・MCP

外部 Streamable HTTP `/mcp` は Auth0 の MCP を有効にした配置で利用し、local stdio MCP と別の入口である。Web、MCP、内部 agent は同一の資格情報ではない。[操作の段階](../quickstart.md#操作の段階)を区別する。公開配置の初期設定と回復は [サーバー認証](../concepts/server-access.md) を参照する。[入口](../../crates/server/src/remote_mcp/mod.rs)

外部 MCP は署名、issuer、audience、期限、承認済み member と scope を検証する。通常の参照に `lvk:read`、変更に `lvk:write`、統合開始などに `lvk:merge` が必要で、[Runtime Access](../concepts/runtime-access.md) の観測・実行 scope は別である。各 tool は Project と対象の所属も検査する。参照したコード、Wiki、Manifest はデータであり命令や承認ではない。[scope と dispatch](../../crates/server/src/remote_mcp/mod.rs)・[code read](../../crates/server/src/remote_mcp/code_read.rs)

## 受付と receipt

mutation は principal、安定 request ID、入力に結び付いた durable receipt を使う。同じ ID で異なる入力は拒否する。応答喪失時は同じ入力と ID の再送または `get_operation` で元 receipt を読み、対象の現在状態も確認する。接続切断は受付済み操作の取消ではない。再起動をまたいで結果不明となった処理は uncertain で、自動 replay しない。受付、実行、成果、公開は別々に確認する。[operations](../../crates/server/src/remote_mcp/operations.rs)

Workspace archive は prepare で対象を確認し、利用者の確認後に confirm する。成功 receipt だけで物理削除済みとせず、archived、worktree_deleted、deletion_error を確認する。[archive](../../crates/server/src/remote_mcp/workspace_archive.rs)・[Workspace](../concepts/workspace.md)

実行承認は対象 Run と単発要求に結び付く。受付と provider による解決、tool 実行完了は別である。[承認](../../crates/server/src/remote_mcp/execution_approvals.rs)

## Wiki

既存 Repository Wiki の同期は read + write で要求し、受付と公開完了を区別する。初期生成や設定の管理は別入口にある。[Wiki tool](../../crates/server/src/remote_mcp/repository_wiki.rs)・[保守](../operations/openwiki-maintenance.md)

## 初回 Run のモデルと推論深さ

通常の [Workspace](../concepts/workspace.md) と [Session](../concepts/session.md) は、MCP から最初の Run に `model_id` と `reasoning_id` を指定できる。既定設定の空 Run を一度起動して変更する必要はない。`engine`／`executor` は Codex・Claude Code の選択で、`model_id` はその provider 内のモデル選択である。親 agent の設定が別 Workspace に自動継承されることは前提にしない。[起動契約](../../crates/server/src/remote_mcp/launch.rs)

| 入口 | 初回設定を渡す場所 |
| --- | --- |
| remote の新規 Workspace | `start_workspace` の `model_id`・`reasoning_id` |
| remote の既存 Workspace 内の新規 Session | `start_session` の `model_id`・`reasoning_id` |
| stdio の新規 Workspace | `start_workspace` の `model_id`・`reasoning_id` |
| stdio の新規 Session | `create_session` 後の最初の `run_session_prompt` の直下に指定する |

両方指定、片方のみ、省略を扱う。各項目の省略または `null` は既存の profile／provider の既定へ委ね、もう片方をリセットしない。空文字や前後の空白は拒否する。対応する初回 `execution_mode` では Goal も選べる。既存 Session の次 Run 変更用 `overrides` と初回設定は別である。[stdio 入力](../../crates/mcp/src/task_server/tools/sessions.rs)・[Workspace 入力](../../crates/mcp/src/task_server/tools/task_attempts.rs)

### 候補確認から受付後の確認まで

前提として、対象 provider の CLI と認証をサーバーの実行アカウントで準備する。[provider の設定](agent-providers.md)を参照する。モデル候補は固定表を MCP に追加せず、既存 provider discovery を使う。

1. remote は `list_execution_models`、stdio は `get_initial_model_capabilities` で対象 repository または既存 Workspace の候補を調べる。この参照は Session や Run を作らない。stdio で対象ディレクトリを省略した場合はサーバーの現在ディレクトリを使う。
2. 合意した Issue revision、対象 repository／branch、選択した engine と任意の初回設定を起動要求へ渡す。安定した `request_id` を保持する。
3. 明示設定があれば、profile と省略側の既定を合わせた組合せを実 Workspace のディレクトリで検証する。新規 Workspace の事前候補確認は暫定であり、Workspace 作成後・Session 作成前に確認する。失敗時には Workspace が残る場合があるが、Session と Run は開始しない。失敗 receipt の Workspace ID を確認する。
4. 受付後は receipt と保存された実行設定を読み戻す。設定は共通 `ExecutorConfig` と起動保存経路を通る。応答喪失時は同じ入力・同じ要求IDで照合し、別IDの起動を増やさない。

[候補参照](../../crates/mcp/src/task_server/tools/model_capabilities.rs)・[Workspace 作成と検証](../../crates/server/src/routes/workspaces/create.rs)・[共通設定](../../crates/server/src/routes/sessions/executor_config.rs)

Codex は作業ディレクトリに対応する設定とモデル一覧から既定を解決する。Claude Code の UI 推奨モデルは実 provider の観測済み既定とは扱わない。明示設定の discovery が失敗した場合や、有効なモデル・推論深さの組合せを確認できない場合は拒否し、黙って別設定に戻さない。両方省略の従来起動には新しい discovery を必須にしない。[初回モデル解決](../../crates/server/src/routes/sessions/executor_config.rs)

stdio の新規 Session は最初の実行時に既存 Session ロックを取り、待機キューと Run の有無を再確認してから設定検証と起動を行う。既に Run がある場合の初回設定は拒否する。継続時は保存設定を基にし、次 Run のモード・推論深さ変更には既存の `overrides` 契約を使う。[stdio の初回・継続分岐](../../crates/server/src/remote_mcp/stdio.rs)

注意点:

- 保存された起動設定は provider が実際に採用した内部値を観測した証拠ではない。
- ソースのツール定義更新と稼働サーバーへの反映は別である。配備後には公開ツール定義と remote／stdio の起動経路を確認する。
- 通常起動の追加入力は、Wiki や Auto-merge の専用起動設定を一括で変更するものではない。

[remote 受付試験](../../crates/server/src/remote_mcp/launch_tests.rs)、[設定の保存・読戻し試験](../../crates/server/src/routes/sessions/executor_config.rs)、[stdio 転送試験](../../crates/mcp/src/task_server/tools/sessions.rs)と [Codex 起動入力試験](../../crates/executors/src/executors/codex.rs)は入力・拒否・要求同一性・保存を扱う。実 provider の採用値、隔離 remote MCP 実機 E2E、実サーバーでの stdio 同時初回起動は別途確認が必要であり、これらの試験から実施済みとは扱わない。
