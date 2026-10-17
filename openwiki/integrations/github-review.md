---
type: integration
title: GitHub 認証・PR とレビューの境界
description: リポジトリ別 PAT の所有・適用範囲、push と PR base、作成・監視の部分成功と旧 Cloud Review の境界。
tags: [github, review, organization, webhook, cloud]
sources:
  - id: openwiki-source-d254ecaf5a9cc6c431ef182a
    resource: repo://crates/git-host/src/github/cli.rs
  - id: openwiki-source-040941dd1546f1e0a90b6c5f
    resource: repo://crates/git/src/cli.rs
  - id: openwiki-source-bd15422ef1716eca23408f1f
    resource: repo://crates/git/src/lib.rs
  - id: openwiki-source-16bb0bc0baa397cce8e201b1
    resource: repo://crates/server/src/routes/github_credentials.rs
  - id: openwiki-source-65c0f36670a161dc05040e69
    resource: repo://crates/server/src/routes/mod.rs
  - id: openwiki-source-27c2433bab843f01a51418b1
    resource: repo://crates/server/src/routes/workspaces/pr.rs
  - id: openwiki-source-72948db7f01925b4b2ff17af
    resource: repo://crates/services/src/services/pr_monitor.rs
  - id: openwiki-source-5837e5316279163dd8bfbfd3
    resource: repo://crates/utils/src/github_credentials.rs
  - id: openwiki-source-4815b99ed5b9f7632484ef04
    resource: repo://docs/integrations/github-integration.mdx
generated: { by: "codex", at: "2026-10-17T03:50:48.298Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-17T03:50:48.298Z
---

# GitHub 認証・PR とレビューの境界

現在の PR 操作は local Workspace と `GitHostService` を起点とする。旧 Cloud Review、組織 GitHub App installation、webhook 起動、R2 upload / review worker / callback は現行構成から削除されている。`!reviewfast` や旧 App 設定を有効な起動契約として扱わない。[現行 router](../../crates/server/src/routes/mod.rs)、[Workspace PR](../../crates/server/src/routes/workspaces/pr.rs)。

## リポジトリ別 PAT の所有と管理

Settings → Repositories → GitHub authentication で、登録 Repo の remotes と既定 push URL から GitHub.com の対象を選ぶ。保存 identity は小文字へ正規化した `owner/repository` であり、Workspace やログイン member ごとではない。同じ instance で同じ GitHub repository を使う操作は資格情報を共有する。[対象の列挙・検証](../../crates/server/src/routes/github_credentials.rs#L32-L63)・[正規化](../../crates/utils/src/github_credentials.rs#L15-L51)

API の一覧は repository と configured だけを返す。保存・置換・削除・接続確認は administrator を要求する。保存時は `github_pat_` 形式を検証するが、GitHub の有効性・権限を検査してから保存するわけではない。期限あり／なしを受け付け、期限切れや組織承認の問題は接続・実操作で判明する。[API](../../crates/server/src/routes/github_credentials.rs#L17-L155)・[入力検証](../../crates/utils/src/github_credentials.rs#L54-L70)

保存先は asset directory の `github-credentials/<owner>!<repo>.json`。Unix では directory 0700、file 0600 とし、temporary file の flush / sync 後に置換する。暗号化保管や同じ OS user のプロセスからの隔離ではない。instance volume と backup に秘密が含まれる。[storage](../../crates/utils/src/github_credentials.rs#L147-L198)・[permission test](../../crates/utils/src/github_credentials.rs#L223-L255)・[運用境界](../../docs/integrations/github-integration.mdx)

置換は次に読み込む操作へ反映され、既に起動した操作は旧 token を保持し得る。削除は LVK 内の登録を消して system 認証へ戻すだけで、GitHub 側の失効ではない。登録状態と接続成功も別である。[保存・削除](../../crates/utils/src/github_credentials.rs#L169-L198)・[運用手順](../../docs/integrations/github-integration.mdx)

## subprocess への適用と失敗

LVK の Git CLI wrapper は `push` / `fetch` / `ls-remote` の認識可能な GitHub URL に対して登録 token を探す。該当時は SSH URL も subprocess 内だけ HTTPS に変換し、remote 設定は変更しない。`gh` wrapper は repository または PR URL から同じ token を選び、PR 作成・関連付け・監視・コメント取得等に使う。`gh` binary 自体は引き続き必要である。[Git wrapper](../../crates/git/src/cli.rs#L889-L929)・[gh wrapper と呼出し](../../crates/git-host/src/github/cli.rs#L148-L225)

token は引数や repository config へ書かず子プロセス環境へ渡す。Git の credential helper をリセットし対象 HTTPS repository のみに応答させ、URL ごとの header、redirect 無効化、prompt 無効化、trace 環境除去を設定する。`gh` は一時 config directory と `GH_TOKEN` を使い、競合する token 環境を除く。戻り値は raw token と Basic の符号化値を伏せる。[環境設定・redaction](../../crates/utils/src/github_credentials.rs#L75-L144)

登録済み token が読めない・壊れている場合は launch 前に失敗し、認証拒否でも別アカウントの system credential へ retry しない。未登録の場合だけ既存認証が使われる。任意の agent shell の `git` / `gh` はこの注入経路の対象外であり、固有の認証を使う。複数 Repo の agent 実行へ資格情報を自動配布する契約は実装されていない。[読み込み失敗](../../crates/utils/src/github_credentials.rs#L169-L177)・[gh の単一実行](../../crates/git-host/src/github/cli.rs#L165-L234)・[適用範囲](../../docs/integrations/github-integration.mdx)

**Check connection** は GitHub API の repository と open PR 一覧を GET するだけで、push や PR 作成を行わず write 権限を証明しない。redirect を追わず各 request に15秒 timeout を設定する。失敗時は HTTP status と期限・対象・権限・組織承認の確認を案内する。[接続確認](../../crates/server/src/routes/github_credentials.rs#L117-L155)

## Push URL と PR base

Git push は既定 remote の `pushurl` があればそれを使う。PR base は target branch が remote-tracking branch ならその remote を使い、そうでなければ既定 fetch remote を使う。push 先が fork でも PR base を自動的に fork へ変えず、head repository URL として push 先を渡す。両 repository の認証・権限が必要な場合、片側の token 登録だけで他方へのアクセスを保証しない。[push 解決](../../crates/git/src/lib.rs#L1543-L1552)・[PR 解決](../../crates/server/src/routes/workspaces/pr.rs#L236-L254)・[head の指定](../../crates/server/src/routes/workspaces/pr.rs#L325-L339)

## PR の作成と保存

PR 作成は対象 Repo / branch / remote を検査し、[Repository Memory の source 完了と Wiki guard](../concepts/repository-memory.md#source-統合と-wiki-反映を分ける)を通し、作業 branch を push してから GitHostService に title・body・base・draft 等を渡す。GitHub の場合は `gh` を実行し、対象に登録済みの PAT があればそれを使い、未登録なら既存 system 認証を使う。これは Cloud App の installation を要求する経路ではない。[手動操作の案内](../../docs/integrations/github-integration.mdx)、[処理順](../../crates/server/src/routes/workspaces/pr.rs#L206-L345)。

push、remote PR 作成、local PR レコード作成は一括 transaction ではない。remote 作成後の local 保存失敗はログに残るため、エラーや表示欠落を理由に新規 PR を盲目的に作り直さず、既存 PR と branch を確認する。unsupported provider・CLI 未導入・認証失敗は区別した結果を返す。[結果と失敗](../../crates/server/src/routes/workspaces/pr.rs#L302-L402)。既存 PR を branch から付ける経路も別にある。[既存 PR の関連付け](../../crates/server/src/routes/workspaces/pr.rs#L404-L534)。

## 観測と完了を分ける

PR monitor は open PR の remote status を調べ、closed / merged を local レコードへ保存する。merged Workspace に open PR が残らず pinned でない場合は archive を試みる。archive 失敗はログとなり、remote merge を元へ戻すものではない。[monitor](../../crates/services/src/services/pr_monitor.rs#L119-L202)。

外部 merge の観測は local target の更新、正式 Integration の検証、Card Done、Wiki 更新の証明ではない。Memory event を統合済みと判断する祖先関係と receipt の境界は [外部 PR と Repository Memory](../concepts/repository-memory.md#source-統合と-wiki-反映を分ける)を参照する。

## 三種類の review

Workspace の agent review はその Session / AgentRun の作業であり、[実装から PR まで](../workflows/task-to-integration.md)の手順に属する。[正式 Integration](../concepts/formal-integration.md)の解決差分 review は候補 commit に結び付く公開 gate、[OpenWiki の独立 Review](../operations/openwiki-maintenance.md)は凍結ソースと知識の照合工程である。いずれも撤去された Cloud review worker の成功 callback とは異なる。レビュー結果だけでテスト、公開、通知到達まで完了したとは扱わない。

## 変更時の検証と限界

storage の置換・Unix permission・破損拒否、header/helper の対象制限は [utils tests](../../crates/utils/src/github_credentials.rs#L201-L315)、秘密を引数に入れないこと・失敗出力の redaction・壊れた token で起動しないことは [gh subprocess tests](../../crates/git-host/src/github/cli.rs#L621-L678)が扱う。[Git safety test](../../crates/git/tests/git_ops_safety.rs)は push URL へ送信し fetch URL を維持する。[UI test](../../tests/workflow/specs/github-credentials.spec.ts)は mock API で登録・置換・エラー・切替・削除を検査する。これらのソースを確認したことは実 PAT の権限や GitHub への write 成功を検証したことではない。
