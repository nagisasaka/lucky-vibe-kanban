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


## dot中間レビューへの対応と利用中previewの保全

Issueコメント `1ed09b26-c30e-4526-9511-c074943c1c26` を全文確認した。dotによる中間画像レビューであり、視覚レビュー完了ではない。

完了日時の `&#x2F;` はcomponent fixtureのi18n設定が本体と異なることが原因だった。本体の `packages/web-core/src/i18n/config.ts` はReact向けに `interpolation.escapeValue=false` を設定済み。fixtureにも同じ設定を入れ、日時要素にentityが残らず通常の日付になることをbrowserで確認した。製品の日時・コマンド・出力処理は変更していない。テーマ切替後のfixture撮影は、本体と同じ文字列色へ遷移が完了したことを待つ。

公開稼働artifactを変更せず、既存のビルド済み実アプリを別の独立一時DBで起動してdarkのcomputed色を確認した。文字列は `rgb(165,214,255)` と背景 `rgb(28,28,28)` の比が約11.09、補助状態文字は `rgb(143,143,143)` と背景 `rgb(33,33,33)` の比が約4.98だった。数値はsRGB相対輝度から算出し、測定範囲を `test-results/local32-dot-app/isolated-e2e.json` に保持する。全画面・全状態の視認性を保証するものではなく、公開認証済みブラウザの測定でもない。この範囲では製品色の変更は行わない。新しいfixture画面は `test-results/local32-dot-fix/`、実アプリローカル画面は `test-results/local32-dot-app/` に分け、既存資料を上書きしない。

後にユーザーが独立previewの表示と実Codex試行の成功を報告した。read-onlyでBasic有効・未認証API401を確認した。本担当は認証を設定せず、秘密の内容を読まず、試provider runも開始していない。ユーザー利用中のため公開previewの停止・再起動・artifact上書きは行わない。独立DBにはユーザー作成の実runがあるので、合成fixtureとして削除しない。差替えが必要ならrootから使用終了確認を得てから行う。

DBと作業repoは独立しているが、serverのHOMEは `/home/appuser`、CODEX_HOMEは未指定で、既定の `~/.codex/auth.json` は存在する（存在のみ確認）。この構成はprovider資格情報の隔離ではない。executor機能は有効で、合成seedの作業先はこのinstance配下の独立Git repoであり、LVK本体repoを実作業する接続ではない。一般の同一OSユーザー権限からの隔離・sandbox保証もない。資格情報やユーザーの実run内容を追加で調べず、能力説明に必要な境界だけを記録する。


## 認証済み実会話で確定した初期表示の再修正

rootが実ユーザー会話の画面を直接確認し、初期tool_calls groupが一般ラベルと件数・時間だけになり、操作と結果を読むまでに二段階の展開が必要だと指摘した。前の単純component fixtureは集約処理を通しておらず、この要件未達を拾えていなかった。最終視覚合格ではない。toolbarは動かせるため今回修正しないというユーザーの最新決定に従い、headerとともに変更対象から除外した。

ユーザーDBはread-onlyで最小限のイベントshapeだけを照合した。Shellの開始/成功は引数command/cwd/source/commandActionsと結果aggregatedOutput/exitCode/durationMsを持つ。durationMsは実際に0だった。公開summary本文は439文字あり、空ラベルに見える原因は未取得ではなく本文を初期表示しない描画だった。会話本文・コマンド・出力・認証の内容は証跡へコピーせず、回帰用本文は合成値へ置き換える。

単一操作groupは操作見出しと結果プレビューを直接表示し、1回の展開で原文詳細に進む。複数操作の折り畳みでも各操作の短い見出しと結果数行を残す。説明・計画は原文の先頭行を初期表示し、空白のみの内容は表示しない。全体Run footerは既存の保持済み完了時刻を経過時間と併記し、不明な完了時刻を補わない。取得根拠のある1秒未満（provider報告0を含む）は切捨ての0sではなく<1sと表示する。詳細のprovider報告値は変更しない。

新しい `tests/chat/execution-group.*` はcanonical投影→aggregateConsecutiveEntries→実AggregatedGroupEntryの経路を通す。受信shapeに基づく合成イベントで、初期command/README.md結果、外側の余分な展開がないこと、1回で詳細と原文コピー、Run footerの保持済み日時と13s、1180/375幅・両テーマを確認する。`test-results/local32-initial-group/` に画面・結果を保持する。公開稼働artifactは旧候補のまま維持し、ユーザーの利用終了とrootの差替承認前に停止・再起動・上書きしない。


修正候補 `04acdbd3485f07bd8cb71296f9feaff86239d2d3` のfocused独立技術レビューは追加findingなし。独立SSR2件と既存時間/footer6件が成功し、`test-results/local32-initial-group/technical-review.md` に記録した。dot視覚レビューの代替ではない。

新frontendは `.build/local32-initial-app-dist` へ別buildし、公開distを変更せず生成に成功した。`LVK_CHAT_FRONTEND_DIR` を指定した実アプリE2Eでは、候補frontendをそのtest browserだけへ配信し、既存ビルドのAPI server（0b0263a3）と別一時DBを使用した。API/Rust変更がないためRust buildは繰り返していない。初期操作/結果、展開前のRun完了日時、原文コピー、stream中入力保持、両テーマ、長文とcleanupが成功した。`test-results/local32-initial-app/initial.png`、light/dark画像、isolated-e2e.jsonに異なるfrontend/backendの対象SHA・hash・試験範囲を明記する。公開認証済みpreviewへ修正を配信したという主張ではない。

公開serverはAgentRunのCodex/agent-process-hostの子孫として起動している。Linuxの通常終了でも所有子孫を回収する実装であり、Goal/AgentRun終端後の保持を保証できない。保持方法をrootへ報告し、operatorによるAgentRun外の正規起動等を、ユーザー利用終了・切替承認後に調整する必要がある。nohup/setsid等の回収回避、現在の停止・再起動・二重起動、認証変更は行わない。公開DBは既にユーザー実runを含む保持データである。

## ホストCodexへの引継ぎ：公開差替えとGoal終端後の保持（未実行）

rootは修正後の実アプリinitial/dark画像を直接閲覧し、初期command/result、Run完了日時、dark contrastの主欠陥が画像上解消したと確認した。ただし公開URLは旧版のままであり、公開でのinteractive確認・最終レビューは未完である。

### 凍結対象と検証の範囲

| 対象 | 識別・証跡 |
| --- | --- |
| 引継ぎ時のclean source | `945c78a2986407b8d10b69257aacdf21c340b265`。本節を含む後続変更は文書のみ |
| 新しい動作コード | `04acdbd3485f07bd8cb71296f9feaff86239d2d3`。945cまでのpackages/crates/shared差分なし |
| 候補frontend | `.build/local32-initial-app-dist`。index.html SHA256 `7083012aaa7f5e95610650994a6bf0c859f7f1bcadd9f8a2c5bacf051d14888c` |
| 検証済み既存API server | `target/debug/server`、0b0263a3、SHA256 `ff201915b30b2a11d23b0f97b3907ec43101f2195a9a3b862644f4073d092d1e` |
| 検証済みagent host | `target/debug/agent-process-host`、SHA256 `0bdb526649d2f8025b8bb6cc8245359ef9aeacaab858d65eec390192650ae2d3` |
| 説明・計画を含む回帰 | `test-results/local32-initial-app-notes/isolated-e2e.json`。frontend候補/API既存版/別一時DB、機能・cleanup成功。実行時9f7c+dirtyのtest変更は945cにcommit済み |
| 画像 | 同dirのinitial.png/light.png/dark.png。初期操作・結果、説明/計画の原文先頭行、13sと保持済み完了日時を確認 |
| 独立技術レビュー | `test-results/local32-initial-group/technical-review.md`、Issueコメント `f215851c-33a0-475c-8db7-f46a11fa2ec1`。追加findingなし |

手順はこのtracked文書でレビューする。新規operator scriptは作成しておらず、private場所のdenied fileをコピーして実行する手順もない。検証を再現する場合はtrackedの `tests/chat/execution-group.test.mjs` と `tests/chat/syntax-isolated-e2e.mjs` を使う。

### operatorが実行する前の条件

1. rootからユーザー利用終了・公開差替えの許可を得る。それまでは公開server PID1642032/PTY21002、port18443、稼働assets、instanceを変更しない。
2. 現serverはCodex/agent-process-hostの子孫であり、Linuxの通常終了時も回収され得る。operatorがこのAgentRun外の正規端末または既存サービス管理で起動を所有する。nohup/setsid等で回収を逃れない。保持方法の確認前にGoal/AgentRunを終端しない。
3. workspace rootは `/var/tmp/lvk-instances/dev/runtime/worktrees/db20-/lucky-vibe-kanban`。instanceはその `.build/local32-user-preview`。ユーザー作成実runを含む保持データであり、古いfixture表記を根拠に削除・再seedしない。同じinstanceのDB、鍵、保存済みBasic設定をそのまま使い、秘密の閲覧・共有コピー・認証再設定はしない。
4. resource coordinationの登録・ownerを再確認し、登録されていればcanonical identityの契約に従う。別利用を停止しない。LOCAL81とのheavy Rust協調も再開連絡までは維持する。

### 差替え手順案

この候補の変更はfrontendのみで、既存API/agent hostとの別DB検証は成功している。不要な重Rust再buildを前提にしない。現バイナリはdefault featuresのdebug buildで、`routes/frontend.rs` のRustEmbedは元checkoutの `packages/local-web/dist` を参照する。operatorはfeature/build/hashと参照元を再照合し、異なる場合はこのassetsのみの手順を適用せずrootへ報告する。

- 許可後、ユーザーの実行が終了していることを確認し、現在のpreview所有プロセスだけを停止してport18443とinstance lockの解放を確認する。同じDB/portへ二重起動しない。
- 元の `packages/local-web/dist` をtask所有の保持場所へ退避し、確認済み候補frontendを同じ参照先へ配置する。現在利用中には移動・上書きしない。旧assets・既存バイナリ・instanceは保持し、DBの移行や巻戻しは行わない。候補を再buildする場合もまず別outDirへ生成し、上記sourceとhashを記録する。
- operatorのAgentRun外の正規実行環境から、以下のforeground起動設定を使う。これは未実行の案であり、この文書自体は切替承認ではない。通常の `instance:run NAME` は非staging名でlocal配置を設定するため、このpublic preview起動の代わりに無条件で使わない。

```bash
cd /var/tmp/lvk-instances/dev/runtime/worktrees/db20-/lucky-vibe-kanban
LVK_INSTANCE_DIR="$PWD/.build/local32-user-preview" \
LVK_ACCESS_DEPLOYMENT=public \
LVK_PUBLIC_URL=https://preview1.luckyvibekanban.com \
LVK_AUTH_MODE= \
HOST=127.0.0.1 PORT= BACKEND_PORT=18443 PREVIEW_PROXY_PORT=0 \
VK_ALLOWED_ORIGINS=https://preview1.luckyvibekanban.com \
VK_PREVIEW_DOMAIN= DISABLE_WORKTREE_CLEANUP=1 BROWSER=true RUST_LOG=warn \
VIBE_KANBAN_AGENT_PROCESS_HOST="$PWD/target/debug/agent-process-host" \
./target/debug/server
```

起動後は未認証業務API401、本人のBasicログイン、既存会話の保持、初期操作/結果、説明/計画、展開/原文コピー、Run完了日時を公開URLで確認する。新しいprovider runや課金を確認手段にしない。rootのinteractive視覚レビューとユーザー最終確認へ進み、operator所有プロセスがGoalの子孫ではないことも確認する。TLS/nginx/DNS/main/devには変更しない。起動・認証・assets確認の失敗をガード緩和で解決せず、実状態を報告する。

### 再起動許可後の固定状態（持続owner経路の決定待ち）

ユーザーから「32も再起動してもよい、今は見ていない」と明示許可を得た。rootの方針は旧版保持→新版反映の二段階ではなく、修正版assetsと正規持続ownerへの一回の切替である。LOCAL81が調査中の正規devserver設定経路の結果を待ち、rootがその設定案を確認するまでは現serverを停止しない。共有repo devscriptは変更せず、恒久operatorコードも追加しない。

候補assetsは既存 `.build/local32-initial-app-dist` に固定した。979ファイル、78,040,738 bytes。相対path順に `path + NUL + sha256` を改行で結合したUTF-8文字列のSHA256は `68d93262603fd5e364f39eface0b617d283c3a4fcfc59f704fd0f90da8c8cb75`。全ファイルのhashは `test-results/local32-auth-preview/candidate-assets-manifest.json` に保持する。新しいbuildや公開assetsの変更は行っていない。

上記public起動env、同instance/public URL、DB・鍵・保存済みBasic設定を使う。cloud browserの認証sessionと実会話を維持するため、再setup・秘密共有・データ再seedを行わない。経路決定後にroot確認済みの設定で切り替え、公開でのinteractive確認とGoal終端後の保持を検証する。nohup/setsid等による回収回避、現時点での停止・再起動は行わない。

### 合意済み正規DevServer entrypointの準備

ユーザーとrootは正規DevServer設定変更と32/81の起動を承認した。共通repo devscriptはrootのUI担当が `exec bash .lvk-preview/start.sh` に設定する。各Workspaceの相対cwdで、trackedの[`.lvk-preview/start.sh`](../../.lvk-preview/start.sh)を実行する方式である。32のscriptは今回の合意から新規作成し、private operator fileをコピーしていない。

scriptは同じ保持instanceのDB/key/configured markerの存在、固定server/agent-host/index hash、空port18443を確認し、public配置・同public URL・同instance・既存Basicを維持するenvでserverへforeground `exec` する。background/nohup/setsid、認証設定、seed、データ削除、assets配置はscriptに含めない。`bash -n .lvk-preview/start.sh` と静的検査・artifact hash照合に成功した。scriptはまだ実行していない。現在は旧indexが配置され、port18443も使用中なので、そのままStartすれば安全側に拒否する。

準備完了をrootへ先に報告し、UI設定/Startと直列に調整する。rootの合図後に本担当が旧PTY21002を正常停止してsettleを確認し、凍結candidate assetsを配置・照合する。その後のStartはrootのUI担当に任せ、直接HTTP/APIでは起動しない。正規DevServerのcontainer ownerに属する独立ExecutionProcessとして、Goal終端の子孫回収から分離することを起動後に確認する。DB/鍵/Basic/実会話/public URLは全保持する。現時点では旧PIDを停止せず、public assetsも変更していない。

このentrypointは別コンテキストでread-onlyの静的レビューも行い、事故につながる追加findingなし。script位置からのcwd固定、既存instance欠落時拒否、publicと保存済み認証の使用、固定hash/空port、単一foreground exec、DB作成・設定変更・停止・背景起動処理がないことを確認した。保存済み秘密の内容は読まず、script実行もしていない。

## 独立preview後の追加改善（2026-10-25、検証中）

初回Goalの再実行ではなく、Issue revision `2026-10-25 14:53:58.932` 末尾の有限な追加要件を対象とする。推論要約・明示計画・通常commentary、入力のプレーンテキスト、terminal遷移の最新追従、送信直後の本文表示を改善する。言語protocol調査は別担当の根拠共有を受け、翻訳処理や未確認のlocale設定を追加しない。

providerの識別情報は既に残っていたが、summaryとplanをthinkingとして投影し、完了後の集約で同じ「説明・計画」へまとめていた。provider付きの行はまとめず、推論要約/thinkingは受信した先頭非空行を小さい開閉行へ、明示planは別の本文/項目表示へ渡す。assistant commentaryは通常のassistant_messageのまま保持する。Codexの `turn/plan/updated` は監査のみだったため、v7で実項目・status・説明をProviderExtensionへ保持する。v6以前の再生と子threadの除外は維持する。plan-mode本文はstructured turn planと混ぜず、未知statusや空の取得済みリストも創作で埋めない。Claudeの成功TodoWriteでは妥当な実todosだけを計画として添え、元のtool状態・入力・結果の詳細を残す。

チャットcomposerはLexicalを維持し、plainText modeを明示する。通常のHTML・内部rich clipboard・Markdown shortcuts・format commandを入力装飾へ変換しない。文字列のMarkdown記号・コード・空白・改行をescape/stripせず送信前flushにも同じserializerを使う。アップロード済み内部添付は既存codecのpreview/removeを保ち、一般のMarkdownや外部画像リンクはノード化しない。read-onlyの過去メッセージや他の文書editorは変更しない。ファイルは既存capture/upload handler、IMEとshortcutは既存KeyboardCommandsPlugin、編集と下書きは既存経路を使う。

最新追従は既存virtualizerのbottom lockを使う。terminalで高さが縮んだ結果のscrollTop clampを、手動の上スクロールと判定しない。行数が同じ置換にもlayout補正を適用する。wheel/touch/keyboardや手動履歴のanchorを尊重する。

送信には既存の楽観表示がなかったため、同じsend/steering経路へ一時行を加える。ACK前は送信中、ACK後は受付済み・履歴待ち、4xx拒否は失敗、通信断/5xxは受付未確認と区別し、下書きは成功時だけ既存の条件付き処理で消す。host/workspace/session・送信前の履歴key・本文・取得済みrun IDで正規履歴と照合する。同文の複数送信は1イベントにつき1つを確認し、確認時に残pendingのbaselineを原子的に更新して、再描画で同じイベントを再利用しない。受付未確認を同文retryだけで破棄しない。

独立レビュー初版は `test-results/local32-additional/technical-review.md`、修正再確認は `technical-recheck.md`。通信不明のfailed扱い、描画をまたぐ同イベント再利用、Rust version testの旧期待を指摘され、修正と軽量再確認を行った。添付codecの独立fixtureと実アプリ確認は区別する。LOCAL180のheavy終了連絡後に資源・プロセス・登録resourceを確認し、jobs2で局所Rust検証と別targetのcandidate buildを行った。公開previewの管理DevServer・DB・Basic・鍵・実会話・distは保持し、候補は別出力で準備する。差替え・再起動は未実行、main統合とdeployも未承認である。

### 追加範囲の検証結果

- frontendの型検査、UI lint、未使用i18nキー検査、対象Vitest 59件はpassed。plain-input、pending-send、conversation-sync、実ConversationListの仮想化layout、既存editor-displayのブラウザ回帰もpassed。native ClipboardEvent fixtureによるIME送信guardはOS入力の完全検証とは区別する。
- `SQLX_OFFLINE=true cargo test --offline -p executors --lib provider_adapter::tests -j 2` は42件passed。v7の明示plan、v6再生維持、child scope、空/不正配列、原言語を確認した。`cargo clippy --offline -p executors --all-targets -j 2 -- -D warnings` もpassed。
- `.build/local32-additional-target` は既存targetを一度複製した候補用cache。`cargo build --offline -p server -p local-deployment --bin server --bin agent-process-host -j 2` は4分2秒でpassed。最初のpackage指定ミスはcompile前に失敗し、正しい所属packageへ修正した。公開 `target/debug` の両binと `packages/local-web/dist` のhashは変更していない。
- `.build/local32-additional-app-dist` のfrontend buildはpassed。index SHA256は `e29e03216868077bb3c8efcec3f790500aa7439751d5f9b7664ec97603b05c06`。build時HEADは9be140f9＋dirty差分であり、HEADだけを候補の識別子にはしない。
- 実アプリ回帰は別の一時DBと合成イベントを使い、follow-up HTTPもprovider起動前にinterceptする。途中のDOM改行/複数ボタンselector/copy位置による失敗を `test-results/local32-additional-app/` に保持し、それぞれtest process終了・port閉鎖・一時DB削除を確認した。新候補での `candidate-3/isolated-e2e.json` は全項目passed、cleanupPassed=true。コピーの最終検査は対象assistant行へscroll/hoverして通常clickし、OS clipboardを待って原文一致を確認する。

公開URLは引き続き旧candidateである。追加候補をdotへ画面資料として渡すことと、dotの視覚合格や公開でのinteractive確認は区別する。今回の追加範囲では実provider run、実ユーザーDBの試験、実Auth0、正式Integrationの全gateは行っていない。

成功時の代表画像は `test-results/local32-additional-app/candidate-3/` の `initial.png`、`dark.png`、`input-rich-paste.png`、`input-attachment.png`、`pending-send.png`、`notes-active.png`、`narrow-notes.png`。ローカルの同pathと通常の画像表示で取得する。未認証の画像公開serviceは起動していない。撮影HEADは9be140f9＋dirty差分、server/agent-host/indexの実hashは同reportと `.build/local32-additional-artifacts/manifest.json` に記録する。

候補binは `.build/local32-additional-artifacts/{server,agent-process-host}`、frontendは `.build/local32-additional-app-dist` に固定し、検証用に複製した28GBのtarget cacheは、固定binのhash一致と使用process不存在を確認して削除した。保持する候補binは合計約2.3GB、frontendは約77MBで、後続cutover用に残す。必要envは既存のpublic preview保持設定のままで、新たなsecretやprovider credentialを要求しない。今後のcutoverではrootの承認と直列調整が必要であり、現launcherの旧hash guardを勝手に変更して起動しない。公開previewは正規DevServerの既存ownerが保持し、Goal配下の候補test serverは終了している。
