---
type: integration
title: Agent providers
description: provider の認証、profile、能力と MCP の境界。
tags: [providers, codex, claude, mcp]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-24T09:04:01.540Z
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
  - id: openwiki-source-379c8cd3e1e9932f9adda9ee
    resource: repo://crates/services/src/services/container.rs
generated: { by: "codex", at: "2026-11-24T09:04:01.540Z" }
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

## Run の継承値と承認

通常 Run の表示用 discovery と起動 override は別のデータである。Codex は既存の作業位置・環境で `config/read` と `model/list` を読み、設定されたモデル・推論深さを優先する。設定がない項目は該当モデルの provider 既定候補から補えるが、モデル候補を実行後の観測値として扱わない。作業位置が未確定なら候補だけを返す。再開 Session では新規 thread 用の defaults を隠す。[Codex discovery](../../crates/executors/src/executors/codex.rs)・[再開判定](../../crates/services/src/services/container.rs)

取得した候補外モデル、独自 effort、特殊な approval 設定は表示用情報に保持し、既知の選択肢へ勝手に丸めない。Claude は未観測の native defaults を補完せず、モデル・推論深さに同じ継承表示の原則を使う。具体的な色・出典・解除操作は [Run 入力欄](../concepts/agent-run.md#次の-run-の設定を読む変更する)を参照する。[値の解決](../../crates/executors/src/executors/codex.rs)・[Claude](../../crates/executors/src/executors/claude.rs)

Codex の承認が未指定なら、通常の thread start と resume でも未指定を維持する。Sandbox Auto を理由に `on-request` を暗黙送信しない。明示された profile や Run の選択は明示値として維持する。[起動値と再開変換](../../crates/executors/src/executors/codex.rs)

| Codex の明示値 | 入力欄の意味 |
| --- | --- |
| `on-request` | 必要時に確認 |
| `never` | 確認しない |
| `untrusted` | 信頼されていない操作を確認 |

未指定であることは LVK 側でのツール自動許可の条件ではない。自動許可判定には明示の `danger-full-access` と `never` の組合せが必要である。承認設定、sandbox、MCP の個別承認、LVK の認可は別の境界として扱う。Claude に Codex 専用の `OnRequest` を適用すると未対応として拒否する。[Codex 判定](../../crates/executors/src/executors/codex.rs)・[Claude permission](../../crates/executors/src/executors/claude.rs)

注意点:

- 既存の保存済み launch override は復元対象であり、過去に人が指定したか機械生成かを推測して一括消去しない。[復元処理](../../packages/web-core/src/shared/lib/executorOverrides.ts)
- 読み取り結果は native thread の開始・再開後の実効値を証明しない。通常 Run の継承改善から、Workflow・Wiki・保存フィルターなどの専用制約が解除されたと解釈しない。
- モデル差し替え時は adapter 側でも旧モデルの reasoning を解除し、同時に明示された reasoning は保持する。候補一覧が空という理由だけで保存済み reasoning を消す処理とは異なる。[Codex](../../crates/executors/src/executors/codex.rs)・[Claude](../../crates/executors/src/executors/claude.rs)

## MCP と Skill

MCP の設定・資格情報は provider の実行環境と権限に依存する。[外部アクセス](remote-access.md)と [実行時権限](../concepts/runtime-access.md)を参照する。selected Skill は name/path の参照で、通常 chat と Goal 起動では渡す場所が異なる。利用可能であること自体は必ず実行する指示ではない。[Codex 実装](../../crates/executors/src/executors/codex.rs)

## 保存フィルターの推論専用実行

[保存フィルター](../concepts/saved-filters.md)の自然文評価は既存のReview Runを使うが、通常のREVIEW全体を制限する設定ではない。hostは永続化したpending予約のWorkspace・Session・実行場所を照合して推論専用roleを選ぶ。Git外の専用direct-folderで起動し、通常のrepositoryやIssueの実行文脈を付与しない。[roleの決定](../../crates/local-deployment/src/agent_run_port.rs)

Codexはfresh threadのみを許し、read-only・承認要求なし・environmentとdynamic toolsなしに固定する。shell・web・plugins・apps・hooksなどの設定も無効化する。Claudeは空のtoolsとstrictな空MCP設定を使い、slash command・hookを無効にする。profileの環境変数・command overrideでこの境界を迂回する設定は拒否する。[共通制約](../../crates/executors/src/source_protection.rs)・[Codex](../../crates/executors/src/executors/codex.rs)・[Claude](../../crates/executors/src/executors/claude.rs)

これはモデルへのプロンプトだけの約束ではなく、hostとprovider起動引数の制約である。provider境界の単体試験とfixtureによる意味評価は、実providerの認証・応答品質・実行成功の代用にはならない。

検証の境界: Codex の approval 継承・Code/Plan/Review/Goal・start/resume は adapter 試験で、再開 discovery は services 試験で扱う。[隔離 native 設定試験](../../scripts/test-codex-approval-config.py)と[offline MCP fixture](../../scripts/test-codex-mcp-readonly.py)は実 CLI を使えるが、外部モデルや実アカウントの end-to-end 成功とは別である。
