---
type: architecture
title: ファイル・差分・プレビュー・端末の境界
description: Workspace 内のファイル・Wiki 閲覧、添付、差分配信、dev server preview、PTY、editor 操作で異なるパス・ホスト・寿命の契約を説明する。
tags: [workspace, files, wiki, diff, preview, terminal]
sources:
  - id: openwiki-source-5aa384eaafe1283b1b8cc4a4
    resource: repo://crates/preview-proxy/src/api.rs
  - id: openwiki-source-0099d841c439c1dde012669e
    resource: repo://crates/preview-proxy/src/lib.rs
  - id: openwiki-source-c16d91d3eae6ee7bafc2593e
    resource: repo://crates/server-access/src/preview.rs
  - id: openwiki-source-a6cad3d930b299ebbbd793a3
    resource: repo://crates/server-access/src/tests.rs
  - id: openwiki-source-a9b6d4be264787a78357c9d6
    resource: repo://crates/server/src/routes/config.rs
  - id: openwiki-source-65c0f36670a161dc05040e69
    resource: repo://crates/server/src/routes/mod.rs
  - id: openwiki-source-875a0d6590ed93103083efa6
    resource: repo://crates/server/src/routes/preview.rs
  - id: openwiki-source-7c1b10f112cb99ba82426b5a
    resource: repo://crates/server/src/routes/terminal.rs
  - id: openwiki-source-75b2a3e21f7d8cbca4f8565a
    resource: repo://crates/server/src/routes/workspaces/attachments.rs
  - id: openwiki-source-bdb4dae5722fe76ec5e44001
    resource: repo://crates/server/src/routes/workspaces/create.rs
  - id: openwiki-source-babbae011de25ca5bb9ba725
    resource: repo://crates/server/src/routes/workspaces/execution.rs
  - id: openwiki-source-37945a97e2b18bbb762fafc9
    resource: repo://crates/server/src/routes/workspaces/files.rs
  - id: openwiki-source-303d74f74def684d4690dea9
    resource: repo://crates/server/src/routes/workspaces/git.rs
  - id: openwiki-source-6c4d6d22a9d50e0def61c50d
    resource: repo://crates/server/src/routes/workspaces/integration.rs
  - id: openwiki-source-af328071d51bff75b5246d3b
    resource: repo://crates/server/src/routes/workspaces/usage.rs
  - id: openwiki-source-5590f47e4cee001a35a42117
    resource: repo://crates/server/src/routes/workspaces/wiki.rs
  - id: openwiki-source-379c8cd3e1e9932f9adda9ee
    resource: repo://crates/services/src/services/container.rs
  - id: openwiki-source-b04f793d57fd8b570d4984e3
    resource: repo://crates/services/src/services/diff_stream.rs
  - id: openwiki-source-ead147478bd848437cb020e7
    resource: repo://crates/services/src/services/wiki.rs
  - id: openwiki-source-3bef0d0bfd4066b26b945f6c
    resource: repo://crates/services/src/services/wiki/openwiki.rs
  - id: openwiki-source-965b490de6f8364cb7695944
    resource: repo://crates/workspace-manager/src/workspace_manager.rs
  - id: openwiki-source-bb23b715172e6051c10e1afc
    resource: repo://packages/ui/src/components/ReadOnlyLinkPlugin.tsx
  - id: openwiki-source-69854837467c3beec23ecb8d
    resource: repo://packages/web-core/src/features/wiki/model/wikiNavigation.ts
  - id: openwiki-source-0cd94f38cc1a16e284fe2b1c
    resource: repo://packages/web-core/src/features/wiki/ui/WorkspaceWikiPanel.tsx
  - id: openwiki-source-a045ed78b54878edd07211c7
    resource: repo://packages/web-core/src/features/workspace-chat/ui/ExecutionInspectionPanel.tsx
  - id: openwiki-source-714cddfb6b8044e60077cdf6
    resource: repo://packages/web-core/src/features/workspace-files/model/chatFileLink.ts
  - id: openwiki-source-b20f21ce6e1fdbde2fcde6ed
    resource: repo://packages/web-core/src/features/workspace-files/model/useChatFileLink.ts
  - id: openwiki-source-18dc5170d4dade4bcfbc64d8
    resource: repo://packages/web-core/src/features/workspace-files/model/useWorkspaceFileContent.ts
  - id: openwiki-source-0f1df48871fe6f036c219dff
    resource: repo://packages/web-core/src/features/workspace-files/model/useWorkspaceFilePreviewResolver.ts
  - id: openwiki-source-1be0c4423a1ed260c67b498f
    resource: repo://packages/web-core/src/features/workspace-files/model/workspaceFileRawUrl.test.ts
  - id: openwiki-source-e26b48aa743da73944d9c0b1
    resource: repo://packages/web-core/src/features/workspace-files/ui/viewers/CodeTextViewer.tsx
  - id: openwiki-source-ece894e6e087af564c07e38d
    resource: repo://packages/web-core/src/features/workspace-files/ui/WorkspaceFilePreviewPanel.tsx
  - id: openwiki-source-9b9697ea8eafb71c1ca35e40
    resource: repo://packages/web-core/src/pages/workspaces/ChangesPanelContainer.tsx
  - id: openwiki-source-0894249b732085a9800940bd
    resource: repo://packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx
  - id: openwiki-source-10caf2340da819f813f5ea62
    resource: repo://packages/web-core/src/shared/components/WYSIWYGEditor.tsx
  - id: openwiki-source-e4b6c8db09a6d8887192b13e
    resource: repo://packages/web-core/src/shared/lib/previewProxyUrl.ts
  - id: openwiki-source-3b6f5885b9d50925c82267dd
    resource: repo://packages/web-core/src/shared/providers/WorkspaceProvider.tsx
  - id: openwiki-source-5ef837b2bc54d286b3dedd8f
    resource: repo://packages/web-core/src/shared/stores/useUiPreferencesStore.ts
  - id: openwiki-source-7156bd32d206a84b272e44b5
    resource: repo://packages/web-core/src/shared/stores/useWorkspaceDiffStore.ts
  - id: openwiki-source-bf19bdc211e26b0fb155fc08
    resource: repo://tests/chat/file-links.md
  - id: openwiki-source-ba7e6a69f93643706f63da9e
    resource: repo://tests/chat/file-links.native.mjs
  - id: openwiki-source-be2dcdcefb33abee0a49e75a
    resource: repo://tests/server-access/preview-static.mjs
  - id: openwiki-source-dcac74ef871fd02e6315fa93
    resource: repo://tests/workflow/specs/diff-tree.spec.ts
generated: { by: "codex", at: "2026-10-21T05:41:24.899Z" }
verified:
  - by: openwiki/0.5.1
    at: 2026-10-21T05:41:24.899Z
---

# ファイル・差分・プレビュー・端末の境界

これらの機能は同じ [Workspace](../concepts/workspace.md) の成果物を扱うが、実ファイルの閲覧、Git との差分、アプリの実行、shell の操作は別の経路である。画面の操作方法は [Reviewing Code](../../docs/reviewing-code.mdx) と [Browser Testing](../../docs/browser-testing.mdx) に譲り、ここでは実装変更に関わる契約を整理する。

## 実行専用 Workspace の確認

実行中の inspection は [用途が execution_only の Workspace](../concepts/workspace.md#操作用途と実行所有者)を表示する。通常 chat composer の代わりに、owner kind/run、状態、公開結果、ファイルの有無、保存済み Session の選択、所有者の Stop を示す。新規 chat・Session・queue・executor 選択は作らず、現在の所有者が出している承認・質問への応答を残す。Wiki 実行中には追加指示を steer でき、Integration には専用介入 UI がある。実行と cleanup 完了後は通常 composer に戻り、所有者の履歴は残る。[inspection UI](../../packages/web-core/src/features/workspace-chat/ui/ExecutionInspectionPanel.tsx)

`GET /api/workspaces/<id>/usage` と executions 一覧は既存 owner 記録から状態を投影する。Integration の Git 公開、Bootstrap の Workflow 完了、Sync の保存済み結果はそれぞれの正本から読み、Generator や AgentRun の成功を公開済みと推測しない。Sync の AgentRun が終端でも repository owner が残る間は `finalizing` である。成功 no-op は新しい Wiki commit がない成功として表示する。[状態の由来](../../crates/server/src/routes/workspaces/usage.rs)

Stop は Integration の cancel、Bootstrap の所有フロー、Sync の監査付き Cancel へ委譲する。停止可能 phase を再検証し、Wiki publication 開始後は取り消さず結果の確定を待つ。終端または停止要求済みなら重複 Stop は無操作となる。[停止の境界](../../crates/server/src/routes/workspaces/usage.rs)。失敗・回復待ちの意味は [正式統合](../concepts/formal-integration.md)と [Wiki 保守](openwiki-maintenance.md)に置く。

ファイル・Wiki・添付などの読取は `container_for_inspection` を経由する。execution_only では存在するパスだけを返し、cleanup 済みや未準備の worktree を再作成しない。一般の interactive 環境は従来の ensure 経路を保持する。表示に伴う復元や script 起動を追加する変更は、この用途の違いを守る必要がある。[共通の読取入口](../../crates/services/src/services/container.rs)

## Project の Todo を確認する

Workspace の Todo panel は作業木内のファイルではなく、現在の Card link が示す Project の共有メモを読む。未関連なら Project を選ぶ。[所属・保存・更新競合](../concepts/project-todos.md)を確認し、メモの完了を実行終了や Card Done と取り違えない。チャットを残した panel 切替の配置は [Web の状態管理](../architecture/web-and-sync.md)を参照する。

## Repo 内ファイルを読む

files API は tree / directory / content / raw の GET を公開する。repo_id は WorkspaceRepo membership で解決し、その Workspace の container_ref/repo.name を root とする。任意の filesystem path を読む API ではない。[router](../../crates/server/src/routes/workspaces/files.rs)・[root の解決](../../crates/server/src/routes/workspaces/files.rs)

パスは slash を正規化し、NUL、絶対パス、Windows drive、親への .. を拒否する。さらに canonicalize した最終 target が Repo root 内にあるか確認し、外への symlink は NotFound とする。directory の列挙でも root 外へ解決される entry を省く。[パス検証](../../crates/server/src/routes/workspaces/files.rs)・[列挙](../../crates/server/src/routes/workspaces/files.rs)・[traversal / symlink test](../../crates/server/src/routes/workspaces/files.rs)

directory は最大2,000 entries、text preview は512 KiBまでで truncated を返す。content は Text / Image / Binary / Unsupported に分かれ、NUL や不正 UTF-8 も binary 判定に使う。先頭だけの preview を file 全体と解釈しない。[上限](../../crates/server/src/routes/workspaces/files.rs)・[content 分類と読み取り](../../crates/server/src/routes/workspaces/files.rs)

raw は streaming 応答で no-store と nosniff を付ける。明示された raster image MIME のみ inline とし、それ以外は application/octet-stream / attachment で返す。HTML や SVG を raw URL で main UI の実行 content に変えない。[raw 応答](../../crates/server/src/routes/workspaces/files.rs)・[MIME policy](../../crates/server/src/routes/workspaces/files.rs)

raw URL は local のものをそのまま使い、退役した remote host ID または host-scoped URL は拒否する。host ID なしの外部 URL は保持する。[URL test](../../packages/web-core/src/features/workspace-files/model/workspaceFileRawUrl.test.ts)

## チャットから Repo 内ファイルを開く

<!-- openwiki: broken internal link [src/example.ts:120] file "src/example.ts:120" does not exist. Fix the href or restore the target, then delete this comment. -->
チャットの Markdown file link は、現在の Workspace / Repo に属するファイルを既存 preview で読むための参照である。`[source](src/example.ts:120)` を開くと file panel が選択される。リンクを別の Workspace の会話に移したときも、解決基準は表示先の Workspace である。本文 renderer から panel までの責務は [Web の接続構造](../architecture/web-and-sync.md#チャットのファイルリンクを既存-panel-へ渡す)にまとめる。[resolver の入力](../../packages/web-core/src/features/workspace-files/model/useChatFileLink.ts#L7-L23)

相対パスは Repo 基準で、`./` と Repo 名 prefix を扱う。Repo が一つなら省略できるが、複数 Repo で所属を一意に解決できなければリンクを無効のままにし、推測して開かない。絶対形式は現在の `container_ref/repo.name/` 配下だけを repo-relative target に変換する。Windows 形式も slash を正規化して同じ条件を使う。元の host checkout の `Repo.path` へ fallback しない。[チャットの変換](../../packages/web-core/src/features/workspace-files/model/chatFileLink.ts#L36-L77)、[共通の Repo 解決](../../packages/web-core/src/features/workspace-files/model/useWorkspaceFilePreviewResolver.ts#L77-L157)

<!-- openwiki: broken internal link [<path with spaces:2>] file "<path with spaces:2>" does not exist. Fix the href or restore the target, then delete this comment. -->
行指定は `:line[:column]`、`#Lline`、`#Lline-Lend` 等を受け付けるが、保存する位置は正の安全な整数の**開始行だけ**であり、column や範囲の選択を約束しない。行 suffix を外してから URL decode を一度だけ行うため、encoded `#` / `:` はファイル名として残る。空白は percent encoding または `[label](<path with spaces:2>)` を使える。後者の transformer は local resolver がある read-only editor で有効になる。[位置と decode 順序](../../packages/web-core/src/features/workspace-files/model/chatFileLink.ts#L21-L35)、[angle destination](../../packages/web-core/src/shared/components/WYSIWYGEditor.tsx#L536-L555)

text/code viewer は選択行に `aria-current=location` を付けて中央へスクロールする。Markdown は行指定がある場合にソース表示へ切り替え、指定なしでは従来の Markdown 表示を使う。描画済み本文の位置をソース行と扱わないための選択である。存在しない行や backend が切り詰めた範囲外には選択行 DOM がなく、追加読込や最終行への補正はしない。[viewer の分岐](../../packages/web-core/src/features/workspace-files/ui/WorkspaceFilePreviewPanel.tsx#L90-L108)、[行の選択](../../packages/web-core/src/features/workspace-files/ui/viewers/CodeTextViewer.tsx#L17-L48)

空の値、通常の `#section`、network path、親参照、control character、不正な percent encoding、未処理の query / fragment、現在の root 外の絶対パスや任意 URI scheme は local resolver が拒否する。HTTPS は plugin が外部リンクとして扱い、HTTP や `file://` は有効化しない。local action がなければ `href` を除き `aria-disabled=true` とする。[入力の検査](../../packages/web-core/src/features/workspace-files/model/chatFileLink.ts#L21-L55)、[リンクの有効状態](../../packages/ui/src/components/ReadOnlyLinkPlugin.tsx#L20-L58)

フロントエンドで解決できたことは、ファイルの存在や読取許可の証明ではない。target の Workspace / Repo / path を既存 content API に渡し、欠落・symlink 逸脱等の取得失敗は既存 preview error と Retry を表示する。最終的な membership と canonical-root 検査は [Repo 内ファイルの読取境界](#repo-内ファイルを読む)のままである。[取得](../../packages/web-core/src/features/workspace-files/model/useWorkspaceFileContent.ts#L13-L26)、[失敗表示](../../packages/web-core/src/features/workspace-files/ui/WorkspaceFilePreviewPanel.tsx#L65-L91)

### ファイルリンクの検証範囲

[resolver の unit test](../../packages/web-core/src/features/workspace-files/model/chatFileLink.test.ts)は相対・現在 root・Windows・行 suffix・decode・曖昧な Repo・拒否入力を扱う。[component と実 backend の E2E](../../tests/chat/file-links.e2e.mjs)は合成会話を使い、内容表示、Enter、空白・日本語、Markdown source、欠落・symlink、content/raw の拒否を検査する。これだけでは通常 layout の切替を保証しないため、`--native` は seed した保存済み会話を通常 Workspace route で開き、リンククリックから実 content API、native panel、120 行目の選択と viewport 内への scroll まで確認する。[native 試験](../../tests/chat/file-links.native.mjs#L5-L81)

[実装時の検証記録](../../tests/chat/file-links.md)には開発時の成功と失敗・修正の記録がある。今回の Sync は凍結 source `271f62441d4a7ecb7f43ff0cd77e02f0371c762f` について renderer、layout、resolver、viewer、backend と試験定義を静的に照合した。正式 Integration は明示的な検証省略と凍結入力に記録されており、この Sync で製品テストを再実行したことや過去の無関係な説明を再検証したことを意味しない。agent による会話生成、実 Basic/Auth0 login、モバイル layout はこの native 試験の検証範囲外である。

## Wiki の本文・ツリーと表示元を確認する

Wiki は `openwiki/` の読取専用表示に統一され、repository 選択・検索・ページツリーと本文を持つ。旧 `.llm-wiki` の形式選択はなく、残っている旧ファイルも読み替えない。OpenWiki の作成・Sync は repository memory settings から行い、reader は初期化や破損修復をしない。[形式と操作の分離](../../packages/web-core/src/features/wiki/ui/WorkspaceWikiPanel.tsx)

表示は **Current workspace**、すなわち選択 repository の現在の worktree であり、未公開の変更も含む。本文 header の repository、Workspace 名、branch、ページ path を確認する。別 worktree の成果物や未公開 Wiki を「target branch へ公開済み」と取り違えない。ファイル変更後は `Reload Wiki files` で再取得する。[表示元と再読込](../../packages/web-core/src/features/wiki/ui/WorkspaceWikiPanel.tsx)、[API の snapshot source](../../crates/server/src/routes/workspaces/wiki.rs)

ツリーは公開された Wiki snapshot のページだけから組み立て、index を入口にする。検索は本文、タイトル、summary、tags 等へ空白区切りの全 term を適用する。OpenWiki の階層内相対リンクや `/openwiki/...`、`[[wikilink]]` は既知ページへ解決するが、Wiki root 外の source path を任意ファイル読取へ変換しない。ページ切替はパス単位であり、リンクの fragment を保持した節スクロールまでこの resolver が保証するわけではない。[ツリーと検索](../../packages/web-core/src/features/wiki/model/wikiNavigation.ts)、[リンク解決](../../packages/web-core/src/features/wiki/model/wikiNavigation.ts)

本文は既存の `MarkdownPreview` を再利用する。desktop では Wiki アイコンまたは右サイドバーの `Open Wiki` で右メインペーンに開き、既定では左に会話を残す。同じアイコンで閉じると右ペーンは未選択になり、以前の Changes / Logs へ戻す専用の復帰操作はない。他のペーンへ直接切り替えることもできる。表示切替で [Session / AgentRun](../concepts/session-and-agent-run.md) を作り直すものではない。[renderer](../../packages/web-core/src/features/wiki/ui/WorkspaceWikiPanel.tsx#L332-L345)・[共通ペーン操作](../../packages/web-core/src/shared/stores/useUiPreferencesStore.ts#L561-L607)

保存済み設定で会話が非表示なら、Wiki を開くだけでは表示に戻らない。共通チャット表示操作、または右ペーンを閉じる操作で戻す。右サイドバーを隠していた場合も Wiki が強制的に開き直すことはない。モバイルでは Wiki タブからツリーと本文を開き、会話には Chat タブで戻る。これは desktop の右ペーン選択を解除する操作とは異なる。保存値の由来を識別できない制約、共通 layout の補正、下書き保持と試験範囲は [通常ペーン操作と保存済み設定](../architecture/web-and-sync.md#通常ペーン操作と保存済み設定)、共有 reader の責任は [Wiki の状態管理](../architecture/web-and-sync.md#wiki-のナビゲーションと本文を一つの状態で動かす)にまとめる。[チャット表示操作](../../packages/web-core/src/shared/stores/useUiPreferencesStore.ts#L534-L552)・[閉じた後の layout](../../packages/web-core/src/pages/workspaces/WorkspacesLayout.tsx#L199-L210)・[モバイル入口](../../packages/web-core/src/shared/components/ui-new/containers/NavbarContainer.tsx#L173-L183)

adapter は `openwiki/` 以下の階層 Markdown を読み、dot-prefixed entry と `INSTRUCTIONS.md` をページから除く。root・子 entry の symlink を拒否し、index を含め最大2,000ページ、1ページ512 KiB、合計16 MiBを超えれば snapshot 全体をエラーにする。切り詰めた Wiki を正常応答にはしない。frontmatter は optional だが、ある場合は正しい YAML mapping を要求する。Claims 解釈・設定書込・初期化・修復はしない。[adapter](../../crates/services/src/services/wiki/openwiki.rs)、[安全な読取](../../crates/services/src/services/wiki.rs)、[旧ファイル保持と容量の test](../../crates/services/src/services/wiki/openwiki.rs)

通常 Repo は membership を要求する。Repo 未接続の DirectFolder だけは `repo_id=workspace.id` で既存フォルダーを読める。旧機能の終了と Card の保存文脈は [Card context](../concepts/card-context-and-llm-wiki.md)、生成物の公開条件は [OpenWiki 保守](openwiki-maintenance.md)を参照する。[scope の解決](../../crates/server/src/routes/workspaces/wiki.rs)

## Session の添付

Session の添付は local File を作業先へ渡す機能である。Board 上の file 参照との関係は [Issue の共同作業](../concepts/issue-collaboration.md) を参照する。

添付 upload は保存レコードを作成し、Session の作業先へ file ID でコピーする。raw 添付の取得は Session が Workspace に属するか確認し、その agent_working_dir または Workspace root 配下の .vibe-attachments を使う。最終 canonical path が添付ディレクトリから外れれば拒否する。Repo file viewer と添付の基準 root は同じとは限らない。[upload](../../crates/server/src/routes/workspaces/attachments.rs)・[添付のパス](../../crates/server/src/routes/workspaces/attachments.rs)・[Session scope](../../crates/server/src/routes/workspaces/attachments.rs)

Workspace 作成は明示された attachment_ids を関連づけ、linked_issue があれば local の Issue にリンクする。Cloud 添付を取得する経路ではない。[作成処理](../../crates/server/src/routes/workspaces/create.rs)

## 差分は現在の比較基準から再構成する

Git diff は専用の local WebSocket 経路を持つ。diff stream は現在の base commit と worktree から差分を計算し、Repo ID と複数 Repo 用 prefix を付けて replace patch を配信する。WorkspaceRepo の target_branch が変わったら base を再計算して stream を reset する。[API](../../crates/server/src/routes/workspaces/git.rs)・[snapshot](../../crates/services/src/services/diff_stream.rs)・[target 変更](../../crates/services/src/services/diff_stream.rs)

stats-only と累積本文容量の上限により、diff の old/new content を省くことがある。本文がないことと変更がないことを同一視しない。追加・削除行数は可能なら省略前に計算して保持する。[省略の契約](../../crates/services/src/services/diff_stream.rs) レビュー後に変更を統合する手順は [Task から統合まで](../workflows/task-to-integration.md) に続く。

### 差分の未取得・エラー・空を区別する

差分 panel と右側 file tree は、初期 loading、取得失敗、正常に取得した空差分を別の状態で表示する。通信失敗を「変更なし」やゼロ行の成功と解釈しない。再接続中に同じ対象の最後の snapshot が残る場合は、古い表示であることとエラーを示す。[差分 panel](../../packages/web-core/src/pages/workspaces/ChangesPanelContainer.tsx)、[file tree](../../packages/web-core/src/pages/workspaces/FileTreeContainer.tsx)

`WorkspaceProvider` が Workspace / host の切替と unmount に合わせて共有差分 store をリセットし、前の対象の snapshot・error・初期化状態を残さない。store 自体が host ID を保持して選別する構造ではない。既存ファイルを見られることや cached diff があることを、現在の target での検証・統合成功の証拠に使わない。正常な empty と未取得・再接続の見分けは [実 component fixture](../../tests/workflow/specs/diff-tree.spec.ts)で確認する。[scope 境界の処理](../../packages/web-core/src/shared/providers/WorkspaceProvider.tsx)、[store の初期値と reset](../../packages/web-core/src/shared/stores/useWorkspaceDiffStore.ts)

## Preview と開発サーバー

Dev server の開始は Workspace 内で既存 dev server の停止を試み、script のある各 Repo を Bash の ScriptRequest / DevServer として起動する。script が一つもなければ API error。途中の起動に失敗した場合、それ以前の起動を全件 rollback する構造ではない。[起動処理](../../crates/server/src/routes/workspaces/execution.rs)

Dev server の稼働だけではチャット送信をブロックしない。専用停止と他 script の活動判定は [Session の入力契約](../concepts/session-and-agent-run.md#会話表示と入力)を参照する。

この script 実行は [coding AgentRun](../concepts/session-and-agent-run.md) とは別の ExecutionProcess 経路である。通常の設定とログ上の URL 検出の説明は [Browser Testing](../../docs/browser-testing.mdx) を参照する。

Preview proxy は main app とは別 port で iframe content を提供し、{port}.localhost から localhost の指定 port へ転送する。旧 remote host UUID 入り label は拒否する。[host 解決と回帰テスト](../../crates/preview-proxy/src/lib.rs)

サーバー配布では `VK_PREVIEW_DOMAIN` を backend の system info から UI へ渡し、HTTPS の `<port>.<domain>` を使う。proxy の loopback redirect も同じ公開 origin へ読み替える。公開port・HTTPS ingress・main origin禁止の詳細は [サーバーコンテナーの preview](server-container.md#preview-は別-origin)に置く。[domain の取得](../../crates/preview-proxy/src/lib.rs)、[公開 redirect の test](../../crates/preview-proxy/src/lib.rs)

この target は port と host の routing 情報である。Repo membership を検証する file API とは異なり、preview route は target_port を受け取る。アクセス制御を追加するときは、見えている Workspace の識別子だけで proxy の許可が決まると仮定しない。[preview route](../../crates/server/src/routes/preview.rs)

subdomain proxy は HTML を読み替え、head に React 検査用 bundle、body に DevTools / click-to-component 等を挿入する。CSP、X-Frame-Options などの header も除外する。Eruda の読み込みには CDN 依存がある。これは開発 preview の加工経路であり、file raw の MIME policy と同じ規則ではない。[header policy](../../crates/preview-proxy/src/lib.rs)・[HTML 加工](../../crates/preview-proxy/src/lib.rs)

一方 /api/preview/{port} の HTTP 転送は localhost upstream を呼び、失敗時は BadGateway、応答は streaming で返す。subdomain 側の injection と同じ責務を重ねない。loopback redirect の公開 URL への変換には focused test がある。[API 転送](../../crates/preview-proxy/src/api.rs)・[応答](../../crates/preview-proxy/src/api.rs)・[redirect test](../../crates/preview-proxy/src/lib.rs)

### 公開 preview への認可委譲

公開Basic/Auth0配置ではpreview listenerの専用認証層を通す。未認証GETはprojectへ転送せず認可案内を返す。利用者はmain LVKでログインし、対象preview originと開始ブラウザーに結び付いたgrantを承認する。grantは60秒・一度限りで、URLではなくform POSTで`/.lvk-preview/exchange`へ送る。交換はbrowser cookie・送信Origin・対象origin・親Web sessionを照合する。[認可と交換](../../crates/server-access/src/preview.rs#L97-L186)、[一度だけの消費](../../crates/server-access/src/preview.rs#L326-L361)

preview側にはSecure / HttpOnly / SameSite=None、Domainなしの`__Host-lvk_preview`を発行する。このcookieはLVK業務APIの認証には使えず、別のport subdomainにも流用できない。親Web session・active member・設定revisionに依存し、logoutや無効化、期限、設定変更で効力を失う。projectへ送る前にAuthorization、内部token、LVK/preview認証cookieを除去する。[cookie](../../crates/server-access/src/preview.rs#L87-L96)、[親の照合](../../crates/server-access/src/preview.rs#L187-L215)、[転送前の除去](../../crates/server-access/src/preview.rs#L279-L305)

対象は設定preview domainの正規化された数値port subdomainに限定する。1024未満と3000・3001・4020・4021・8443・8444を拒否し、main originと同じURL portを要求する。main hostをpreview domain内へ置けない。これはWorkspace/Repo所属の認可ではなく、信頼チーム内の開発portへの委譲である。UIのURL生成やnginxの除外集合はこのbackend検査の代わりにならない。[対象検査](../../crates/server-access/src/preview.rs#L28-L58)

SSEは親sessionのleaseで止まり、WebSocketはserverからproxyへ`ConnectionLease`を渡して失効時に転送taskを終了する。Localモードではこの公開handoffを要求しない。第三者cookieが拒否される場合は独立タブで認可・表示する。[専用gate](../../crates/server-access/src/preview.rs#L216-L324)、[WS配線](../../crates/server/src/routes/preview.rs#L112-L128)、[proxyの終了](../../crates/preview-proxy/src/lib.rs#L431-L440)

[handoff回帰テスト](../../crates/server-access/src/tests.rs#L1554-L1737)は別ブラウザー・再利用・別origin・業務APIへの流用を拒否し、logout後の拒否を確認する。実socketのlease終了は [proxy test](../../crates/preview-proxy/src/lib.rs#L1304-L1379)を参照する。別の [静的配信fixture](../../tests/server-access/preview-static.mjs)は、ビルド済みVite previewの`/@fs`・`/@vite/client`から非公開ファイルや開発用endpointが露出しないことをHTTP fetchで検査する。このfixtureはブラウザーでのhandoffや実Auth0接続を検証しない。

## Terminal と editor

Terminal は interactive 用途を要求してから Workspace directory を検証し、Repo が一つならその下、複数なら Workspace root を初期 cwd とする。WebSocket 接続ごとに PTY session を作り、input/output は base64、resize は別 command として扱い、接続終了時に PTY を閉じる。会話の Session を resume する経路ではない。[cwd と起動](../../crates/server/src/routes/terminal.rs)・[通信と終了](../../crates/server/src/routes/terminal.rs)

editor は path の解決と外部 editor の起動を担う。実行専用 Workspace は editor path の GET も middleware で拒否し、SSH tunnel 開設の認可に転用させない。[用途 guard](../../crates/server/src/middleware/model_loaders.rs)file_path 未指定で Repo が一つならその Repo を開き、指定された file_path は Workspace path へ join する。現行 editor resolver は file viewer の normalize/canonical-root 検査を再利用していない。この差を消す変更では、既存の editor path 契約を先に確認する。[editor path](../../crates/server/src/routes/workspaces/integration.rs)

旧 remote host の SSH tunnel / editor relay は現行 router に存在しない。local editor の導入手順は [VSCode Extension Integration](../../docs/integrations/vscode-extension.mdx) を参照する。外部 extension の公開版互換性は checkout だけでは確定しない。

## 実行履歴を調査・保全する

画面の会話履歴や process logs だけでは provider の元フレームをすべて保持した証拠にならない。[Native Audit の保存先・version 照合・bundle エクスポート](../architecture/agent-runtime.md#native-audit-の保全エクスポート寿命) は再現調査の一次説明である。Workspace の明示削除は Session の Audit ファイルも background 削除するので、保全の判断は [Workspace の削除 lifecycle](../concepts/workspace.md#archive削除期限-cleanup) と合わせて行う。[削除対象](../../crates/workspace-manager/src/workspace_manager.rs)
