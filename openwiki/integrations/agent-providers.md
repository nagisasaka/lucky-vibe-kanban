---
type: reference
title: Agent provider・設定・MCP・Skill
description: LVK の provider adapter が保証する能力、設定の解決、Skill の注入、MCP の二つの接続方向と変更時の境界。
tags: [providers, capabilities, mcp, skills, configuration]
sources:
  - id: openwiki-source-70da521f2a76cd78ae162a26
    resource: repo://crates/executors/Cargo.toml
  - id: openwiki-source-a7d7b5cf722fd5837c4eb16e
    resource: repo://crates/executors/default_profiles.json
  - id: openwiki-source-a5b446edff482bd8a00e1a92
    resource: repo://crates/executors/src/agent_settings/public_api.rs
  - id: openwiki-source-ff0c687c0ecdbf94f91da913
    resource: repo://crates/executors/src/agent_tools.rs
  - id: openwiki-source-9ad3b97da67cba82e9d5ffc0
    resource: repo://crates/executors/src/agent_tools/public_api.rs
  - id: openwiki-source-7fd554effd70be44f98fe686
    resource: repo://crates/executors/src/config_write.rs
  - id: openwiki-source-bfcf08b9feacd16360e862a4
    resource: repo://crates/executors/src/executors/claude.rs
  - id: openwiki-source-5514d83ac049ec5c68456f7b
    resource: repo://crates/executors/src/executors/claude/auth.rs
  - id: openwiki-source-b352d709d360d1bf77a8f1d5
    resource: repo://crates/executors/src/executors/claude/command_adapter.rs
  - id: openwiki-source-7c91fae76fb68d8e144a3d4f
    resource: repo://crates/executors/src/executors/claude/slash_commands.rs
  - id: openwiki-source-22b82518475034ade38104e6
    resource: repo://crates/executors/src/executors/codex.rs
  - id: openwiki-source-c88a7c6fabdac15166554032
    resource: repo://crates/executors/src/executors/codex/client.rs
  - id: openwiki-source-38fc795b99b30483be700b71
    resource: repo://crates/executors/src/executors/codex/client/openwiki.rs
  - id: openwiki-source-1222b138c478c7fe70625991
    resource: repo://crates/executors/src/executors/mod.rs
  - id: openwiki-source-f4e0f9b2ce513d3f8b8b070e
    resource: repo://crates/executors/src/executors/provider_adapter.rs
  - id: openwiki-source-38bb7eaa90f0c92a0514844d
    resource: repo://crates/executors/src/legacy_wiki.rs
  - id: openwiki-source-f07f62eba81c4ffbaf0db9d5
    resource: repo://crates/executors/src/profile.rs
  - id: openwiki-source-bccd47f2decfe8aa6a080767
    resource: repo://crates/executors/src/provider_policy.rs
  - id: openwiki-source-8fd7f4fe2d73e778cc63c256
    resource: repo://crates/local-deployment/src/process_host.rs
  - id: openwiki-source-5a8e26da42457b4b47bca3ee
    resource: repo://crates/local-deployment/src/process_host/approvals.rs
  - id: openwiki-source-0a300791bc056c6eee6a577a
    resource: repo://crates/mcp/src/bin/vibe_kanban_mcp.rs
  - id: openwiki-source-773e3d7e88410badd1cab059
    resource: repo://crates/mcp/src/task_server/mod.rs
  - id: openwiki-source-4c00e6ad62d940e70c2ebbb3
    resource: repo://crates/mcp/src/task_server/tools/mod.rs
  - id: openwiki-source-9994439c3151f599f949bf7f
    resource: repo://crates/mcp/src/task_server/tools/workspaces.rs
  - id: openwiki-source-71f837a8aa36443ad9688945
    resource: repo://crates/server/src/remote_mcp/mod.rs
  - id: openwiki-source-a9b6d4be264787a78357c9d6
    resource: repo://crates/server/src/routes/config.rs
  - id: openwiki-source-65c0f36670a161dc05040e69
    resource: repo://crates/server/src/routes/mod.rs
  - id: openwiki-source-379c8cd3e1e9932f9adda9ee
    resource: repo://crates/services/src/services/container.rs
  - id: openwiki-source-dd14ba47f6d5e604d8574f1d
    resource: repo://crates/utils/src/shell.rs
  - id: openwiki-source-c5de8230bc8ac4c0c0000287
    resource: repo://deploy/server/release.test.mjs
  - id: openwiki-source-bb1ebe868e35e9e500714501
    resource: repo://Dockerfile
  - id: openwiki-source-416673f852b62093837a9a91
    resource: repo://docs/agents/claude-code.mdx
  - id: openwiki-source-459fb9f29be1bc52c827e706
    resource: repo://docs/development/codex-compatibility.md
  - id: openwiki-source-bd8b8c5ef2cd6ade70bfa3e5
    resource: repo://docs/future/agent-runtime/delegated-agent-display.md
  - id: openwiki-source-223410809ba8bde718d60d3b
    resource: repo://packages/ui/src/components/ModelList.tsx
  - id: openwiki-source-f2fdb07edb2a73ea40538a98
    resource: repo://packages/web-core/src/shared/components/ModelSelectorContainer.tsx
  - id: openwiki-source-71c7c94338c94576d4db90b5
    resource: repo://packages/web-core/src/shared/dialogs/settings/settings/settingsRegistry.tsx
  - id: openwiki-source-f66eb76c24833219ccd50c84
    resource: repo://packages/web-core/src/shared/lib/modelSelector.ts
  - id: openwiki-source-bd678cc808ff9bca99021f97
    resource: repo://packages/web-core/src/shared/lib/runtimeAccessApi.ts
  - id: openwiki-source-92ef988228c55634defb8970
    resource: repo://scripts/test-codex-app-server.py
  - id: openwiki-source-7d0c710ba947513a98d2c88d
    resource: repo://tests/claude-subscription/README.md
  - id: openwiki-source-d8023ed75464072c99cdef5c
    resource: repo://tests/claude-subscription/subscription.spec.ts
generated: { by: "codex", at: "2026-10-23T15:33:24.249Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-23T22:40:49.979Z
---

# Agent provider・設定・MCP・Skill

LVK は provider ごとの通信を adapter で解釈し、共通の [AgentRun](../concepts/session-and-agent-run.md) として実行する。現行の通常ビルドの CodingAgent は ClaudeCode、Gemini、Codex、OhMyPi の4種類である。provider を追加するときは enum と起動処理だけでなく、能力の解決、制御の符号化、native event の正規化を一緒に扱う。[対象 enum](../../crates/executors/src/executors/mod.rs#L130-L137)・[adapter 境界](../../crates/executors/src/executors/provider_adapter.rs)

## Profile と実行能力

Profile は保存した provider 設定の variant であり、個々の実行では model、agent、reasoning、permission、execution mode、budget、subagent concurrency の override を適用できる。Code / Plan / Goal と PermissionPolicy は別軸である。旧 `PlanWithGoal` は保存・API 互換値として残るが新規起動では Plan となる。設定画面の操作は [Agent Configurations](../../docs/settings/agent-configurations.mdx)、実行中の変更・Goal の寿命は [Session と AgentRun](../concepts/session-and-agent-run.md) を参照する。[型と override](../../crates/executors/src/profile.rs#L21-L37)・[override の項目](../../crates/executors/src/profile.rs#L141-L176)

Profile は既定値と user の profiles.json を合成して読み、保存時は既定値との差分を正規化・検証して書く。読み込みの parse 失敗はログを残して既定値へ戻るため、画面に選択肢があるだけでは保存した設定の適用を確認できない。[読み書き](../../crates/executors/src/profile.rs#L351-L410)

次は adapter が実行用 snapshot に記録する状態であり、上流製品全体の機能一覧ではない。4種類とも SessionResume / Images / Review / MCP / TokenUsage は Native。差がある項目は以下の通り。

| Provider | Steering | Approval | Subagents | Goal |
| --- | --- | --- | --- | --- |
| Gemini | Unsupported | Native | Unknown | Unsupported |
| Codex | Native | Native | Unknown | Native |
| ClaudeCode | Native | Native | Native | Unsupported |
| OhMyPi | Native | Unsupported | Native | Unsupported |

Unknown は能力が解決されていない状態であり、「上流に機能がない」という証拠にはならない。能力を要求する経路は Native を許可し、Emulated は明示許可を要し、Unknown と Unsupported は拒否する。native 子 agent の観測と capability gating の整合性を変える際は両方を確認する。[snapshot](../../crates/executors/src/executors/provider_adapter.rs)・[能力の検証](../../crates/executors/src/executors/provider_adapter.rs)

Approval が Native という capability snapshot と、host に人の応答を待つサービスが接続されていることは別である。[Codex の command/file 待機と Plan・質問の境界](../concepts/session-and-agent-run.md#承認質問と現在の接続状態) を合わせて確認する。Codex host は command/file 用 waiter を持つが、他 provider の注入は Noop のままであり、capability 名だけから全 provider の対話承認を保証しない。[host の分岐](../../crates/local-deployment/src/process_host.rs#L348-L366)

Codex の Supervised override は、approval policy が未指定または Never の場合に UnlessTrusted へ変更し、別の明示 policy は保持する。sandbox 自体はこの override で変更しない。client の自動承認は DangerFullAccess と未指定/Never の組合せだけに限定する。[override と自動承認条件](../../crates/executors/src/executors/codex.rs#L628-L680)・[回帰テスト](../../crates/executors/src/executors/codex.rs#L1755-L1776)

起動時には provider と profile の一致を検証する。Initial は native session を持てず、FollowUp / Resume は native session を必要とする。reset_to_message_id を伴う会話の巻き戻しは ClaudeCode の FollowUp / Resume に限定される。native session ID は UUID と仮定せず、空白・制御文字や provider ごとの option 解釈の危険を拒否する。[起動の検証](../../crates/executors/src/executors/provider_adapter.rs)

### Codex の override と解決済み effort を分ける

LVK の profile/実行 override が空でも、app-server は利用者・project 設定から model と reasoning effort を解決できる。client は `thread/start` / `thread/resume` の応答を採用してから collaboration mode と turn を構築する。したがって依頼の JSON の `reasoning_effort: null` だけで「設定ファイルの effort が無視された」と結論しない。逆に provider が返した `None` も解決結果として採用し、前回の明示値を残さない。[採用処理](../../crates/executors/src/executors/codex/client.rs#L183-L190)、[start/resume と turn](../../crates/executors/src/executors/codex/client.rs#L257-L322)

実際の値を診断するときは、依頼 override と native の thread 応答・後続 turn の設定を分けて確認する。継承の回帰テストは [reasoning_tests](../../crates/executors/src/executors/codex/client/reasoning_tests.rs#L67-L205) にある。LVK の型が値を表現できることを、任意の provider/model がその値に対応する保証とは扱わない。

### Discovery と手動 override の境界

model discovery が返す reasoning 候補は表示用メタデータであり、保存済みの明示値を消す権限を持たない。候補が空、古い、または手動値を含まなくても、selector は非空の `reasoning_id` を保持する。候補の既定値を明示 override として補完せず、未指定・`null`・空文字は表示上の設定追従となる。[表示値の解決](../../packages/web-core/src/shared/lib/modelSelector.ts#L175-L184)・[container の接続](../../packages/web-core/src/shared/components/ModelSelectorContainer.tsx#L197-L217)

推論メニューは候補がなくても保持中の値があれば表示でき、一覧にない値はチェック付きの disabled 項目として示す。利用者は **Follow CLI config** を選んで `null` へ戻せる。モデルボタンのラベルも候補からモデルを取得できない場合に保持中の effort を使う。この表示は provider がその effort を受理した証拠ではない。[メニュー](../../packages/ui/src/components/ModelList.tsx#L32-L106)・[モデル行の表示条件](../../packages/ui/src/components/ModelList.tsx#L210-L220)・[ボタンのラベル](../../packages/web-core/src/shared/components/ModelSelectorContainer.tsx#L394-L407)

これは CLI/TOML の解決方針を変更する契約ではなく、発見結果の変化で利用者の選択を暗黙に書き換えないための境界である。送信・再表示の保持は [composer の手動推論設定と復元](../concepts/session-and-agent-run.md#composer-の手動推論設定と復元)を正本とし、Scratch の None 省略、preset 選択、モデル変更時の解除をそこで説明する。[候補欠落の回帰テスト](../../packages/web-core/src/shared/lib/modelSelector.test.ts#L126-L173)は表示値の保持を検査するもので、live model の対応能力を検証しない。

## Claude のサブスクリプション専用 profile

`SUBSCRIPTION` は通常 Issue → Workspace → Session で公式 `claude` CLI の subscription login を使うための任意 preset で、`require_subscription_login: true` と `approvals: true` を持つ。DEFAULT の置換でも新しい provider / scheduler でもない。既存の stream-json、native resume、取消 adapter を使い、Claude native Goal や Arena の追加対応を意味しない。`approvals: true` だけで人の待機 UI が配線されるわけではなく、上記の host approval 境界を引き続き確認する。[preset](../../crates/executors/default_profiles.json#L3-L15)・[direct launch](../../crates/executors/src/executors/claude.rs#L215-L237)・[command adapter](../../crates/executors/src/executors/claude/command_adapter.rs#L33-L115)

利用者は **LVK の実行環境内の同じ account と設定ディレクトリ**で CLI を準備し、`claude auth login` / `claude auth status` を行ってから Subscription preset を選ぶ。別マシンでの login を server の設定と取り違えない。導入・同意・実アカウント確認の手順は [Claude Code 操作文書](../../docs/agents/claude-code.mdx)に置く。LVK は credential file を読み取って SDK へ転用せず、public auth metadata を確認する。[auth probe](../../crates/executors/src/executors/claude/auth.rs#L1-L128)

### 認証の確認と拒否条件

実行直前の `spawn_internal` は現在の cwd と execution env に profile env を合成して認証を確認する。新規・follow-up / resume の direct launch がこの経路を通る。`claude --settings … auth status` は10秒で制限し、成功 exit、`loggedIn: true`、`authMethod: "claude.ai"`、`apiProvider: "firstParty"` のすべてを要求する。JSON 不正・未 login・異なる provider・timeout を別課金方式への fallback で回復しない。stderr は捨て、stdout の原文を診断へ含めない。[起動前確認](../../crates/executors/src/executors/claude.rs#L739-L775)・[判定](../../crates/executors/src/executors/claude/auth.rs#L35-L49)・[環境と timeout](../../crates/executors/src/executors/claude/auth.rs#L78-L128)

専用 profile は router、base command override、非空の追加 CLI 引数を拒否する。また API key/token、OAuth token 注入、custom headers、base URL / profile、Bedrock / Vertex / Foundry 選択など、ソースで列挙する競合環境変数の非空値を拒否し、診断には変数名だけを出す。任意の wrapper や引数で preflight 後の認証を変えられないようにするという理由がソースコメントに記録されている。他方式を意図するときは別 profile を使う。[拒否集合と理由](../../crates/executors/src/executors/claude/auth.rs#L12-L99)

runtime と実 CLI discovery probe は per-process settings の `forceLoginMethod: "claudeai"` および空の `ANTHROPIC_CUSTOM_HEADERS` を渡し、通常の CLI 設定ファイルを書き換えない。ただし managed settings は本来の優先順位を維持するため、管理者設定を超える普遍的な保証ではない。従来の `disable_api_key` は一変数を取り除く別機能で、厳格 profile はその除去より前に競合を検査する。[settings と適用](../../crates/executors/src/executors/claude/command_adapter.rs#L11-L33)・[runtime / discovery](../../crates/executors/src/executors/claude/command_adapter.rs#L80-L145)・[除去との順序](../../crates/executors/src/executors/claude.rs#L739-L775)

### 選択 profile と discovery の一貫性

`/api/agents/profile-policy` は executor / variant を解決し、存在しない profile を拒否する。厳格 Claude profile だけ auth probe を行い、実行ファイル欠落を MissingExecutable、それ以外の認証確認失敗を AuthRequired として、InitialRun / FollowUp / SessionResume / WorkflowAgentStep / TeamSlot を除く。一般の installation 検出は `.claude.json` の更新時刻を login 証拠にしない。[policy API](../../crates/server/src/routes/config.rs#L804-L838)・[失敗と能力](../../crates/executors/src/provider_policy.rs#L96-L134)・[installation](../../crates/executors/src/executors/claude.rs#L727-L736)

policy と discovery は共通の scope 解決を使う。Session 指定時は所属 Workspace と agent working dir、Workspace だけならその container path、Repo だけなら Repo path を使う。policy は scope がなければ server cwd を使う。この確認は server 環境での readiness であり、実 launch の完全な execution env を保証しない。[scope 解決](../../crates/services/src/services/container.rs#L105-L193)・[policy env](../../crates/server/src/routes/config.rs#L823-L836)。[composer の接続](../concepts/session-and-agent-run.md#選択した-claude-profile-の-readiness)は選択 variant と scope を保持し、確認失敗を別 profile の Ready で補わない。

discovery stream も選択 variant を渡し、未存在 profile を DEFAULT へ代替しない。厳格 profile の要求から DEFAULT の global cache refresh を並行起動しない。Claude cache key は command 設定と `require_subscription_login` を含み、認証モードを分離する。実 CLI の command/plugin probe は認証確認後に起動するが、cache hit は既存 options を返し得るため、モデル・補完候補が表示されたことは現在の login 成功の証拠ではない。[stream の選択](../../crates/server/src/routes/config.rs#L893-L908)・[別 profile の refresh 抑止](../../crates/services/src/services/container.rs#L196-L233)・[cache key](../../crates/executors/src/executors/claude.rs#L367-L369)・[cache hit](../../crates/executors/src/executors/claude.rs#L513-L540)・[CLI probe](../../crates/executors/src/executors/claude/slash_commands.rs#L239-L265)

### 検証の範囲と残る確認

[auth unit tests](../../crates/executors/src/executors/claude/auth.rs#L147-L260)は競合設定、秘密値の非表示、wrapper 拒否、metadata / exit 判定を扱う。[frontend tests](../../packages/web-core/src/shared/hooks/useAgentProviderPolicy.test.ts)と [discovery URL tests](../../packages/web-core/src/shared/lib/agentDiscovery.test.ts)は profile 別の失敗と scope を検査する。[隔離 browser 試験](../../tests/claude-subscription/subscription.spec.ts)は実 backend / host と offline CLI fixture を接続し、拒否・初回実行・期限切れ後の follow-up 拒否・Stop / native resume と Issue UI の preset 選択を検査する。高速終了時の証拠は [shared host の drain 契約](../architecture/agent-runtime.md#os-終了より遅い-provider-終端証拠)で扱う。

fixture は公式 CLI でも実モデルでもない。auth metadata は実モデル応答、refresh 可能性、残 quota を証明しない。公式 CLI / account での期限切れ・更新・quota・network 挙動と managed settings の影響は別途確認が必要である。[検証手順と限界](../../tests/claude-subscription/README.md#live-validation-remains-separate)。この Sync は統合済み source と試験定義の静的照合であり、製品テストや live Claude 検証の再実行ではない。

## Skill と Tool Manager

Codex の selected_skills は name/path の参照であり、渡し方は実行経路によって異なる。Skill が利用可能であることは、その Skill を必ず実行する要求ではない。

| 経路 | Skill を渡す場所と順序 |
| --- | --- |
| 通常 chat（Goal の直接起動以外） | 構造化 `UserInput::Skill` を本文 `UserInput::Text` より前に並べ、turn を開始する |
| Goal の直接起動 | 選択した name/path の JSON 参照を thread の `developer_instructions` に追加してから thread を start/resume し、Goal を開始する。既存の developer instructions は保持する |

[起動分岐](../../crates/executors/src/executors/codex.rs)、[chat 入力](../../crates/executors/src/executors/codex.rs)、[Goal context と resume への継承](../../crates/executors/src/executors/codex.rs)、[resume 変換](../../crates/executors/src/executors/codex.rs)

この区別の理由は、Goal activation に UserInput 配列がないこと、余分な推論 turn を発生させず、永続する 4,000 文字上限の objective を Skill 参照で膨らませないことである。これはソースコメントと [Control and skills の設計説明](../../docs/future/agent-runtime/delegated-agent-display.md#control-and-skills) に記録された理由である。同文書は明示的な承認・廃止ステータスを持たず Validation plan も含むが、この Skill の節は現行コードおよび [通常 chat の順序テスト](../../crates/executors/src/executors/codex.rs)、[Goal の初回・再開・既存指示保持テスト](../../crates/executors/src/executors/codex.rs) と整合する。文書内の将来の再投影構想まで実装済みとは扱わない。

両経路とも、旧 LVK 同梱 `skills/llm-wiki` 配下の参照を除外する。同じ名前でも利用者が別の場所に置いた Skill は残し、既存ファイルを削除しない。旧 Pipeline の互換処理は [Card context と LLM Wiki の終了](../concepts/card-context-and-llm-wiki.md)に置く。[選択のフィルター](../../crates/executors/src/legacy_wiki.rs#L101-L119)

Plan の承認後遷移と PlanWithGoal の退役は [Session の Plan 契約](../concepts/session-and-agent-run.md#plan-の遷移)、resume 時に現在の host context を会話へ反映する順序と失敗は [Provider session の継続](../concepts/session-and-agent-run.md#provider-session-の継続と取り込み)を参照する。

Tool Manager は MCP Server と Skill を User / Project scope で扱う。Project は project_path があるときに探索し、個別の読み取りエラーも結果に収集する。これは会話単位の設定ではなく provider の実ファイルを管理する API である。[型](../../crates/executors/src/agent_tools.rs#L89-L117)・[探索](../../crates/executors/src/agent_tools.rs#L591-L618)・[操作 API](../../crates/server/src/routes/agent_tools.rs#L1-L156)

変更・削除と copy 元は expected_revision を照合し、既存対象への copy は replace と対象 revision を必要とする。provider をまたぐ MCP コピーは移植可能な部分を出力する。元 provider 専用 metadata は内部の結果には残るが、通常の公開応答には秘密値を含む原文として返さない。コピー成功は元設定の全項目の再現を意味しない。[copy の内部処理](../../crates/executors/src/agent_tools.rs#L495-L548)、[変更・削除の照合](../../crates/executors/src/agent_tools.rs#L698-L728)、[copy の公開応答](../../crates/executors/src/agent_tools/public_api.rs#L226-L239)

## 設定の公開と変更の安全境界

設定一覧・Tool discovery・diff は編集用の原文ではなく safe DTO である。env・引数・URL・header・未知の拡張、native file 本文や Skill asset は通常応答へ返さず、設定有無・項目数・非機密 allowlist を示す。parser エラーに秘密の原文行が含まれる可能性もあるため、通常エラーは定型化する。[設定 view](../../crates/executors/src/agent_settings/public_api.rs#L13-L109)、[Tool view](../../crates/executors/src/agent_tools/public_api.rs#L138-L174)

機密原文の表示・編集は明示確認と revision 検証を要する。非表示値を `***` のような文字列で round-trip せず、`Preserve / Replace / Clear` を明示する。省略項目を勝手に空にせず、未知の provider 設定も対象外なら維持する。native 設定の変更は共通の process-local lock を使うが、外部 editor の書込をロックするものではなく、観測 revision を別途照合する。[書込 intent](../../crates/executors/src/config_write.rs#L1-L27)、[明示読取](../../crates/executors/src/agent_tools/public_api.rs#L289-L302)

Host を切り替えた後に古い応答を反映しないことも、この機密設定の境界の一部である。選択した host が unavailable の場合に別 host の設定へ書き込まない。[Web の Host identity](../architecture/web-and-sync.md#ドラフト表示設定通知の保存境界)

## CLI 検出と provider 互換性

実行可否の確認と実 launch は同じ executable 解決を使う。絶対パス指定は別の PATH 候補へ置き換えず、通常名は PATH、有限時間の login-shell refresh、利用者の一般的な bin 配置を調べる。検出のために CLI を自動インストールしない。[共通 resolver](../../crates/utils/src/shell.rs#L55-L90)、[command builder](../../crates/executors/src/command.rs)

adapter は provider の nested message / tool block も解釈するが、raw frame は引き続き Audit に残す。現行 Codex mapper は v5、他 provider の mixed-content mapper は v3 である。補正を古い Audit replay へ遡及適用せず、選んだ version の意味で再生する。Claude の一つの envelope 内の text/tool の展開も adapter 内で行い、UI に provider 固有解析を持ち込まない。[version 選択](../../crates/executors/src/executors/provider_adapter.rs)、[mixed content](../../crates/executors/src/executors/provider_adapter.rs)

配布 image の CLI pin は `0.163.0`、Rust の `codex-protocol` / `codex-app-server-protocol` は `rust-v0.144.1` のままである。CLI version と消費する wire 型の version は一致を要求しない。[Docker pin](../../Dockerfile#L147-L150)・[依存](../../crates/executors/Cargo.toml#L42-L43)。[互換性の判断記録](../../docs/development/codex-compatibility.md#wire-types-and-upgrade-boundary)は、より新しい protocol crate の rollout/otel/state と SQLite link 依存の衝突を理由に wire subset を維持したと説明する。これは記録された判断であり、この Sync で依存 upgrade を再実験した結果ではない。

保持 fixture を Rust 型へ decode するテストと Docker pin / fixture version の一致検査がある。実 CLI を使う offline smoke は一時 Codex home と local mock Responses で Plan/Default、resume、steering、Goal を検査する。[wire test](../../crates/executors/src/executors/codex/client.rs#L2013-L2039)・[pin test](../../deploy/server/release.test.mjs#L78-L95)・[smoke](../../scripts/test-codex-app-server.py)。これらは使用 RPC の互換確認であり、live model 品質・browser・配布済み実行環境の更新を証明しない。操作手順と過去の検証結果は [互換記録](../../docs/development/codex-compatibility.md)を参照する。v5 の context / 子観測と v4 replay の維持は [runtime 投影](../architecture/agent-runtime.md#context-と子観測の投影)で説明する。

## MCP の二つの方向

### Coding agent → 外部 MCP server

LVK の設定操作は agent の native config を更新する。Codex は TOML の mcp_servers、他の現行 provider は JSON の mcpServers を用いる。advanced editor は明示的な機密読取確認と expected_revision を要し、既存 enclosing config の対象 server map を置換する。同じ provider を LVK 外で使う場合にも設定が及ぶ。[config 形式](../../crates/executors/src/executors/mod.rs#L139-L171)・[書き込み](../../crates/server/src/routes/config.rs#L363-L443)

接続例と通常の設定手順は [Connecting MCP Servers](../../docs/integrations/mcp-server-configuration.mdx) を参照する。そこに示す JSON 例を Codex の保存形式そのものと解釈しない。

OpenWiki maintenance writer は例外的に、登録 MCP が実行の前提である。LVK は該当 thread だけに `openwiki mcp --host codex` を設定し、`enabled=true` / `required=true` と60秒の startup timeout を指定する。thread/start・resume の RPC 待機は required OpenWiki writer の場合75秒に延長し、通常は30秒である。thread 登録後、通常 turn / Goal activation の前に、その thread の MCP catalog で六つの lifecycle tool が揃うことを確認する。server 名や ready 状態だけでは十分としない。欠落・不正な pagination・15 秒の preflight timeout は writer 起動失敗として返し、この確認自体では Wiki run を開始しない。[thread 設定](../../crates/executors/src/executors/codex.rs)、[起動順](../../crates/executors/src/executors/codex.rs)、[tool 検査](../../crates/executors/src/executors/codex/client/openwiki.rs)

writer の六つの lifecycle tool だけを thread-local `approval_mode=approve` にする。global approval policy を緩めず、Reviewer や通常 coding へ適用しない。

通常 coding の thread は OpenWiki MCP を無効にし、旧 `.llm-wiki` の生成停止と canonical OpenWiki の読取専用契約を developer instructions に追加する。保守 writer に設定する required MCP とは異なる所有境界である。[通常 thread の設定](../../crates/executors/src/executors/codex.rs)

Reviewer は別の信頼済み role で、ReadOnly / approval Never を設定し、OpenWiki MCP と Skill を thread 内で無効化する。writer の可用性確認を Reviewer に適用して、書込道具を復活させてはいけない。[Reviewer override](../../crates/executors/src/executors/codex.rs)。登録 MCP ではなく自作の shell/stdin bridge で OpenWiki を呼んでも、LVK の [完了証明](../operations/openwiki-maintenance.md)を代替しない。

### 外部 MCP client → LVK

LVK のローカル MCP server は stdio で動き、HTTP backend に要求を転送する。Global は Issue、Project、Repo、Workspace、Session などを扱い、Orchestrator は実行 context を必須とする限定 router で、list_workspaces と delete_workspace を公開しない。[起動](../../crates/mcp/src/bin/vibe_kanban_mcp.rs#L23-L49)・[context の取得](../../crates/mcp/src/task_server/mod.rs#L87-L119)・[公開 router](../../crates/mcp/src/task_server/tools/mod.rs#L52-L73)

両 stdio router は `get_workspace_tokens` を公開する。Workspace ID を省略すると現在 context を使い、context がなければ明示 ID が必要になる。Orchestrator は別 Workspace を拒否し、既存 `/api/workspaces/{id}/tokens` の読取へ転送する。欠損を補う AgentRun は起動しない。[tool](../../crates/mcp/src/task_server/tools/workspaces.rs#L99-L130)・[scope](../../crates/mcp/src/task_server/tools/mod.rs#L182-L208)。値の契約は [Workspace 累積トークン](../concepts/workspace.md#workspace-累積トークン)、認証付き HTTP MCP の別入口は [get_workspace の集計](remote-access.md#workspace-の報告済みトークンを読む)にまとめる。

backend の解決は VIBE_BACKEND_URL が優先され、続いて host/port の環境変数と port file を用いる。したがって「stdio client で接続する」ことと「backend が必ず localhost に固定される」ことは別である。導入例は [Vibe Kanban MCP Server](../../docs/integrations/vibe-kanban-mcp-server.mdx) を参照する。[backend URL の解決](../../crates/mcp/src/bin/vibe_kanban_mcp.rs#L100-L133)

loopback HTTPを選んだ場合だけ、stdio clientは同じdata directoryの`access-internal.token`を内部ヘッダーへ読み込む。外部backendへは送らず、HTTP redirectは追わない。任意backendを指定できることは、内部認証を外部へ委譲することではない。[client](../../crates/mcp/src/task_server/mod.rs#L247-L274)、[共通認証](../concepts/server-access.md#webmcp内部agentを同じ資格情報にしない)

失敗は MCP の error result と success:false / error / details で返す。HTTP 2xx でも backend envelope の success と data を確認してから成功扱いにする。自動操作を追加するときは transport の成功だけで Issue や Session の操作完了を判定しない。[エラー境界](../../crates/mcp/src/task_server/tools/mod.rs#L94-L143)

認証付き Streamable HTTP `/mcp` は別の外部入口であり、旧 Cloud / relay ではない。Auth0の承認済みmember・scopeと公開 tool の制約は [外部 MCP](remote-access.md#認証付き外部-mcp)を参照する。内部 Astra の推論・認証経路は廃止された。[廃止後に残る共有機能](../concepts/project-chat.md)と coding provider の起動を区別する。

Global stdio には [Project Todo](../concepts/project-todos.md)と[設計メモ](../concepts/design-notes.md)の操作も含む。設計メモの有界読取・確定版receipt・本文と独立追記の利用契約はリンク先を参照する。限定 Orchestrator router には含めない。Todo の REST / 外部 MCP の再送契約を、同じ tool 名だけで交換可能とみなさない。[router の分離](../../crates/mcp/src/task_server/tools/mod.rs#L49-L75)

Settings の「MCP連携」は LVK が外部へ公開する [Runtime Access の管理設定](../concepts/runtime-access.md#管理設定の保存適用取消)であり、上記のcoding agent向けnative MCP config editorとは別の入口である。直接アプリケーションを動かす権限・環境・停止の契約は同ページを参照する。[設定registry](../../packages/web-core/src/shared/dialogs/settings/settings/settingsRegistry.tsx#L95-L143)
