# Codex resume developer指示の限定的な重複削減（LOCAL-203）

LVK source base: `45fdda98d586940ef45411ac6fcd2872e9143fad`。
旧読取監査の `069bc57081eb5d6e573bde8fe789fe7090f9d42f` へ戻さず、現行の
`client.rs::thread_resume` に残っていた config全文→inject全文の経路を対象とした。
Workflow、Memory checkpoint/restoreの追加model turn、usage・認証・quotaは変更しない。

## 動作と安全側の限定

初回 `thread/start` とresume configには従来どおり完全なdeveloper指示を渡す。
通常resumeで、同じ固定指示が当該threadの通常履歴に残る条件を満たした場合だけ、
`thread/inject_items` をCURRENT repository-memory JSON行とparallel-context JSON行へ縮小する。
同じrunでも最新identityを再注入する。残る固定指示には、古いrun ID/draftを使わない指示、
Memory/Wiki方針、resource調停、caller指示、skill参照等を含める。
固定側は空白も含めて全体一致が必要で、変更時は全文を追加する。
過去のどれかが一致するだけでは足りず、後続の異なるdeveloper指示は一致判定を無効にする。
injectの失敗は従来どおり継続を停止する。追加のmodel turnは作らない。

縮小はUnix、initializeで報告されたCLI版が `0.144.1` / `0.163.0` の場合だけ。
他版・不明版・prerelease・非Unixは全文fallbackで、通常のCLI互換性を拒否しない。
新しいowned app-serverの最初のthread loadだけを対象とし、in-process再resume、
明示的なhistory/path overrideは全文fallback。fresh/resetは従来の完全な指示を保つ。

公開protocolの `thread/read(includeTurns=false)` から対象threadのpathだけを取得する。
表示用 `ThreadItem` はraw developer履歴の保持証拠にならず、これだけでdedupしない。
信頼するpath authorityはLVKが起動したlocal app-serverであり、任意のuser指定pathではない。
対象IDの一致、absolute `.jsonl`、canonical path一致、regular file、
session_meta ID/既知CLI版を検査する。他thread探索、directory列挙、archive/fork親の追跡はしない。
Unixの `O_NOFOLLOW | O_NONBLOCK` は既存RepositoryMemoryStoreの読取patternに合わせた。
symlink/nonregularを拒否し、各読取前後でinode/device/length/mtime/ctimeを照合する。
これは同じOS userによる悪意あるapp-server/ファイルシステム改変を防ぐsandboxではない。

resume前の履歴を読み、resume後にも同じファイルを上限付きで読み直す。
元prefixの完全一致と同じinodeを必要とする。対応sourceの0.163.0はresume時に
`thread_settings_applied`を保存するため、末尾へのこの既知の非model履歴eventと
model-switch `warning` だけを許容する。新しいresponse項目、部分書込み、未知event、
置換、読取中の変化は全文fallback。特に以下は履歴に固定文が見えても縮小しない。

- compact / context_compacted / rollbackを一度でも含む履歴（後続の全文追加後もfallback）
- forked_from_id / forked_from_ordinal_exclusive / history_base
- retained_context、新しいresponse形式、未知のrollout/event形式、JSON不正・不完全文書
- path/ID不明、読取失敗、timeout、16 MiB超過（sparse fileもmetadataで早期判定）

上限は1読取16 MiB、resume前後の最大2読取で32 MiB＋JSON解析と文字列比較。
metadata RPC、各読取にそれぞれ2秒のdeadlineを置く（解析CPU時間は別）。
file descriptorは各読取で閉じ、sidecarや履歴ファイルを書かない。
読取と公開resumeの間の整合も確認し、resume後だけのdisk内容をそのまま保持証拠にしない。

## 公開sourceの根拠と限界

依存protocolの固定commitは `44918ea10c0f99151c6710411b4322c2f5c96bea`
（Cargo.lock、`rust-v0.144.1`）。安全な `codex --version` は `codex-cli 0.163.0`。
既存local公開source `83086188ae0614bd882818ff1aa23f3194962012` は同じ版番号だが、
実行binaryの厳密commitは不明。同版番号だけでbinary一致を証明したとは扱わない。
この実装は以下で読める公開実装の通常replayを前提とし、未知形式では縮小しない。

- [0.163 codex_thread.rs:841 inject_response_items_for_turn](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/core/src/codex_thread.rs#L841)：baseline欠落時の初期文脈復元後、raw項目を履歴へ追加。
- [app-server thread_processor.rs:2934 read_thread_view](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/app-server/src/request_processors/thread_processor.rs#L2934)：metadata読取は既存loaded threadの参照とpersisted view読取であり、新たなmodel実行threadを作らない。
- [session/mod.rs:1671 record_initial_history](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/core/src/session/mod.rs#L1671)：resumeのreconstructionと設定receipt追記。
- [session/rollout_reconstruction.rs:495](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/core/src/session/rollout_reconstruction.rs#L495)：通常suffixのresponse replay、compactのreplacement history、rollbackの削除を区別。world_stateや設定eventはresponse履歴を置換しない。
- [context_manager/history.rs:534](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/core/src/context_manager/history.rs#L534)：replay→record_item_with_metadata。developerのinput_textはoutput truncation対象ではなく、is_api_messageで採用される。
- [history.rs:603 for_prompt](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/core/src/context_manager/history.rs#L603)、[history.rs:966 normalize_history](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/core/src/context_manager/history.rs#L966)、[session/turn.rs:1666 build_prompt](https://github.com/openai/codex/blob/83086188ae0614bd882818ff1aa23f3194962012/codex-rs/core/src/session/turn.rs#L1666)：tool pairや非対応mediaの正規化からpromptへの経路。通常developerテキストを同文dedupする処理ではない。
- [0.144 session/rollout_reconstruction.rs:320](https://github.com/openai/codex/blob/44918ea10c0f99151c6710411b4322c2f5c96bea/codex-rs/core/src/session/rollout_reconstruction.rs#L320)、[history.rs:141](https://github.com/openai/codex/blob/44918ea10c0f99151c6710411b4322c2f5c96bea/codex-rs/core/src/context_manager/history.rs#L141)：旧protocol版の通常response保持も照合。compact中の先頭trimはcompact用history clone上の処理で、今回compact履歴はそもそも対象外。

## 無課金fixture

`node scripts/dev-cargo.mjs test -p executors executors::codex:: --lib -j 6`。
`resume_instructions` unit testは合成JSONLのみ、stdio fixtureはtest実行ファイル自身を
fake app-serverとして起動する。Codex/modelや新しいAgentRunは起動しない。
RPCではconfig全文・縮小inject・settings receipt追記・inject拒否を確認する。

`initial_new_run_same_run_and_repeated_resumes` の合成文字列は固定文
`Fixed policy: 日本語 safety and draft rules.` とrun/draftだけのJSON。
下表はdeveloper text値の文字数/UTF-8 bytesであり、RPC JSON全体のtransport量ではない。

| 段階 | start/resume config 文字/bytes | inject文字/bytes | 合成履歴の固定文件数 | identity件数 |
| --- | ---: | ---: | ---: | ---: |
| 初回old run | 141 / 147 | なし | 1 | 1 |
| resume 1 current run | 149 / 155 | 108 / 108 | 1 | 2 |
| resume 2 same run | 149 / 155 | 108 / 108 | 1 | 3 |
| resume 3 same run | 149 / 155 | 108 / 108 | 1 | 4 |

これはwrapperの判定とRPCの再現証拠であり、実effective promptの測定ではない。
初期developer bundleにCodex自身のpermissions等が同居して全体一致しない場合、
初回resumeは全文fallbackとなり得る。compact後や未知履歴の重複、configの再送、
必要なidentity履歴は残る。実rollout・個人履歴・認証情報は調査していない。
実model入力、cached/noncached token、料金、quotaへの実害/削減量は未測定。
文字数からtokenやquota率を推定しない。

## LOCAL-204への統合注意

主変更は `client.rs::thread_resume` と新しい `client/resume_instructions.rs`。
initialize応答のuser_agentを新しいOnceLockへ保存する1行とそのfield/初期値も必要。
204の `initialize_inner(true/false)` へ統合するとき、この保存を共通handshake成功箇所へ残す。
203はordinary/passive lifecycle、version観測のbool gate、account_read bridge、
usage/notification処理を変更しない。libcは既存Linux dev-dependencyからUnix runtime
dependencyへ移し、bounded file openに利用する。

## この実装runの検証記録

2026-10-26、Workspace `756a9c7e-52f3-41b0-b15d-216a6e8901f5`、
AgentRun `e963f2fa-f490-42ff-8661-cf18c0af7a22`。追加review/agent fanoutなし。

| command | result |
| --- | --- |
| `node scripts/dev-cargo.mjs test -p executors executors::codex:: --lib -j 6` | 129 passed / 6 ignored（最終境界調整前の広めの回帰） |
| `node scripts/dev-cargo.mjs test -p executors resum --lib -j 6 -- --nocapture` | 最終codeで19 passed / 1 ignored。ignored stdio fixtureはparent testが明示起動して検証 |
| `node scripts/dev-cargo.mjs clippy -p executors --all-targets -j 6 -- -D warnings` | 最終codeで成功。非test targetのcompileも含む |
| 対象2 Rust fileの `rustfmt --edition 2024 --check`、`git diff --check` | 成功 |

単独の `check -p executors` は待機中に取り消し、Rust gateを逐次実行した。
全workspace gate、非Unix実行、実binaryのprompt/compaction E2Eは実行していない。
共有資源snapshotに登録resourceはなく、既存dev-cargo入口とCargo lockを使用した。
並行作業を止めず6 jobsで実行。観測MemAvailableは約10.8–12.3 GiB、
全体swap使用は約1.13→最大1.57→1.43 GiBで、直近swap-outは0。
共有hostの値であり本runへの因果帰属やcompiler peak測定ではない。
release build、main merge/push、配備、sccache停止、server restartは行っていない。

Manifestのidentity/commit/pathはLVKのrun完了finalizerがGitから決定する。
このrun専用semantic draftを渡し、canonical immutable eventの手書き・先行生成はしない。
統合ownerはrun完了後、このAgentRun IDの公開Change Manifestとsource SHAを照合する。
