# LOCAL-330: INSTRUCTIONS整理のレビューと反映待ち事項

本ファイルは今回の変更をレビューするための資料であり、Wikiや保守writerに読み込ませる指示ではない。
canonical Wikiは変更していない。LVK版の候補は `assets/openwiki-instructions-lvk.proposed.md`。
共通テンプレート `assets/openwiki-instructions.md` だけが既存の製品入力への変更である。

## 確認した原文と課題

2026-11-06、HEAD `262317d217a44aa047c14a874ae17293d8806957` の91行の原文を確認した。
Issue転載と作業木原文の一致は検証コマンドで確認する。
原文SHA-256: `56744793c91c92f2f85c877d7110839dec3654407fc5a80cb81f40ec0a6d01b1`。

原文は目的・根拠・所有権・保守権限が見出しなしで並び、後半で根拠規則、運用項目、vendor案内、保守制約が再登場する。
重要な禁止事項はあるが検索・参照しにくい。共通テンプレート末尾のLVK記述はホスト制約であり、LVKリポジトリ固有のdocs廃止方針とは適用範囲が違う。
原文には全ページの導入、段落構成、用語への導線、見出し変更時のリンク保全の方針が明示されていない。

## 構成と適用範囲

共通部分を Purpose and readers / Evidence and uncertainty / Structure, terminology and links /
Documentation ownership and operations / LVK-managed maintenance boundary に分けた。
LVK版はこの共通部分に LVK repository documentation policy を加え、所有権、運用上の区別、根拠・lifecycle・受入をまとめる。

「LVK-managed maintenance boundary」はこのテンプレートを供給するLVKホストの既存制約であり、新たな他リポジトリ用レイアウト規約ではない。
他リポジトリは自分の文書所有権・移行方針を適用できる。docs廃止、別サイト禁止、LVK認証・Backup・Runner・Runtimeの具体項目はLVK版だけに置いた。
用語集の新設や特定のページ階層は強制しない。

## 旧規約からの対応表

行番号は固定原文のもの。共通見出しは両候補で同じ。

| 旧行 | 新しい説明先 | 統合・整理の理由 |
| --- | --- | --- |
| 1–4 | Purpose and readers、Documentation ownership and operations | 主目的と運用ページの必要情報を分離。sourceから読めることを手順省略の理由にしない規約を維持 |
| 6–11 | Purpose and readers | 優先対象・避ける内容を同じ場所に保持 |
| 13–19 | Evidence and uncertainty | source/tests/configの優先、古い文書、Manifest/Memory、Design Notes、不明な理由をまとめる |
| 21–25 | Structure, terminology and links、Documentation ownership and operations | 安定パスと所有権・移行方針を分類。README編集禁止は保持 |
| 26–27、75–76のvendor部分 | Documentation ownership and operations | 同じ意味の反復を一つに統合。ローカル設定キー・契約の説明義務を保持 |
| 29–33 | LVK-managed maintenance boundary | 専用Workspace、通常編集禁止、scheduled updater/provider/API key禁止、生成workflowの証拠除外、原文保持を維持 |
| 35–44 | LVK repository documentation policy / Ownership and stable entry points | LVK限定と明示。docs/・別サイトの復活禁止、退役文書への依存禁止、README/Wikiの責任とパス保持を維持 |
| 46–48 | 共通のDocumentation ownership and operations、LVKのOperational distinctions to preserve | 手順の共通要件は一箇所、LVKのself-contained要件は参照して保持 |
| 50–53 | Operational distinctions to preserve / Authentication | 全項目保持 |
| 54–58 | 同 / Backup and restore | 全項目保持。停止backup後の同一deployment再開とhealth/HTTPS確認、blind rollback禁止も保持 |
| 59–62 | 同 / Runner | 全項目保持。切断・取消・timeoutは終了証明ではなく、独自unlock/replay/recoveryを創作しない |
| 63–66 | 同 / Runtime | file/execute、roots/sandbox、Workspace、defaults、実行account/config-directory、discovery/overrides/readiness/成功を保持 |
| 67–71 | Evidence and uncertainty、Operational distinctions to preserve / Feature constraints and rationale | 歴史・動機の扱いを根拠節へ統合。機能制約はLVK節でも保持。不明な動機と不足手順の報告を区別 |
| 73–75の根拠部分、76–77の機密部分 | Evidence, lifecycle and acceptance | integrated source/Manifest、Design Notes、credentialの複製・証拠露出禁止を保持 |
| 79–84 | Evidence, lifecycle and acceptance | pages/frontmatter sources/Claims、正規lifecycle、管理情報手編集禁止、知識保全、正確な範囲・除外・gap・限界、完了/公開/意味受入の区別を保持 |
| 86–91 | Evidence, lifecycle and acceptance | 専用writerのみ、source/tests/README/本ファイル/共有環境への権限非付与、独断commit/公開・破壊的復旧禁止、Review/host safety/source drift/再生成時保持を維持 |

安全制約や運用項目の削除はない。重複表現は参照または同じ見出しへの統合にした。
短さ自体を目標にしていないため、見出しと採用済み執筆方針の追加で全体行数は増える。

## 意図的な追加と変更していない判断

次は単なる言い換えではなく、Issueの実装開始合意で採用済みの執筆方針の追加である。

- 全ページで人間にも理解できる構成とし、冒頭に主題・読む時機を示す。
- 概要から詳細、一段落一論点、無関係な主張を一文に詰めない、条件・手順には適切なリストを使う。
- 用語初出時は既存の説明先へリンクし、必要な場合だけ言い換える。用語集新設は必須にしない。
- 廃止設計を経緯として分け、見出し変更時の参照修正・互換アンカーを考慮する。

LVK版では受付/実行/終了の区別を受入節にも明記した（原文ではRunnerの具体項目に存在）。
安全境界の変更、強制的な全ページ改稿、新たな承認gate、host prompt重複追加は行っていない。
LOCAL-331のページ指定更新機能やLOCAL-332のMemory削除を先取りしていない。両Issueの状態も変更していない。
未確定なのは、ユーザー所有INSTRUCTIONSを変更する権限・経路の確定と、後続保守の時期・対象範囲である。

## 現行実装との整合と正確なblocker

`crates/services/src/services/openwiki.rs` の `host_prompt` は既にsource/tests/config優先、Manifest/Memoryの非命令扱い、Design Notes補助、根拠のない理由の創作禁止を注入する。
`sync_prompt` はさらに全Manifestの読取り、未統合Memoryの現行仕様化禁止、根拠に応じた依存調査、正確なWikiのno-opを規定する。新しい注入機構は不要。
共通テンプレートは同ファイルの `publication_paths` でINSTRUCTIONS未存在時の期待値にも使われる。
既存INSTRUCTIONSがある場合はsource commitの正確なbytesが期待値であり、テンプレート変更は既存ファイルを上書きしない。
内容差分は `OpenWiki changed user-authored INSTRUCTIONS.md; publication refused` で拒否される。

このWorkspaceの `AGENTS.md` は通常codingからuser-owned INSTRUCTIONSも編集禁止としている。
さらに現在の実行コンテキストはcanonical openwiki/全体をread-onlyとし、保守ツールの実行を禁止している。
そのため候補をcanonicalへコピーしたり、別worktree・HTTP・別MCP経由でguardを回避したりしていない。
既存writerはユーザー所有指示の編集ルートではない。確認した保守経路にこの変更の例外はなく、今回は正規Syncも起動できない。
これはSyncの失敗やno-opを観測したという意味ではなく、未実行である。

## 残る最小手順と反映確認

1. ユーザー所有INSTRUCTIONSの変更を許す実行コンテキストと担当を明示する。必要なら運用側でその権限を整える。writerの保持guardは変更しない。
2. その担当が適用時点の最新原文・テンプレート・promptと候補を再照合し、承認された通常の指示ファイル変更としてLVK候補を適用する。マージ・公開許可は別に扱う。
3. 承認された統合先へ反映後、別工程の専用maintenance writerがそのsourceを基準に正規lifecycleを実行する。INSTRUCTIONSを保守中に改変しない。現在の担当が独断でmain反映や共有環境変更をしない。
4. 許可された正規Syncで結果を確認する。`PendingChanges` はbegin-noopを許す。既存の `CurrentSource` / `SinceCommit` は明示照合用で `force=true` とfinishを要求するが、ページ指定機能ではない。今回は全Wiki改稿を依頼せず、必要範囲・時期は332後の方針に合わせる。
5. 受付とwriter終了、finish/sourceChanged、公開receipt、実際の内容の受入を別々に確認する。no-opや指示ファイルだけの変更を既存ページ改善の証拠にしない。

## 代表ページの評価（読み取りのみ）

対象: `openwiki/concepts/formal-integration.md`。ページ本文・frontmatter・Claimsは変更していない。

| 観点 | 現在の評価と候補方針での確認点 |
| --- | --- |
| 冒頭 | 主題と実装責務は説明済み。読む時機が明示されず、現行の概要と撤去済み設計が同じ段落にある。保守時には「自動マージの採用commit、検証、公開、失敗後の状態確認を調べる際に読む」のような導入が候補になる |
| 段落 | 「選択するのは commit」冒頭は選択、受付条件、event帰属、再送条件を一段落で説明。契約を省かず論点ごとに分けられる |
| 用語 | local target、host、Manifest、receipt等が初出で集まる。既存概念ページの該当節を調べてリンクする余地がある。新用語集を要求しない |
| 歴史とリンク | 旧見出しを旧参照用に残している点は保持方針と整合。「競合だけを有限の Agent job にする」を単に削除しない。READMEの入口パスも維持する |
| 意味保持 | 固定commit、受付時検証方針、writer収束、code/wiki SHA、公開とDoneの区別、journal・復旧限界を編集後も照合する必要がある。今回は製品仕様全体の再検証ではない |

以上は現ページの文章評価であり、生成後の品質確認ではない。冒頭の例は編集候補であり、未適用の仕様説明を現行事実として認定しない。

## 検証範囲

文書変更なのでbuildや製品テストは不要。原文のGit blob一致、候補の共通部分一致、既存のINSTRUCTIONS参照パス、候補とレビュー資料の存在、canonical Wiki差分なし、diffの空白を確認する。
旧91行全体は上表と候補を目視照合した。新たな見出し参照リンクやページ移動は導入していない。
アプリ実行、writer生成、実Wiki反映・公開・全リンクの網羅検査は未実行であり、意味的完全性や全ページ改善の成功とは報告しない。
