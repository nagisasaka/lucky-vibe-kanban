# 7source候補のhandoff manifest

Workspace `e131bcdc-3b0f-4d4a-9f8f-aca87d15ed33`。開始HEADは独立206の
`901bdf9e5e44373c3557608d95e1431a93939e8a`、mainは
`45fdda98d586940ef45411ac6fcd2872e9143fad`のまま。
main更新・正式Integration・Wiki publication・配備/release build・sccache停止・再起動は上位ownerへ残す。

## source・合成

206から203→204→183→81→214→215を通常merge。全7sourceを祖先に保持し、履歴書換えや200/212/213の別途取込みはない。
81の既存件数表示c456e106とbase取込み63ff87aeも凍結sourceの祖先として保持した。

| LOCAL | source |
| --- | --- |
| 81 | `dbf47c47bfa92f62e344407868e514bf957111ae` |
| 183 | `1de0603c5e045b2e10f998a27c466fa38e1f1633` |
| 203 | `ca059584a696fef6016de05ce6e75ecaf470fef6` |
| 204 | `648409726dddae62f8494065c83253fb50ff156c` |
| 206 | `901bdf9e5e44373c3557608d95e1431a93939e8a` |
| 214 | `7325d70f6566d823821ff5f65a6b3b9f5614ea60` |
| 215 | `7bf29de7109edcffed606e738d172e3676038ccf` |

元ref/Issue/Workspace/parent/tree、merge parent、変更path/hash、log/SHA/所要・artifact SHAは
[詳細証跡JSON](seven-source-candidate-evidence.json)。最終QA commitのHEAD/tree/main/clean/7祖先は、commit後に
`.build/seven-source-candidate-fdf7c33f/final-revision.json`へ保存する。

手動競合は2path。

- `codex/client.rs`: normal initialize(true)のversion観測＋user_agent保存、passive(false)の隔離を両立。
  resume最適化をpassiveへ流さず、PassiveCallbacks/owned cleanup/deadline/5秒RPC/2MiB制限と206の一回承認・正確ID・Cancel/no-auto-approve/pending-countを保持。
- `IssueWorkspaceCard.tsx`: 81のcompact状態/既読/DevServer表示と183のPreparing・時計抑止を両立。RunningDotsを重複させず、複数run/scripts集約is_runningは維持。

自動mergeも、stdio global/orchestratorのprovider_usage＋dev_server、remoteの上記＋edit_issue_comment、catalog uniquenessを確認。
215のissuer/subject owner・legacy拒否・BEGIN IMMEDIATE CAS/receipt、214のfilename*/CRLF/byte/auth/pathを保持。
183のStarting/attempt/failure fence/host予約と206のscript停止追跡は別経路。既存119 migration blobは不変。

## 検証・資源

| 結果 | 内容 |
| --- | --- |
| Rust 251 passed / 9 ignored、235.722秒 | resume/passive/account/承認、DB41＋rank2、agent_run_port27、DevServer registry2、stdio9、server34（comment8/raw14等） |
| focused initialize 1 passed、31.892秒 | 最終lint修正後のnormal/passive fixture |
| mock HTTP 2 passed、5.206秒 | 候補test artifactを再利用したcatalog/auth回帰 |
| Clippy passed、90.859秒 | 変更7crate全target、server/qa-mode、warnings禁止 |
| UI model 52 / rank 4 passed | canonicalActions/Conversation、taskAttempt、DnD |
| 型3package・局所lint・format passed | 正規local依存install、lockfile変更なし |
| component browser passed | Kanban Preparing/状態/DnD、MCP承認desktop/mobile light/darkのoffline mock |

旧行保持migrationと全migration/FK fixtureもpassed。live DB移行の保証とは区別する。
初回の新fixture import漏れ、never_loop lint、timeout時のボタン消失の期待値、整形警告は修正・再確認済み。
最終scan helperの構文/自身shell文字列誤検出も修正し、04:29:48 JSTに生存試験process/zombie/build writer各0を確認した。process停止操作なし。

開始確認04:05:24 JST、merge作成04:06:05〜04:07:29 JST。各検証時刻は詳細JSON。
8CPU、Rust jobs6/test threads2、重command逐次。Rust RSS合計peak約4.70GiB、host available最小約9.37GiB。
Clippy約4.36GiB/10.19GiB。2秒swap sampleはin/out0頁。RSSは共有頁の重複を含み、swap sampleは全区間の証明ではない。
共有build-dir不整合は今回未発生。197入口/default/sccacheを維持し、例外出力先・cache削除・daemon停止なし。
Git auto-gcの既存loose-object/gc.log警告は保持し、手動gc/prune等は行わない。

## root cloud QAと残る確認

```bash
node tests/chat/mcp-approval.mjs --prepare-only
# Chromiumの表示assert/screenshotを再実行する場合
node tests/chat/mcp-approval.mjs
```

生成物は `.build/seven-source-candidate-fdf7c33f/mcp-approval/{fixture.html,desktop-light.png,mobile-dark.png}`。
既存ChatToolSummary/SessionChatBox＋明示mock policyを使用。第三者server/tool、Project/Workspace、一回scope、timeout後の承認消失を確認できる。
Toggle操作/ボタンはmock stateだけを変更する。API/provider/実承認応答はなく、redaction済み合成引数の表示試験である。

独立206の実agent承認待ち到達・no answer・cleanup証拠は再利用し、新しい実agentは起動していない。
実認証browser、実secret redactionの変換、人間承認後の実呼出し成功、全backend/full formal suite、SQLx全prepare/type全再生成、配備artifactは未実施。
上位ownerが正式Integrationのvalidation/skip判断と公開・配備・再起動を行う。

原ログ/metadata/stage snapshotは `.build/seven-source-candidate-fdf7c33f/` に保持。
正規node_modules約981MiB/local pnpm store約74MiBと通常targetは再検証用に保持し、他Workspace/共有cacheは整理しない。
