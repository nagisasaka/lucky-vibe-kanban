---
type: concept
title: 稼働版のソースに基づく対話ヘルプ
description: UIとremote MCPから使う一般ヘルプ、Project不要の版固定ソース読取、会話の寿命と配布・実環境検証の境界。
tags: [help, source, mcp, distribution, testing]
verified:
  - by: openwiki/0.7.2
    at: 2026-11-12T04:13:28.445Z
sources:
  - id: openwiki-source-b688f8904e0728c38535a521
    resource: repo://crates/git/src/repository_read.rs
  - id: openwiki-source-a373f9a8da41f89b1ce1d5bd
    resource: repo://crates/server/src/bundled_source.rs
  - id: openwiki-source-cb711830e5a5d11f35f1d1c7
    resource: repo://crates/server/src/remote_mcp/help.rs
  - id: openwiki-source-71f837a8aa36443ad9688945
    resource: repo://crates/server/src/remote_mcp/mod.rs
  - id: openwiki-source-2ac65265d4518e3d0e1d338e
    resource: repo://crates/server/src/remote_mcp/repository_read.rs
  - id: openwiki-source-4936f3b3e77c32b1d6e30fb0
    resource: repo://crates/server/src/repository_read.rs
  - id: openwiki-source-d40201a53108a0c612f229ef
    resource: repo://crates/server/src/routes/help.rs
  - id: openwiki-source-2b4c7ad36a9b595e1991b189
    resource: repo://crates/server/src/routes/sessions/review.rs
  - id: openwiki-source-63e502af7564c9aae1780fd6
    resource: repo://deploy/developer/verify-access.mjs
  - id: openwiki-source-83ada096aaf457a8ce0ab3cb
    resource: repo://deploy/server/bundle-source.sh
  - id: openwiki-source-32f842cb226352305c70c9c8
    resource: repo://deploy/server/source.test.mjs
  - id: openwiki-source-bb1ebe868e35e9e500714501
    resource: repo://Dockerfile
  - id: openwiki-source-2a0e145e456438352b2e8e59
    resource: repo://packages/web-core/src/shared/components/InteractiveHelp.tsx
  - id: openwiki-source-84b875ef198dd7cc343720cd
    resource: repo://tests/help/README.md
generated: { by: "codex", at: "2026-11-12T04:13:28.445Z" }
---

# 稼働版のソースに基づく対話ヘルプ

LOCAL-375のヘルプは、稼働版と同じ公開ソース・OpenWiki・実在する貢献資料・Git履歴を調べて一般的な使い方を説明する。既存ヘルプUIとremote MCPは同じサービスを使う。開発したい場合は通常のProject / Issue / Workspaceへ進む。質問だけでcloneや実装を開始しない。[共通サービス](../../crates/server/src/routes/help.rs)・[UI](../../packages/web-core/src/shared/components/InteractiveHelp.tsx)

## 質問から回答まで

既存のProject/Workspaceガイドで「LVKに質問」を選び、質問を送る。設定済みの既定coding agentの認証・利用条件・料金が適用される。新規会話はasset directory配下の `help-conversations/<Workspace UUID>` を使うDirectFolder WorkspaceとSessionを作り、Projectを作らない。作業場所がGit内なら拒否するため、開発起動では `LVK_INSTANCE_DIR` をcheckout外に置く。[開始と対象検査](../../crates/server/src/routes/help.rs#L40-L160)

実行は既存SessionのREVIEW設定を使う通常会話であり、初回はInitial、継続はFollowUp intentになる。通常コードレビューのnative Review intentは維持する。継続では保存済みexecutor設定とprovider sessionを使い、グローバル既定agentの変更だけで会話のproviderを替えない。[共通launch](../../crates/server/src/routes/sessions/review.rs)・[設定継続](../../crates/server/src/routes/help.rs#L113-L150)

受付は回答成功ではない。UIは実行中にpollし、待機・実行・入力待ち・承認待ち・取消中と、成功・失敗・取消・crash・監査失敗を区別する。回答と質問ごとのsource commit/build revisionを表示し、入力・承認は既存Workspaceへのリンクで扱う。ヘルプを閉じても実行を取消さず、会話は削除まで保持する。削除は実行が終端になってから既存Workspace削除へ委譲する。[会話・取消・削除](../../crates/server/src/routes/help.rs#L162-L263)・[表示とpoll](../../packages/web-core/src/shared/components/InteractiveHelp.tsx)

REVIEWは強制read-only sandboxではない。ヘルプpromptは公開同梱資料の説明・調査に目的を限定し、実装・build・deploy・再起動や無指示のProject作成を禁じる。既存providerの権限・承認を維持し、ユーザーrepo・秘密・`/repos`全体への包括許可には広げない。作者の記述の引用、diffの事実、解釈を分け、根拠がなければPR規則や作者の意図を創作しない。[追加指示](../../crates/server/src/routes/help.rs#L40-L52)

## 上位AIから直接読むか、質問するか

調査箇所が分かる場合は `query_repository` に `source:"lvk"` とoperationを渡す。Project/repository IDは不要で、任意pathを対象rootに指定するAPIではない。target省略は配布commit、履歴は `target:{mode:"commit",ref:"<commit>"}` を明示する。Workspace modeは拒否し、read/list/grep/log/diffは既存readerの予算・除外・cursor・Git object読取を使う。dirty作業木を過去commitへ混ぜない。既存Project付きリクエストは従来のIDと認可を保つ。[selectorとschema](../../crates/git/src/repository_read.rs#L28-L96)・[共有read](../../crates/server/src/repository_read.rs#L76-L131)

一般質問はremote MCPの `ask_help({prompt,session_id?})` で開始・継続し、返されたSessionを `get_help_conversation({session_id})` で確認する。`cancel_help_conversation` と `delete_help_conversation` も同じSessionを指定する。readは `lvk:read`、変更は追加の `lvk:write` が必要。askは冪等ではなく、応答喪失時に自動再送しない。[tool契約](../../crates/server/src/remote_mcp/help.rs)・[接続](../integrations/remote-access.md)

Skillと通常Workspaceのオンデマンドガイドにも同梱sourceと対話ヘルプの使い分けを案内する。GitHub直接調査も可能だが、最新branchを稼働版と同一視しない。[Skill](../../plugins/lvk/skills/lvk-work/SKILL.md)・[通常agentガイド](../../crates/utils/src/agent_guides.rs#L25-L49)

## 参照版と配布の境界

serverは同梱sourceをプロセス内で一度検査する。build revisionが既知の40桁SHA、非shallow、HEADがbinary revisionと一致することを要求する。既定pathは `/opt/lvk/source`、server側overrideは `LVK_SOURCE_PATH`。欠落・版不明・不一致は明示エラーであり、起動時・質問時のfetch/pullはない。質問別の参照版と現在配布版を分けて読む。[検査](../../crates/server/src/bundled_source.rs#L17-L63)

Dockerは公開upstreamから指定SHAと全祖先を独立fetchする。CI shallow checkoutやcontextの `.git`除外に依存せず、callerのGit設定・hooks・credentials・alternatesを持ち込まない。全branch/tagは約束しない。sourceはHOMEと `/repos` volume外に置きUID/GID `10001:10001` 所有とする。参照sourceを変更しても稼働binaryは変わらない。[配布構成](../../Dockerfile)・[取得helper](../../deploy/server/bundle-source.sh)・[同梱範囲と測定](../../deploy/server/SOURCE.md)

## 別環境で残る受入検証

今回の統合はローカルmainへのソース公開とWiki更新である。以下は未実施で、Docker-free kit・mock browser・以前のdevelopment dirty binaryのHTTP成功を代用しない。実行は別途許可された隔離環境で行い、共有資源がある場合はsnapshotと正式な一括予約を先に行う。

1. 公開済みの完全SHAとclean context、Docker/Compose、隔離したTLS/認証fixtureと一時volumeを用意する。今回のローカル候補はGitHubへpushしないため、現時点ではそのSHAの公開fetch成功を前提にしない。未公開SHAを最新branchへfallbackさせない。
2. `.github/workflows/publish-server.yml` のmanual dispatch相当の非公開buildを使い、同じrevision引数でbinaryとsourceを作る。完成imageをimmutable digestで記録し、`bash deploy/server/smoke-image.sh IMAGE` を既存deploymentと異なる破棄可能な環境で実行する。これは一時TLS/認証・volume・containerを作る試験であり、稼働環境に向けない。
3. 実行ユーザーUID10001でsource HEADと `/api/info` build revisionの一致、`git log` / `git show`、認証付き `/api/repository/read` のlist/log/diffと祖先commitを確認する。`verify-access.mjs` がこのGit CLI/libgit2経路を含む。`.git` が独立directoryでshallow/alternatesや外部gitdir依存がなく、build入力を失っても履歴を読めることを確かめる。所有者エラーをwildcard safe.directoryで迂回しない。[image probe](../../deploy/developer/verify-access.mjs)
4. 異なる公開SHAの旧新imageを用意し、隔離環境のHOME/repos/work volumeを保持して新imageに作り直す。保持データと新binary/sourceの版一致、祖先message/diffを再確認する。同一imageの再起動smokeだけでimage更新の成功とはしない。失敗時は隔離して証拠を保持し、実DBへの盲目的rollbackはしない。
5. 認証済み実browserと実providerで一般質問・継続・作者message引用と解釈の分離を確認する。実際に回答生成している時点で取消し、終端状態とprovider/process終了、停止後削除を確認する。以前のinitialize待ち取消だけではこの条件を満たさない。
6. 別環境の実OAuth/MCP clientでread/write scope、4つのhelp toolとsource読取を試す。受付・進行・成功・失敗・取消を区別し、未設定/未認証provider、source欠落・版不明・不一致も成功表示しない。認証変更や実credentialの持込みはその環境の承認範囲に限定し、秘密を証拠へ出さない。
7. digest・binary revision・source commit・UID・履歴count/サイズ・実行結果・未確認項目を記録する。終了時は所有test processをstop/waitし、予約があればcleanupとidle検証を終えて解放する。失敗・切断・timeoutだけで終了確認とはしない。

[source検証手順](../../deploy/server/SOURCE.md)、[ヘルプfixtureと過去実測の限界](../../tests/help/README.md)、[今回のSHA別検証記録](../operations/development.md#local-375の対話ヘルプ統合検証)を参照する。
