---
type: integration
title: Agent providers
description: provider の認証、profile、能力と MCP の境界。
tags: [providers, codex, claude, mcp]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-24T02:54:42.940Z
sources:
  - id: openwiki-source-bfcf08b9feacd16360e862a4
    resource: repo://crates/executors/src/executors/claude.rs
  - id: openwiki-source-5514d83ac049ec5c68456f7b
    resource: repo://crates/executors/src/executors/claude/auth.rs
  - id: openwiki-source-22b82518475034ade38104e6
    resource: repo://crates/executors/src/executors/codex.rs
  - id: openwiki-source-f23c89ae54db56391783fef4
    resource: repo://crates/executors/src/source_protection.rs
  - id: openwiki-source-788fa2f6f1a3d449a1a9efe4
    resource: repo://crates/local-deployment/src/agent_run_port.rs
generated: { by: "codex", at: "2026-11-24T02:54:42.940Z" }
---

# Agent provider・設定・MCP・Skill

LVK は Codex と Claude Code の native 通信を adapter で共通 [Agent Run](../concepts/session-and-agent-run.md) に変換する。profile は provider の保存済み variant、実行 override はその Run の選択であり、mode と permission も別である。capability snapshot は LVK の実行経路での能力を表し、provider 全体の機能や実際の成功を保証しない。[操作の段階](../quickstart.md#操作の段階)を区別する。[provider](../../crates/executors/src/executors/mod.rs)・[契約](../../crates/executors/src/runtime/contracts.rs)・[既定 profile](../../crates/executors/default_profiles.json)

## 実行 account と認証

LVK server の認証と provider CLI の認証は別である。login、status、実行を同じサーバー内 OS account と設定 directory で行う。標準 container の永続 home と手順は [サーバー運用](../operations/server-container.md#provider-の実行アカウントを準備する)を参照する。別 machine の login や設定ファイルの存在だけで有効な認証や次の実行成功を推定しない。

Codex は非空 `CODEX_HOME`、それ以外は実行 account の `~/.codex` を使う。native MCP 設定と login 状態もその解決先に依存する。カスタム home は永続化と秘密保護を別途確保し、profile だけの環境 override が server 自身の探索先も変えると仮定しない。[Codex](../../crates/executors/src/executors/codex.rs)

Claude の `CLAUDE_CONFIG_DIR` を使う場合も同一環境で CLI の login／status を行う。SUBSCRIPTION profile は起動直前に公式 CLI の認証 metadata を検査し、失敗時に別課金方式へ黙って fallback しない。画面の installation や discovery の候補は認証、quota、実モデル応答の証明ではない。[auth probe](../../crates/executors/src/executors/claude/auth.rs)・[起動](../../crates/executors/src/executors/claude.rs)

## Discovery と実行

選択 profile と Workspace／Session／Repo scope は discovery に渡される。候補の表示は一時 probe や cache の結果であり、実 launch の全環境や接続成功を保証しない。保存された明示 reasoning override を候補一覧が変わっただけで消さない。Codex の未指定 override と native thread が解決した effort は別に診断する。[discovery](../../crates/services/src/services/container.rs)・[selector](../../packages/web-core/src/shared/lib/modelSelector.ts)・[Codex client](../../crates/executors/src/executors/codex/client.rs)

Claude の supervised 実行は対応する承認 service と直接 CLI の条件を満たす必要があり、未接続や矛盾した permission 設定を暗黙に AUTO へ変えない。capability と host の接続状態は区別する。REVIEW は調査結果を報告して停止する意図で、製品変更の採用や公開の許可ではない。[Claude guard](../../crates/executors/src/executors/claude.rs)・[host](../../crates/local-deployment/src/process_host.rs)

## MCP と Skill

MCP の設定・資格情報は provider の実行環境と権限に依存する。[外部アクセス](remote-access.md)と [実行時権限](../concepts/runtime-access.md)を参照する。selected Skill は name/path の参照で、通常 chat と Goal 起動では渡す場所が異なる。利用可能であること自体は必ず実行する指示ではない。[Codex 実装](../../crates/executors/src/executors/codex.rs)

## 保存フィルターの推論専用実行

[保存フィルター](../concepts/saved-filters.md)の自然文評価は既存のReview Runを使うが、通常のREVIEW全体を制限する設定ではない。hostは永続化したpending予約のWorkspace・Session・実行場所を照合して推論専用roleを選ぶ。Git外の専用direct-folderで起動し、通常のrepositoryやIssueの実行文脈を付与しない。[roleの決定](../../crates/local-deployment/src/agent_run_port.rs)

Codexはfresh threadのみを許し、read-only・承認要求なし・environmentとdynamic toolsなしに固定する。shell・web・plugins・apps・hooksなどの設定も無効化する。Claudeは空のtoolsとstrictな空MCP設定を使い、slash command・hookを無効にする。profileの環境変数・command overrideでこの境界を迂回する設定は拒否する。[共通制約](../../crates/executors/src/source_protection.rs)・[Codex](../../crates/executors/src/executors/codex.rs)・[Claude](../../crates/executors/src/executors/claude.rs)

これはモデルへのプロンプトだけの約束ではなく、hostとprovider起動引数の制約である。provider境界の単体試験とfixtureによる意味評価は、実providerの認証・応答品質・実行成功の代用にはならない。
