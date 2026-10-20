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

| 操作 / 入力 | 期待と観測 |
| --- | --- |
| ビルド済みアプリを開く | タイトルとrootの描画を確認 |
| relative `src/example.ts:3` をクリック | 既存ビューに実ファイルの3行目を表示・選択 |
| 現Workspace絶対パス `#L4` をEnterで開く | 同ファイル4行目を選択、新規タブなし |
| 空白・日本語 `<...:2>`、URLエンコード `#L2` | 実ファイルの2行目を選択 |
| `README.md:2` | Markdownソースの2行目を選択 |
| missing / 外へ向くsymlink | 既存の「Could not load preview」、外部内容なし |
| Workspace外・親参照・encoded親参照・javascript | hrefなし、aria-disabled=true |
| HTTPSリンク | 元URL、target=_blank、rel=noopener noreferrerを保持 |
| content/rawへ直接親参照・Unix絶対・Windows driveを要求 | 各400 |
| content/rawへsymlink逸脱・欠落・非所属Repo・未知Workspaceを要求 | 各404、外部内容なし |

初回E2Eは親環境の `LVK_PUBLIC_URL` 継承により `403 Invalid host` で失敗した。
試験プロセスだけ公開URL設定を解除しLocalモードを明示して修正後に成功。
前回のメモリ枯渇で中断した検査を合格として数えていない。

## 実backend / fixture / 未確認

HTTP/API mockは使用していない。Workspace/Repo取得、ファイル読取、content/raw境界拒否は実backend。
会話本文と画面配置はfixtureで、製品の `AppChatMarkdown` と既存viewerを同じoriginへmountする。
DB初期データのみ専用SQLiteへseedする。Agent実行・会話永続化・通常layoutのpanel切替までは検証していない。
Localモードの試験であり、実Basic/Auth0ログインや外部HTTPSサイトの内容取得を検証した意味ではない。

専用 `LVK_INSTANCE_DIR`、DB/data/runtime、kernel割当loopbackポート、独立ブラウザcontextを使用。
finallyおよびSIGINT/SIGTERMでserver/Chromiumを終了し、一時instanceを除去する。
今回の失敗・成功双方のserverポート閉鎖と、自己所有E2Eプロセスの残存なしを確認した。
通常の `target/`、依存パッケージ、frontend `dist/` は再利用用に保持。
