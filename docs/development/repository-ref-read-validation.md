# repo/ref 読取MCP：方式AとLOCAL-207の局所検証

2026-10-27 UTC。実装・局所検証の記録であり、配備済み機能や実ChatGPT接続の検証結果ではない。
方式Aの型付き読取を `query_repository` にまとめ、stdioとremoteが同じサービスを呼ぶ。
既存Workspaceツールの既定値・出力形式を維持し、初回範囲readのrevision必須だけを緩和した。

## 境界と検証対象

- commit対象は登録・認可済みRepositoryのローカルref、HEAD、full SHAのみ。revparse式・reflog・履歴message検索式は公開しない。
- Workspace対象は自身のHEAD/index/worktree/非ignore untrackedを観測する。過去refへのdirty自動重畳はしない。metadata revisionは原子的snapshotではない。
- direct diffは非祖先も許可。merge-baseは明示指定し、共通祖先なしはエラー。単一commitは親との差分、rootは空tree、mergeは親番号を必須とする。
- cursorは元要求・対象・固定head/base/end・revision・進行位置をHMACで結び、プロセス再起動で失効する。検索は一致の終端から再開し、Unicode・隣接空一致の抑制と部分countを保持する。
- Repository/indexのファイルごとの再openを検索ループから除いた。既存SnapshotReader・旧diff・新読取は、本文展開前のblob headerサイズ判定を共有する。
- private path、Git symlink/gitlink、実体symlink、binary、巨大blobを保護する。任意host path/URL、shell、モデル呼出し、常駐索引は追加していない。

## 局所テスト

実行コマンド：

```sh
node scripts/dev-cargo.mjs test -p git -p server -p mcp --lib -j 4 -- repository_read remote_mcp::code_read snapshot::tests
```

| 対象 | 結果 | 確認した範囲 |
| --- | --- | --- |
| git | 16 passed、計測1 ignored | main用Workspaceなしの初回read/full SHA再現、dirty除外、branch更新中のcursor、logの同時刻commit、root/merge/非祖先/共通祖先なし、明示merge-base、削除、private/binary/巨大/symlink/gitlink、無効range/regex/ref、Unicode・空一致、出力/scan/count途中再開、Workspace変更・index変更検出、既存snapshot境界 |
| mcp | 2 passed | global/scoped公開、対象スコープ拒否、HTTP adapterの要求保持・正常応答・revisionエラー伝達 |
| server | 16 passed | remoteと共通サービスの結果一致、Project/Repository/Workspace所属、HTTP未認証拒否、Project境界、旧検索/read/diffの既定動作・revision・出力上限、初回行read |

最終lintも成功した（git/mcp/serverのall-targetsと、その依存consumerを検査）：

```sh
node scripts/dev-cargo.mjs clippy -p git -p mcp -p server --all-targets -j 4 -- -D warnings
```

合計34件。stdio adapterのHTTPテストはloopback fixture、remoteの比較はin-processである。
実Auth0/ChatGPTからの接続や実MCPネットワーク遅延の測定ではない。

ビルドは開始前・途中にavailable memory、memory PSI、swap I/O、並行ジョブを確認しCargo jobs=4を選択した。
共有Cargo成果物に、ソースには存在する `Access::preview_layer` が見つからないAPI不一致が再発した。
このcheckoutの依存ソースmtimeを更新して再コンパイルした後に上記は成功した。
共有キャッシュ削除・設定変更・他Workspaceの変更は行っていない。
stdioテストが発見した応答envelopeの二重展開と、lint指摘は修正して再検証した。

## 実レポ探索の計測

```sh
node scripts/dev-cargo.mjs test -p git measure_repository_exploration --lib -j 4 -- --ignored --nocapture
```

1 passed。探索対象SHAは **`a3247dadd5b4385e07a7848fd0009aa07d8ffa71`**。
探索ディレクトリとalias設定を既知とした、Kanban route→import→使用箇所のシナリオを計測した。
aliasは別途 `packages/local-web/tsconfig.json:21` と `vite.config.ts:123–124` のreadで照合したが、この計測には含めていない。

| 項目 | 実測 |
| --- | --- |
| 共通Gitエンジンの論理呼出数 | 9 |
| 応答データ量 | 27,848 bytes（エンジンValueのcompact JSON合計） |
| 経過時間 | 87 ms（単発、同一host、network/認可DB処理を含まない） |
| 探索完遂 | 全9応答でcomplete=true、exclusions_total=0 |
| 実MCP往復回数・wire量・network RTT | 未計測 |
| 旧方式との同条件比較・速度改善率 | 未計測。改善済みの数値とは扱わない |

| 呼出 | 操作 | items | JSON bytes | engine ms |
| --- | --- | ---: | ---: | ---: |
| 1 | routesでLocalProjectKanbanをgrep | 15 | 7,545 | 26 |
| 2 | Project routeの1–8行read | 1 | 1,493 | 0 |
| 3 | LocalProjectKanbanの1–5行read | 1 | 1,267 | 0 |
| 4 | ProjectKanban内のKanbanContainerをgrep | 3 | 2,330 | 21 |
| 5 | ProjectKanbanの1–15行read | 1 | 2,165 | 0 |
| 6 | ProjectKanbanの109–115行read | 1 | 1,272 | 0 |
| 7 | KanbanContainer内のKanbanBoard/KanbanGridをregex grep | 14 | 6,390 | 28 |
| 8 | KanbanContainerの50–65行read | 1 | 1,610 | 1 |
| 9 | KanbanContainerの1094–1145行read | 1 | 3,776 | 1 |

0 msはミリ秒への切捨てであり、処理時間ゼロを意味しない。
検索結果だけで使用部品を断定せず、routeのimport/`component`、wrapperのreturn、
`ProjectKanbanBoard`の`<KanbanContainer />`、そのimport元と`<KanbanBoard>`の実使用をreadで辿った。
UI描画やブラウザー操作の実行検証ではない。

## 残る制約・未確認

Workspace revisionはmetadata/indexの観測であり、変更検出時は最初から取り直す。
原子的snapshotや粗い時刻精度のfilesystemでの完全な変更検出は保証しない。
commit SHAを再現性の固定点として使う。cursorはサーバー再起動で失効する。

1 source side 512 KiB、1 range 200行、item/exclusion JSON合計64 KiB。
単一行でも64 KiBの出力枠を超えるrangeはエラーで、行readでは返せない。
一覧/検索のtree/indexとdiff pathは100,000 entriesまで、log cursorは100,000 commitsまで。
通常のscanは16 MiB・1000 files・100,000 regex match visits・2秒の協調予算を持つ。
列挙・query準備・cursor再構築はscan時計から分け、総時間を別途返す。
個別OS/libgit2 readとdiff計算を強制中断する仕組みではない。

`complete`は走査終了、`search_complete`は除外もない検索完遂である。
binary/oversized/unsafe等は各item statusやexclusionsを確認し、未探索を不存在としない。
旧literal検索は従来の終端上限を維持するため、継続検索には新toolを使う。

実Auth0/ChatGPT、remote MCP wire/stdioプロセスを含む実配置E2E、Windows・粗いmetadataのfilesystem、
長時間負荷、配布物・稼働DBでの検証は行っていない。
UI変更がないためfrontend build/全frontend検査や、無関係な全backend suiteは実行していない。
mainへのマージ、push、配備、再起動、LOCAL-207の状態変更・アーカイブ、canonical OpenWiki更新は対象外のままである。
