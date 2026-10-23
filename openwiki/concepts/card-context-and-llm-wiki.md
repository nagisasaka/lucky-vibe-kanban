---
type: concept
title: Card context と並列作業の参照
description: 保存済み Card context の継承、並列 Workspace の参照、固定 commit の読取、および旧 LLM Wiki の廃止境界。
tags: [card-context, pipeline, parallel-context, knowledge]
sources:
  - id: openwiki-source-38bb7eaa90f0c92a0514844d
    resource: repo://crates/executors/src/legacy_wiki.rs
  - id: openwiki-source-788fa2f6f1a3d449a1a9efe4
    resource: repo://crates/local-deployment/src/agent_run_port.rs
  - id: openwiki-source-391f34050223d13f679b3793
    resource: repo://crates/server/src/routes/parallel_context.rs
  - id: openwiki-source-406c792263884bbf462d666c
    resource: repo://crates/server/src/routes/sessions/agent_run.rs
  - id: openwiki-source-5590f47e4cee001a35a42117
    resource: repo://crates/server/src/routes/workspaces/wiki.rs
  - id: openwiki-source-cc3b0007b9399fe7dda21b8e
    resource: repo://crates/services/src/services/parallel_context.rs
  - id: openwiki-source-1c3e0ab9fadf12dd97d8fd4f
    resource: repo://crates/services/src/services/pipelines.rs
  - id: openwiki-source-1f99436ccc32fb0e2aed140f
    resource: repo://docs/superpowers/specs/2026-09-09-llm-wiki-design.md
  - id: openwiki-source-6e1b3f3856aa7fc901ac98d4
    resource: repo://docs/workspaces/llm-wiki.mdx
  - id: openwiki-source-c0945aff6520d3600561e893
    resource: repo://packages/ui/src/components/PromptContext.tsx
  - id: openwiki-source-1d9ab14665c35291c675b104
    resource: repo://packages/ui/src/lib/splitPromptContext.ts
  - id: openwiki-source-483f7cd15b3732e8f861ec5c
    resource: repo://packages/web-core/src/features/pipeline/model/cardContext.test.ts
  - id: openwiki-source-ddf0f7c06298de76526a4f7c
    resource: repo://packages/web-core/src/features/pipeline/model/cardContext.ts
  - id: openwiki-source-74b3285af184d8dc6de75ea4
    resource: repo://packages/web-core/src/features/pipeline/model/cardPipeline.ts
  - id: openwiki-source-98daa093d94c1426c81c6133
    resource: repo://packages/web-core/src/features/pipeline/ui/CardContextSection.tsx
  - id: openwiki-source-06868af84d952b8087cf3ebb
    resource: repo://packages/web-core/src/i18n/locales/ja/common.json
  - id: openwiki-source-1dc3b065185d29e12afc5b4a
    resource: repo://packages/web-core/src/i18n/locales/ja/tasks.json
  - id: openwiki-source-5d59d00b98fae58771e90f3a
    resource: repo://tests/chat/prompt-context.fixture.tsx
  - id: openwiki-source-7cc1d248ce00f0c5ef61c76e
    resource: repo://tests/chat/prompt-context.test.mjs
generated: { by: "codex", at: "2026-10-23T23:13:08.630Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-23T23:13:08.630Z
---

# Card context と並列作業の参照

Card context は [Issue](project-and-issue.md) の依頼本文と一緒に保存する初期指示である。Pipeline はその中へ埋め込む宣言的テキストで、ロードしても server job は始まらない。[WorkflowAttempt](workflow-attempt.md) の実行グラフとは責任が異なる。[Pipeline の契約](../../crates/services/src/services/pipelines.rs#L1-L12)

## 保存と fresh Session への継承

新規カードの既定値は **Shared directories のみ**。保存済み Card context、旧 Pipeline、空ブロックによる明示的な無効化は保持し、依頼本文の編集で preset を再生成しない。共有 preset の ON/OFF は案内の編集であり、[共有ファイル](workspace.md#共有ディレクトリの意味)の削除や [Repository Memory](repository-memory.md) の無効化ではない。[保存と既定値](../../packages/web-core/src/features/pipeline/model/cardContext.ts#L24-L85)、[回帰テスト](../../packages/web-core/src/features/pipeline/model/cardContext.test.ts#L65-L121)

`lvk:card-context` と `vk:pipeline` の独立行 marker がブロック境界になる。Pipeline の再構成は最後の完全なブロックを置換し、周囲の本文と手書き補足を保持する。安定した pipeline/stage ID で生成行を区別する。[再構成](../../packages/web-core/src/features/pipeline/model/cardPipeline.ts#L19-L109)

初回 Workspace リクエストは保存済み context を含む。既存 Workspace に作る fresh Session にも、現在関連する local Card の **保存済み Shared directories 部分**をそのまま渡す。Card が未関連のときだけ initial prompt に戻り、関連 Card の空 description は旧 Card の指示を復活させない。continuation には保存指示を繰り返さず、現在の identity と参照案内を渡す。[初回リクエストのテスト](../../packages/web-core/src/features/pipeline/model/cardContext.test.ts#L65-L74)、[保存方針の取得](../../crates/services/src/services/parallel_context.rs#L134-L157)、[fresh / continuation](../../crates/services/src/services/parallel_context.rs#L160-L198)

実際の launch は選択された Repo ごとに policy を読み、存在するときだけ並列参照 context を追加する。provider の再開と現在 run の identity は [Session](session-and-agent-run.md) および [Codex 接続](../integrations/agent-providers.md)が扱う。[launch の結線](../../crates/local-deployment/src/agent_run_port.rs#L758-L775)、[追加条件](../../crates/local-deployment/src/agent_run_port.rs#L848-L865)

## チャットの補助情報と元の全文

設定画面の「共有ディレクトリの補助案内」（英語では `Shared directory guidance`）は、ONで案内を追加し、OFFでも共有領域と Repository Memory を無効にしないことを明示する。checkbox は `toggleSharedDirectories` → `withCardContext` → `onDescriptionChange` で保存対象の文脈を更新する。[設定の接続](../../packages/web-core/src/features/pipeline/ui/CardContextSection.tsx#L33-L44)・[checkbox と説明](../../packages/web-core/src/features/pipeline/ui/CardContextSection.tsx#L96-L109)・[日本語文言](../../packages/web-core/src/i18n/locales/ja/common.json#L11-L17)。この設定と、以下のチャット内の開閉は別の操作である。

user message の `PromptContext` は、完全な既知 marker ブロックを既定で閉じた「補助情報」にし、通常本文は従来の Markdown renderer に渡す。認識対象は `<!-- lvk:card-context:start -->` / `end`、`lvk:shared-directories`、`vk:pipeline` の三種類だけである。marker は行頭から単独行で一致する必要があり、末尾の空白・tab は許す。見出し、文中の単語、marker のない runtime 案内から補助情報を推定しない。[表示の分岐](../../packages/ui/src/components/PromptContext.tsx#L16-L34)・[marker の判定](../../packages/ui/src/lib/splitPromptContext.ts#L15-L37)。実際の呼出側との責任分界は [Web の user message 表示](../architecture/web-and-sync.md#user-message-の補助情報を表示時だけ折り畳む)を参照する。

parser は backtick / tilde の fenced code 内の marker を無視する。引用符 `>` やインデント付きの行も marker の完全一致にならず、そのまま通常本文に残る。異なる種類の正しく入れ子になったブロックは外側単位で折り畳み、閉じた後の同種ブロックの反復も扱う。一方、同じ種類の未閉鎖ブロックへの再入、対応しない end、閉じられない start があれば、既に見つけた正常ブロックも含め**メッセージ全体**を未分割へ戻す。ここで「表示する」は通常 renderer へ渡す意味であり、HTML comment 自体の可視化保証ではない。[stack と全体 fallback](../../packages/ui/src/lib/splitPromptContext.ts#L15-L61)

分割結果は元の文字列を `slice` したもので、marker や CRLF を正規化しない。補助情報を展開すると marker を含む plain text が見え、「元の全文を表示」も元の `content` を `pre` へ渡す。「元の全文をコピー」は表示 DOM や Markdown のコピー処理を経ず `navigator.clipboard.writeText(content)` を呼び、失敗時は成功扱いにせず全文展開によるコピーを案内する。折畳み対象が一つもなければ、この追加の全文表示・コピー操作は出さず従来 renderer のみになる。[文字列の保持](../../packages/ui/src/lib/splitPromptContext.ts#L42-L61)・[全文表示とコピー](../../packages/ui/src/components/PromptContext.tsx#L16-L61)・[失敗時の文言](../../packages/web-core/src/i18n/locales/ja/tasks.json#L375-L381)

これは表示量を減らす仕組みであり、送信 token の削減、保存 prompt の書換え、権限や Wiki 消費の切替ではない。表示用 parser を Card の保存・編集 parser と取り違えない。元の保存文脈を継承する条件は上記の [fresh Session 契約](#保存と-fresh-session-への継承)に従い、backend では並列案内の条件分岐と Repository Memory の注入を別々に扱う。[独立した注入境界](../../crates/local-deployment/src/agent_run_port.rs#L908-L941)

### 表示変更の検証範囲

`node --test tests/chat/prompt-context.test.mjs` は Chromium の隔離 component fixture で、通常文・fence・引用・不完全／不一致 marker の fallback、CRLF を含む再結合、入れ子 preset、反復ブロック、開閉とキーボード、全文コピーの成功／失敗、編集 callback、案内 ON/OFF の復元を検査する。390 / 1280px と light / dark の実 theme CSS で横 overflow も検査する。[入力と操作の assertion](../../tests/chat/prompt-context.test.mjs#L65-L179)。clipboard と Markdown renderer は置換されており、OS clipboard、実 Markdown の全描画、保存 API や実 provider の統合試験ではない。[fixture の renderer と編集 callback](../../tests/chat/prompt-context.fixture.tsx#L23-L47)・[clipboard の置換](../../tests/chat/prompt-context.test.mjs#L53-L62)。初回要求・明示 OFF・旧文脈の保持は別の [Card context model の回帰](../../packages/web-core/src/features/pipeline/model/cardContext.test.ts#L43-L128)で確認する。

この Sync は凍結入力全3チャンクと source `ba8519ab44f29d2493412b2f3d21f38d55fe07fd` を対象に、設定と保存 model、会話の呼出側、表示 parser、backend の注入境界、独立した試験定義を静的に照合した。入力には開発時の成功報告と、正式 Integration の明示的な検証省略が別々に記録されている。本 Sync では製品テストを再実行しておらず、無関係な過去の説明全体を再検証したとは扱わない。

## 並列活動と固定したソースを読む

`/api/repos/{id}/parallel-context` は登録 Repo の interactive Workspace、現在の Card link、実行数、観測時の branch HEAD と immutable Manifest の参照を返す。peer の checkout を復元せず common repository の branch を読む。活動と Manifest は別 snapshot であり、Manifest がないことは活動ゼロ、現在の Card link は過去 event の帰属、という意味にはならない。[discovery](../../crates/services/src/services/parallel_context.rs#L63-L131)

Workspace と Manifest は独立した cursor でページングする。既定25・最大100件で、memory 読取失敗は `memory_error` として残す。空結果と情報源の故障を区別し、他 Workspace の私的 Memory や raw conversation を参照する仕組みにしない。Wiki の target / source / publication commit は maintenance の観測情報であり、今の作業木へ最新 Wiki を配達した証拠ではない。[情報の解釈契約](../../crates/services/src/services/parallel_context.rs#L192-L198)

`/api/repos/{id}/snapshot` は指定 commit の Git object を読む。path 未指定なら一覧を100件ずつ返し、本文は通常ファイルの UTF-8・128 KiB以内に限定する。symlink / submodule を辿らず、大きな blob は理由を返して同じ固定 object の分割読取へ案内する。[snapshot API](../../crates/server/src/routes/parallel_context.rs#L43-L79)。組合せを試すときは [detached preview の操作](../workflows/task-to-integration.md#並列変更を試してから正式統合する)を使い、peer の作業木や target を変えない。

DirectFolder には自動共有リンクや peer discovery がない旨を通知する。一方、現在の repository-memory identity と Wiki 読取専用規則は別に適用する。[DirectFolder 分岐](../../crates/services/src/services/parallel_context.rs#L168-L170)

## 旧 LLM Wiki からの移行境界

旧 `.llm-wiki/` は作業 branch 内でコードと同じレビュー・統合に従う知識を意図していた。[旧設計](../../docs/superpowers/specs/2026-09-09-llm-wiki-design.md#architecture)はその経緯を記録するが、現行の生成・閲覧契約ではない。現在は repository 設定の OpenWiki へ統一され、既存 `.llm-wiki` とカードの保存内容を自動削除・移行しない。[廃止の案内](../../docs/workspaces/llm-wiki.mdx)

Pipeline loader は現在同梱定義を持たず、残存する `wikillm.toml` を表示候補から外す。他の定義は保持し、不正 TOML は警告して読み飛ばす。seed 機構も既存ファイルを上書きしない。[loader](../../crates/services/src/services/pipelines.rs#L11-L12)、[読取規則](../../crates/services/src/services/pipelines.rs#L136-L190)

新しい実行要求では、LVK が生成したと識別できる旧 Recall/Enrich 行だけを取り除き、手書き補足と他 Pipeline を残す。改変された stage や不完全な旧 marker はエラーで停止し、保存済みカードを書き直さない。同名でも利用者自身の場所にある Skill は削除対象にしない。実行時の選択から外すのは LVK の旧 `skills/llm-wiki` 配下だけで、disk 上のファイルは保持する。[互換 filter](../../crates/executors/src/legacy_wiki.rs#L14-L119)、[要求境界](../../crates/server/src/routes/sessions/agent_run.rs#L366-L375)、[保持と曖昧入力のテスト](../../crates/executors/src/legacy_wiki.rs#L132-L152)

## 閲覧と更新権限

Workspace Wiki はその checkout の `openwiki/` を読む。通常 coding は正本を編集せず、統合後の専用 maintenance が生成・更新する。Viewer の repository 解決、OKF・path・容量制限は [Workspace inspection](../operations/workspace-inspection.md#wiki-の本文ツリーと表示元を確認する)、初期生成と Sync は [OpenWiki maintenance](../operations/openwiki-maintenance.md)が一次説明先である。[現在の reader](../../crates/server/src/routes/workspaces/wiki.rs#L148-L168)
