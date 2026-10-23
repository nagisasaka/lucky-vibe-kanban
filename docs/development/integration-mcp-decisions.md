# Integration承認・取消MCPの実装検証

対象は元TODO `7816687d-13f4-4e96-bb96-7528f0b99a39`、Workspace
`42cab2a7-f252-424a-87fd-c936c1fd3392`。
開始時base SHAは `e701d0156935f7460eb457cc2875dd8654f6be73`。
最終commit SHAは実装完了報告に記載する。この記録と同じcommitのRustソースを検証対象とする。

## 契約と選択

- `approve_integration` / `cancel_integration` を既存HTTP MCP catalog・schema・dispatch・`lvk:merge`認可へ登録。
- 承認はUIの `decide_gate` と同じ共通処理。MCPでは同じmutation lock内で読取revision・base/candidate SHA・既存scopeのdigestも照合する。既存のGit最新target照合とDBの原子的scope判定を維持する。
- このbaseには専用pending gate読取MCPがないため、既存 `get_integration` に `pending_gates` sectionを追加。UIと同じ保存済みreviews/gatesを読む。新しい候補IDや承認台帳は作らない。
- 取消はUIの既存取消handlerへ委譲。MCP adapterはIntegration DBを書き換えない。既存operation receiptがprincipalとrequest UUIDに対する冪等性を提供する。
- 受付receiptは履歴として保持。現在状態はsummaryの `lifecycle` を再取得する。`cancelled` 保存と予約削除は別工程なので、同一SQLite snapshotで両方を確認して `cancellation_complete` を返す。
- 認証主体を持つ既存MCP audit/receipt経路を通す。既存domain gate actor表現自体は変更しない。
- stdio MCP、Goal制御、自動承認、実環境の設定変更、main mergeは対象外。

## 局所検証の再現方法

リポジトリrootで、先に `ps` / `free -h` / `df -h .` を確認する。
利用可能なsccacheと既存targetを使い、Cargoは2 jobs、重い工程は直列に実行する。

```sh
CARGO_BUILD_JOBS=2 RUSTC_WRAPPER=/usr/local/bin/sccache \
  SCCACHE_DIR="$(realpath .lvk-shared/cache)/sccache" \
  cargo test -p server --lib remote_mcp::integration --no-run
```

表示されたserver lib test executableを `test_bin` に指定する。
同じbinaryを直接使えば、無関係な再buildを挟まず同一ソースの試験を実行できる。

```sh
"$test_bin" remote_mcp::integration_write_tests --test-threads=1
"$test_bin" integration_reads_separate_validation_publication_and_current_wiki_evidence --test-threads=1
"$test_bin" http_discovery_authentication_and_tool_call_roundtrip --test-threads=1
"$test_bin" mechanical_conflict_approval_continues_without_retry_and_rejects_stale_approval --test-threads=1
"$test_bin" exception_preserves_failure_and_never_waives_identity_or_settlement --test-threads=1
"$test_bin" cancellation_distinguishes_unaccepted_identity_from_inflight_or_missing_evidence --test-threads=1
```

host scenarioの2試験はそれぞれ新規の一時 `local9-scenario-*` ディレクトリを作り、
絶対パスを `LVK_INSTANCE_DIR` に設定して単独実行する。既存instanceを使わない。
`LVK_ACCESS_DEPLOYMENT=local`、`LVK_AUTH_MODE=local`、
`HOST=127.0.0.1`、`DISABLE_WORKTREE_CLEANUP=1` とする。
`LVK_PUBLIC_URL`、`LVK_MCP_CONFIG`、`VIBE_BACKEND_URL`、
`VK_ALLOWED_ORIGINS`、`LVK_INTEGRATION_REAL_PROVIDER` は継承しない。
実行プロセスと子プロセスの終了を確認してから、一時instanceを削除する。

```sh
"$test_bin" isolated_mcp_integration_approval --ignored --nocapture --test-threads=1
"$test_bin" isolated_mcp_integration_cancel --ignored --nocapture --test-threads=1
```

前者は実Git競合→保存review→MCP承認→次source→公開、後者は承認後の
`integrating` 中にMCP取消→受付時の予約保持→停止処理→予約解放を扱う。
どちらもUI/MCPのstale target拒否、読取候補の失効、権限不足、異なるProject、
同一request同時送信、終端後のreceipt/現在状態再取得、遅い取消を検証する。
Providerの回答は固定イベント。製品の状態機械・Git・SQLiteは実装を使う。

## 実行記録

2026-10-24 JST。工程は一部重複するため、所要時間の合計は全体時間ではない。

| 工程 | 時刻・所要 | 結果 |
| --- | --- | --- |
| 開始・現行契約調査 | 08:00:41–08:02頃（約2分） | baseとUI/MCP入口・既存scope・予約解放順序を照合 |
| 実装・隔離試験定義・運用doc | 08:02頃–08:10:43（約9分、compileと並行） | 新toolと共通経路、読み戻し、試験を実装 |
| 初回compile | 08:02頃–08:04頃 | 2 jobsで開始後、統括の順次試験指示を受け自分のprocess groupのみSIGINT。終了130。テスト未実行 |
| compile再開 | 08:05:13–08:14:55（9分41秒） | LOCAL127終了・再開許可後。unused Json warningを1件修正 |
| 最終sourceのbinary確定 | 08:15:21–08:16:07（45.23秒） | 上記Cargo command成功、警告なし |
| 局所試験8件 | 08:16:16–08:16:23（約7秒） | 全8件passed、failed/ignored 0 |
| 記録・最終差分確認・commit | 完了報告に記載 | Rust変更後の広範囲再試験は実施せず |

実行binaryは `target/debug/deps/server-4ecb5813dcfd5681`。
上記6つの通常filterと2つのignored scenarioを、それぞれ単独プロセスで
`--nocapture --test-threads=1`（scenarioはさらに`--ignored`）付きで実行した。
各filterの実行件数は1、すべて成功。host試験のproviderは固定イベントで、実AIは起動していない。
`rustfmt --edition 2024 --check --config skip_children=true`（変更Rust9ファイル）と
`git diff --check` も成功。

`df -h .` の空きは開始時125 GiB、再開前123 GiB、最終compile直前113 GiB、
試験直前/終了後112 GiB（終了後Used 830 GiB、Use 89%）。ディスク不足は発生していない。
再開前RAM availableは約12 GiB、終了後約13 GiB。各重いCargo実行前にprocess/RAM/diskを確認し、
他の重いRust buildは見えなかった。別Workspaceのprocessやデータは操作していない。

sccacheは初回観測が `/home/appuser/.cache/sccache`、最終観測はcanonical
`.lvk-shared/cache/sccache`。server停止・再設定・cleanは行っていない。
最終の共有統計はhits 2171 / misses 1633 / non-cacheable calls 259、
cache errors 2（Assembler）、compilation failures 0。統計は他clientと共用で、
本タスク単独の値と解釈しない。

試験processと一時instanceは終了・削除済み。再利用する通常`target/`は約12 GiBを保持する。
一時runner/logは結果を本記録へ転記して片付ける。正式Integrationや実環境の承認・取消、
デプロイ、再起動、認証設定変更は行っていない。

## 未検証範囲

実Auth0/ChatGPT、実AIの競合解消品質、実プロセスを伴う新規UI/E2E、
stdio MCP、全面build/lint/test、正式Integrationとデプロイは実施しない。
UIの操作構成や生成TypeScript型は変更していない。
新規host scenarioはMCP adapterから実domainを呼ぶ試験であり、新tool成功時の
HTTP transport全体を通すE2Eではない。HTTP catalog/権限拒否は既存の隔離fixtureで検証する。
