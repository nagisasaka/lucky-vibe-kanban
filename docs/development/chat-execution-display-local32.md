# LOCAL-32 実行表示改善の調査・実装契約

調査基点: `271aa60f14dd1ac94b2cf2a9fe0e1878bef3d2ce`。対象は Codex / Claude Code。これは作業記録であり、実装・試験完了の宣言ではない。

## 根拠と既存レビュー

LOCAL-32 の revision `2026-10-25 09:45:33.680` を実行範囲とする。既存コメント `90c0252d-e360-43a5-b1b0-930fb42633db`（Codex）、`18717a3d-00b7-4449-9bd1-d80a5b71accf`（Claude）を全文確認した。Codexの検査対象SHAは今回の基点と一致する。Claudeは古い `eda9ff3` も参照しているため、指摘を現コードで再確認する。古いコメントのGemini対象案・途中の人間判断待ちは最新のユーザー決定へ持ち込まない。外部Claudeへの追加依頼はしない。

参照TUIは Codex CLI `0.163.0`、公式コミット `83086188ae0614bd882818ff1aa23f3194962012` に固定する。ローカルCargo checkoutのHEADを照合し、以下の公式ソース・テストを読んだ（テスト実行とは区別する）。LVKのwire型は `rust-v0.144.1`。表示できる情報はLVKの実受信イベントで決まり、TUIは表示構成の参考とする。wire依存更新はこの改善の前提にしない。

- [compact.rs](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/tui/src/exec_cell/compact.rs): 短いコマンド、状態、保持出力末尾、空出力、詳細の有無。成功・空出力の端末追加操作だけを省略する条件がある。
- [transcript.rs](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/tui/src/exec_cell/transcript.rs) / [transcript_tests.rs](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/tui/src/exec_cell/transcript_tests.rs): 詳細のコマンド、保持出力、非ゼロ終了値、取得済み時間。グループ化後も端末入力を保持し、private reasoningを混ぜない。
- [live_output_tests.rs](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/tui/src/exec_cell/live_output_tests.rs): 容量上限、UTF-8境界、分割CRLF、ANSIを含む長文の保持と省略。

PTY観測・実provider接続はまだ実施していない。スクリーンショットから仕様を推測していない。

## 現コードのイベント対応表

正規経路は `provider_adapter.rs` → canonical event / live event → `canonicalAgentConversation.ts` → `DisplayConversationEntry.tsx` / 集約UI。旧provider個別のnormalize処理だけを直して完了とはしない。

| 情報 | 現状と問題 | 改善の境界 |
| --- | --- | --- |
| Codex command / cwd / commandActions | canonical引数に保持、Shell commandだけを実行表示へ変換 | 見出しは決定的に生成し、原文は詳細・コピー用に保持 |
| Codex aggregatedOutput / exitCode / durationMs | 結果に保持、durationMsは実行表示で未使用 | 取得済み時間とLVK観測時刻を区別する |
| 出力差分 | live配信、完了結果で収束。再接続時の全量復元保証なし | 未取得・途中を空出力と言わない |
| Codex reasoning summary | 完成summaryはThinking、live summaryは表示不足 | providerの公開summaryのみ対象。private contentは表示しない |
| Codex Plan | Thinkingに投影され意味の区別が失われる | 計画本文と内部の実行状態を分ける |
| Claude tool_use / tool_result | 開始時の名前・引数を結果イベントのunknown_tool / nullで置換する経路 | 同一操作ID・同一attemptの開始情報を保持して結果と結合 |
| Claude nested stream_event / thinking | canonical投影不足。旧claude.rsの対応だけでは正規経路の保証にならない | fixtureで受信可能な本文と未投影を確認する |
| キャンセル | Codex cancelled/canceledをDeniedへ変換 | 拒否・キャンセル・不明を区別する。結果のない終了を成功にしない |
| 集約状態 | 活動中の項目がなければ常にsuccess | 失敗等を折り畳んでも見落とさない |
| 個別時間 | entry.timestampを開始・終了の両方に渡す経路がある | 不明な時間を0秒で補完しない。ChatElapsedTime既存の不明値抑制は維持 |
| 任意通知 | 監査に保存されても一般表示へ未投影の場合がある | 未受信と同一視しない。内部通知の全件羅列は避ける |
| provider未提供の思考・時間 | 復元不能 | 生成・翻訳・追加LLM要約をしない |

## 表示・受け入れ契約

基本は「短い操作見出し＋必要な結果数行＋詳細」。通常結果は末尾を少数行プレビューし、失敗の状態と終了コードは折り畳んでも見えるようにする。反復操作を集約する場合も、異なる終端状態を成功へ変換しない。実行中、承認待ち、拒否、キャンセル、結果不明を意味に沿って区別する。

詳細は受信・保持された内容を表示する。原コマンド・出力は短縮用文字列と分け、空白、改行、バックスラッシュをコピーで保持する。未取得・途中・判明している省略を明示し、「全文」で未取得の出力まで保証しない。既存Prism・テーマ・未知言語と長文のプレーン表示を再利用する。

providerの説明・計画は本文と出所を保持し、内部の「応答を準備中」等の状態から分ける。英語本文の表示経路に翻訳はなく、providerの本文を保持する。providerが英語を選んだ理由はLVKから判断できない。追加カードや概念をむやみに増やさず、既存の展開を活用する。

個別時間は証拠があるときだけ表示し、詳細で取得済み終了時刻を確認できるようにする。Workspace累積時間で代用しない。操作IDに加えattempt境界・再接続を検証し、過去の開始情報やlive差分を別の操作へ混ぜない。

## 検証と残作業

1. canonicalイベントfixtureでprovider差、並列ID、終端状態、開始情報保持、再接続、説明・計画の投影を再現する。
2. UIで成功、失敗、拒否、キャンセル、不明、実行中、空出力、長文、未知言語、展開・原文コピーを確認する。変更する共通表示は対象外providerの既存動作も回帰確認する。
3. 影響パッケージの型検査・lintと必要なRust試験を選ぶ。重い実行前に資源確認し、同じ検査は変更理由なく繰り返さない。
4. 独立fixture/seedのプレビューを空き枠で用意し、デスクトップ/狭い画面、ライト/ダーク、ストリーミング、入力/スクロールを確認する。共有資源は必ずcoordinationを使う。
5. 実装者とは別の新規コンテキストで技術レビューし、修正・再確認する。dotへ対象HEAD、技術レビュー、画面証跡、URL、操作方法を渡す。dotの視覚レビューを自称完了にしない。
6. 最後にユーザーへ独立プレビュー確認を提示する。マージ・デプロイ・稼働dev再起動はこのGoalに含めない。

## 実装時の判断と現時点の証跡

Codex mapperをv6、Claude mapperをv4へ進め、旧版の投影を残した。Codexでは明示的なキャンセル、未知状態、非ゼロ終了値を正規状態へ保持する。公開summaryとPlanは種類とprovider item IDを持つextensionへ投影し、private contentは投影しない。Claudeの完成したtext/thinking/toolブロックは本文と順序を保持する。signatureやredacted thinkingは表示しない。旧Claude v3のテストは明示的に旧mapperを選び、従来の投影を検証する。

Claudeのnested stream_eventは監査に残るが、今回はその途中JSONやthinking差分を新たに表示せず、完成した内容ブロックを使う。部分引数を完成操作と誤認させず、完成ブロックとの重複表示も避けるためである。これはprovider未提供という主張ではない。Codexの公開summary差分は既存live経路から表示し、同じattempt/itemの完成summaryへ収束させる。切断時にはlive差分を捨て、復元できると表示しない。

操作の詳細は開始時の名前・引数を保持して結果へ結合し、attempt境界をキーに含める。承認待ち・承認済み開始待ちを実行中と混同しない。詳細には入力、原コマンド、保持出力、取得済みcwd・終了情報を表示する。通常プレビューは末尾3行。失敗時は診断語を含む最初の箇所の前後と末尾を最大数行表示し、結果の途中にあるエラーも見落としにくくする。この選択で状態判定や追加LLM要約は行わない。短縮用文字列を原文・コピー元へ戻さない。

個別時間は観測した開始/終了時刻とprovider durationMsを別々に保持し、provider所要時間を優先する。不明な端点・逆転時刻を0秒で埋めない。完了結果だけ再生した場合も、開始時刻は不明のままにする。説明・計画のタイムスタンプを作業時間に流用しない。

チャットのMarkdownコピーに原文保全オプションを使用し、コード内の `\\_` を書き換えない。他の共有エディター利用箇所の既定コピー動作は変えない。

2026-10-25、基点HEAD上の未コミット実装に対して次を実行した。最終候補HEAD・独立レビュー・実アプリの検証は今後追加する。

- `cargo run --bin generate_types --offline`: 成功。生成型を手編集していない。
- `cargo test -p executors provider_adapter --lib --offline`: 41件成功。キャンセル大文字表記の追加後に `cargo test -p executors execution_states_preserve --lib --offline` も成功。
- web-coreのcanonical conversation/timeline、tool lifecycle/output preview、ChatElapsedTime: 計49件成功。
- web-core/UI/local-webの型検査、UI/local-web lint、未使用i18nキー検査: 成功（最終候補の再確認は残る）。
- `node --test tests/chat/syntax-highlight.test.mjs`: 既存ハイライト回帰試験成功。
- `node --test tests/chat/execution-display.test.mjs`: 実CSSを使ったコンポーネントfixtureで成功。代表状態、展開、コピーAPIへ渡す原文、長文更新時の入力保持、ライト/ダーク、1280/375幅でページ横溢れなし。画面とreportは `test-results/local32-execution/`。provider実接続や実アプリ全体の検証とは区別する。

初回Rust試験ではClaudeの親ブロック再展開による再帰と、旧mapperを参照するテストの版指定不足を検出した。再帰を除き、旧版検証は旧mapperを明示するよう修正して41件を再実行した。fixture画面のダークテーマは本体と同じdocument rootへ適用する。表示案・試験の成功だけでdotの視覚レビュー完了とは扱わない。

## 独立技術レビューと修正

一次候補 `65035ed726068c9e0028af999cac7620aac24745` を実装者とは別の新規コンテキストでレビューした。報告は `test-results/local32-execution/technical-review.md` とLOCAL-32コメント `76a7e3c4-76f1-4335-adcf-02618474723c` に原文保存した。指摘はP2が2件で、どちらも独立fixtureで再現した。

- 開始後の承認要求でtoolOutputが置換され、詳細の入力・cwdが失われる。承認への遷移では既存の表示情報を保持し、拒否時だけ未取得出力の状態を更新するよう修正した。
- 承認要求が操作開始より先の場合、後続の操作結果が別行になり、承認済み開始待ちが残る。承認のみの行もattempt/tool IDで登録し、開始・完了を同じ行へ収束させるよう修正した。開始を省略した完了、許可・拒否、逆順履歴再生の回帰試験を追加した。

レビューの未確認リスクであった公開summaryの複数partについても、nativeのsummaryIndexを新mapperのlive payloadへ任意フィールドとして保持し、同一part内の差分を結合、異なるpartの区切りを保持する。旧mapperのlive payloadは変えない。複数partが交互に届くfixtureと完成summaryへの収束を試験した。

一次候補では `node tests/chat/syntax-isolated-e2e.mjs` も成功した。実production frontend/server、独立一時SQLite DBで起動し、AgentRunの一覧とWebSocketのみをfixture化した。実OS clipboard経由でコマンドとコードの `\\_` を含むMarkdown原文一致、ストリーミング中の入力保持、未知/プレーン/長文、両テーマの実色を確認した。server/browserの終了・使用ポートの閉鎖・一時DB削除も成功。証跡は `test-results/local32/isolated-e2e.json` とlight/dark画像である。provider実接続をしたという意味ではない。

修正途中の新しい画面証跡は `test-results/local32-execution-fix/` に分け、dotへ引き渡した一次候補の画像を上書きしない。既存nginxの `preview1.luckyvibekanban.com → 127.0.0.1:18443` が空いていることを確認し、設定変更なしで固定fixtureのforegroundプレビューを使用した。公開メタデータとPNGのHTTPS 200応答を確認済み。一次候補の公開プレビューはそのHEADで固定し、修正候補への切替時は別途明示する。この段落は一次候補公開時の記録である。公開static fixtureは後のユーザー指示で自身のPTYだけを停止した。現在の候補・実アプリプレビュー状態は以下に分けて記録する。


## 修正後候補と独立実アプリプレビュー

動作候補は `0b0263a315e955f676e05af74bd65e09a23681cb`（clean）。承認経路の2件は `c224ff45` で修正した。独立再確認で見つかった空白だけの公開summary delta欠落は `0b0263a3` で修正し、空白・改行・インデントの保持を独立fixtureでも再確認した。確定P2計3件は解消確認済み。原文は `test-results/local32-execution/technical-recheck.md` とIssueコメント `f1c1b042-87cb-45b8-bbb6-bd9ee222b4b8` に保持し、コメントの全文readbackも確認した。

動作候補の検証は次の範囲である。文書のみの後続変更を理由に同じ重いbuildを繰り返さない。

| 検証 | 結果・証跡 |
| --- | --- |
| executors clippy全target、warnings deny | 成功、5分56秒 |
| server / agent-process-host build | 成功、38.72秒。最終候補の実バイナリSHA256は `test-results/local32-auth-preview/preview-manifest.json` |
| local-web production build | 成功、1分2秒。既存のBrowserslist/Tailwind/chunk警告あり |
| frontend関連5ファイル | 53件成功。最終空白修正後は変更箇所のcanonical conversation36件を再確認 |
| 実CSSのcomponent browser | 成功。`test-results/local32-execution-final/` に現候補cleanのデスクトップ/狭幅・両テーマ・展開・stream画面 |
| 実アプリ独立一時DB E2E | 表示・原文コピー・stream・両テーマ・長文は成功。終了直後にブラウザ子PID1件が残り、cleanup判定でコマンドexit1。後続PID消滅を別途確認。元結果 `test-results/local32-final/isolated-e2e.json` と `cleanup-recheck.json` を分けて保持し、全体成功とは記録しない |

最終ユーザープレビューは、static component fixtureではなくWorkspace版の実serverとproduction frontendを使用する。既存の `preview1:18443` 枠を使い、`.build/local32-user-preview` の独立DBへCodex/Claudeの合成履歴をseedした。実providerやコマンドは起動していない。オフラインで実frontend投影処理へseedを通し、5操作の終端状態・空出力・原文コマンド、summary/plan/thinkingの区別を確認した（`test-results/local32-auth-preview/seed-projection.json`）。これはHTTP上の型復号や認証済UIの確認ではない。

nginxの独立preview経路は認証を追加しないため、実アプリをpublic配置で起動した。外向きHTTPSで認証状態locked、未認証の業務API423、ブラウザ初期設定画面を確認した。独立DBには認証設定がなく、認証済UIとseed履歴のHTTP確認は未実施。管理者による独立previewの認証設定、または承認済みの専用設定経路が必要である。共有Auth0の秘密コピー、callback変更、認証無効化、nginx/TLS/DNS/本番設定変更は行っていない。キーも発行していない。認証の設定経路と現在のartifact情報は `test-results/local32-auth-preview/HANDOFF.md` にまとめた。

LOCAL-81との資源協調により、追加の重Rust処理は再開連絡まで保留する。dotの視覚KISSレビュー、認証済み実アプリの確認、最後のユーザープレビューへの引渡しは未完である。provider実接続・PTY観測を行ったという主張はしない。mainマージ・デプロイ・稼働dev再起動は行わない。
