# LOCAL-30: チャットのローカルファイルリンク

対象TODO: `4d9acc95-6c1e-4333-9bb0-2359f267bb01`
Workspace: `3700477f-ac8e-4aec-a3ef-220c43cbf3af`

## 実装と境界

元の `ReadOnlyLinkPlugin` はHTTPS以外のリンクを無効化していた。
チャットの `AppChatMarkdown` から `useChatFileLink` を渡し、既存の
`resolveWorkspaceFilePreviewTarget` / `WorkspaceFilePreviewActionsProvider` /
`WorkspaceFilePreviewPanel` に接続した。backendの変更はない。

- 相対パスはRepo基準。Repo名prefixと現在の `container_ref/repo.name` 配下の絶対形式も対応。
- `:line[:column]`、`#Lline` 等は開始行を選択。Markdownの行指定はソース表示、指定なしは既存Markdown表示。
- 空白はURLエンコードまたはCommonMarkの `<...>` 形式に対応。
- 複数Repoで曖昧なリンク、Workspace外、親参照、任意URI schemeは無効。`file://` や元のhost checkoutの `Repo.path` へフォールバックしない。
- ファイルの実在・Repo membership・symlinkのcanonical root検査は既存files APIが担当。
- HTTPSの新規タブ・noopener/noreferrerを維持。HTTP等の従来無効だったschemeは拡張していない。

## 再現コマンド（リポジトリroot、必ず逐次）

```sh
RUSTC_WRAPPER=/usr/local/bin/sccache SCCACHE_DIR=$(realpath .lvk-shared/cache/sccache) cargo build -j 2 --bin server --bin agent-process-host
pnpm --dir packages/local-web run build
pnpm run web-core:check
pnpm run ui:check
pnpm run ui:lint
pnpm run local-web:lint
packages/local-web/node_modules/.bin/eslint --config packages/local-web/.eslintrc.cjs packages/web-core/src/features/workspace-chat/ui/AppChatMarkdown.tsx packages/web-core/src/features/workspace-files/model/chatFileLink.ts packages/web-core/src/features/workspace-files/model/useChatFileLink.ts packages/web-core/src/features/workspace-files/ui/viewers/CodeTextViewer.tsx packages/web-core/src/shared/components/WYSIWYGEditor.tsx --max-warnings 0
pnpm --dir packages/web-core exec vitest run --maxWorkers=1 src/features/workspace-files/model/chatFileLink.test.ts src/features/workspace-files/model/useWorkspaceFilePreviewResolver.test.ts
node tests/chat/file-links.e2e.mjs
```

## 観測結果（2026-10-20）

上記ビルド・型検査・lintが成功。local-web build内のtscも成功。
Vitestは2ファイル・36件成功。frontend buildにはBrowserslist、Tailwind、chunk sizeの警告がある。

E2Eは自身の `target/debug/server` とビルド済みfrontendを使用。
Chromium `147.0.7727.15`、`http://127.0.0.1:39853` で成功した。

| 操作 / 入力                                                     | 期待と観測                                           |
| --------------------------------------------------------------- | ---------------------------------------------------- |
| ビルド済みアプリを開く                                          | タイトルとrootの描画を確認                           |
| relative `src/example.ts:3` をクリック                          | 既存ビューに実ファイルの3行目を表示・選択            |
| 現Workspace絶対パス `#L4` をEnterで開く                         | 同ファイル4行目を選択、新規タブなし                  |
| 空白・日本語 `<...:2>`、URLエンコード `#L2`                     | 実ファイルの2行目を選択                              |
| `README.md:2`                                                   | Markdownソースの2行目を選択                          |
| missing / 外へ向くsymlink                                       | 既存の「Could not load preview」、外部内容なし       |
| Workspace外・親参照・encoded親参照・javascript                  | hrefなし、aria-disabled=true                         |
| HTTPSリンク                                                     | 元URL、target=\_blank、rel=noopener noreferrerを保持 |
| content/rawへ直接親参照・Unix絶対・Windows driveを要求          | 各400                                                |
| content/rawへsymlink逸脱・欠落・非所属Repo・未知Workspaceを要求 | 各404、外部内容なし                                  |

初回E2Eは親環境の `LVK_PUBLIC_URL` 継承により `403 Invalid host` で失敗した。
試験プロセスだけ公開URL設定を解除しLocalモードを明示して修正後に成功。
前回のメモリ枯渇で中断した検査を合格として数えていない。

## 実backend / fixture / 未確認

HTTP/API mockは使用していない。Workspace/Repo取得、ファイル読取、content/raw境界拒否は実backend。
会話本文と画面配置はfixtureで、製品の `AppChatMarkdown` と既存viewerを同じoriginへmountする。
DB初期データのみ専用SQLiteへseedする。このfixture単独では通常layoutのpanel切替を検証しない。
下記の独立nativeケースでその配線を補完する。Agent実行による会話生成は検証していない。
Localモードの試験であり、実Basic/Auth0ログインや外部HTTPSサイトの内容取得を検証した意味ではない。

専用 `LVK_INSTANCE_DIR`、DB/data/runtime、kernel割当loopbackポート、独立ブラウザcontextを使用。
finallyおよびSIGINT/SIGTERMでserver/Chromiumを終了し、一時instanceを除去する。
今回の失敗・成功双方のserverポート閉鎖と、自己所有E2Eプロセスの残存なしを確認した。
通常の `target/`、依存パッケージ、frontend `dist/` は再利用用に保持。

## 独立レビュー補完: native layout E2E（2026-10-20）

対象sourceは `ad3317ce084482ca8f97c635a55623f6f2c07ad3`。製品コードは変更せず、
同Workspaceで既にそのsourceから作成した `target/debug/server` とlocal-web `dist/` を再利用した。
再現はビルド済みのリポジトリrootで次のfocused commandのみを実行する。

```sh
node tests/chat/file-links.e2e.mjs --native
```

`file-links.native-seed.py` は専用DBにinteractive/direct_folder Workspaceと完了済みSession/AgentRunイベントをseedする。
ブラウザは実アプリの `/workspaces/<id>?session_id=<id>` を開く。
`file-links.native.mjs` はbody置換、script注入、custom provider、HTTP/WS mock、panel状態の事前設定を一切行わない。
会話取得から通常chat renderer、`WorkspacesLayout` の
`MainWorkspaceFilePreviewActionsBridge`（openTarget + FILES切替）、
`WorkspaceFilesSurfaceContainer`、`WorkspaceFilePreviewPanel`、`CodeTextViewer` まで製品配線を通す。

| 操作                                                                   | 期待 / 観測                                                                            |
| ---------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| 独立Chromium contextで通常Workspaceを開き、初回ガイドのCloseをクリック | seedした会話の「Open native source」が表示。クリック前はpreviewなし                    |
| チャットの `[Open native source](src/example.ts:120)` をクリック       | native panelの「Close preview」が表示。実content APIは指定workspace/repo/pathに対し200 |
| 内容と行位置を確認                                                     | `const nativeSelected = 120;` と行番号120を表示、aria-current=locationが1件            |
| viewer内スクロールと新規タブを確認                                     | scrollTop=1968、選択行y=533–553がviewer y=86–1000内、ブラウザページは1件               |

成功時はloopback `127.0.0.1:33191`、Chromium `147.0.7727.15`。pageerrorは0件。
実backend・実ビルド済みfrontendを使用、mockなし。会話データのみDB seedであり、agent自体は起動していない。
初回試行は初訪問ガイドがクリックを遮りtimeout。ガイドのrole=dialogを仮定した試行もselector不一致で失敗。
実際のCloseボタン操作に合わせて修正後、上記ケースが成功した。失敗した試行をpassには数えない。

server/Chromiumを終了し、専用instance/DB/runtimeと試験repoを除去。
初回失敗で残った自己所有scratchも、該当ポート閉鎖・関連プロセス不在を確認して除去した。
失敗・成功のmainポート34357/35021/38465/33191は閉鎖済み。
製品source変更がないため全面backend build、型検査、既存全試験は再実施していない。
Basic/Auth0ログイン、agent実行、モバイルlayoutはこの追加ケースの対象外。
