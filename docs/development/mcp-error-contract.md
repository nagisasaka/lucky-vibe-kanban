# MCP の失敗・結果不明・復旧契約

この文書は TODO-87 を起点とする LVK 提供 MCP 全体の棚卸しと互換方針を記録する。外部 MCP サーバー内部の実装は対象外。LVK が仲介する通信・応答の失敗は対象とする。過去の LOCAL-257 の原因を SQLite busy と断定しない。

## 共通境界

remote の正本は `crates/server/src/remote_mcp/mod.rs` の `tools` と `call_tool_authenticated`、stdio の正本は `crates/mcp/src/task_server/tools/mod.rs` の router と HTTP helper。両方の診断に共通の `utils::mcp_error::Diagnostic` を使い、category・outcome・recovery・message・correlation_id を提供する。成功 payload と tool 入力 schema は維持する。remote の tool failure は `structuredContent.error` を追加し、既存の MCP `isError` を維持する。既存 receipt の `status` と既存 result code を維持し、失敗・不明の診断を追加する。

分類と確定性は独立する。validation / conflict / permission / not_found / busy / unavailable / internal は原因の分類であり、これだけでは副作用の有無を決められない。outcome_unknown は commit 境界、delivery、実行または証拠の喪失により成功の可能性が残る場合に使う。受理や running は終端成功を意味しない。

認証失敗の標準 JSON-RPC ErrorData、OAuth の `mcp/www_authenticate` challenge、未知 tool と schema validation の標準表現は保持する。無差別に独自エラーへ置換しない。SQL、stack、ホストパス、credential、入力値の生文や外部 API の自由文を診断へ転送しない。

## remote の全群対応表

以下は `tools()` の全登録群。いずれも共通 dispatch 診断の対象であり、非同期書込はさらに receipt 境界の対象となる。

| 登録群 | 主な契約・既存共通処理 | 個別の扱い |
| --- | --- | --- |
| read / project_read | Project・Issue・status等の有界読取 | 副作用なし。validation/not found/DB failure を分類 |
| provider_usage | provider 使用量読取 | 外部参照不能を成功のゼロ値と混同しない |
| merge_read / evidence_read / verification_read / code_read | 保存済み実行証拠・Git・差分読取 | 証拠の欠落は実行失敗の証明ではない |
| repository_read | repository query | 検索・IO診断の秘密とパスを遮蔽 |
| activity / run_events / runtime_read / integration_read | 保存済み進捗・runtime・統合状態 | pending/terminal と通信成功を区別 |
| questions | 人間の判断待ち参照 | 読取エラーを判断済み扱いしない |
| resource_observation | resource 状態観測 | 観測不能から所有権解放を推定しない |
| issue_write | create/update/get_operation、共通 Operations | Conflict 以外全 uncertain だった。transaction 前・中・commit の確定性を区別 |
| issue_tags | normal tag 変更、Operations | Validation 以外全 uncertain だった。commit 前の拒否と commit 拒否確定・結果不明を型で区別 |
| issue_comments | comment 作成・編集と読取、Operations | begin/read/write/commit の全 Err が uncertain だった。権限/CAS を保持 |
| todos | TODO read/create/update/delete、Operations | Database 全 uncertain だった。create/update/delete を明示 transaction とし、commit 前の拒否と commit 結果不明を区別 |
| design_notes | immutable revision 読取・write、Operations | Database/Json 全 uncertain だった。commit 前の JSON serialization は rollback 対象 |
| launch | coding 開始・Auto-merge 開始、Operations | domain admission と後続実行完了を区別。delivery 不明を保持 |
| runtime_write | run 停止/継続等、Operations | 実行要求後の観測失敗は対象 run を読戻す |
| goals | Goal 読取・変更・dispatch、Operations | dispatch 到達可能性が残る失敗は再送を安易に薦めない |
| dev_server | managed dev server 制御と状態読取、Operations | 実行後の観測失敗は不明。停止受付から停止完了を推定しない |
| environment | files/process/settings/switch、Operations と各 engine | `environment_files/process/settings/switch` を含む。partial recursive delete、置換後の再観測失敗、process 起動・restart の受付を保持 |
| attachments | upload/place | ファイル単位の部分結果を保持。保存成功と配置・読取成功を区別 |
| repository_wiki | Wiki 状態読取・maintenance admission、Operations | admission と Wiki 完了を区別。通常codingで保守は実行しない |
| issue_archive / workspace_archive | prepare/confirm/unarchive、Operations・domain guard | 人間確認、revision、所有権を保持。受付から物理削除完了を推定しない |
| get_connection / list_projects | mod.rs 直結の接続・一覧 | 同じ共通 dispatch 対象 |

`config/auth/errors/operations` は非catalog共通モジュール、`code_query/issue_comment_text/issue_comments_browser/environment_*` は上記群の補助実装であり対象外という意味ではない。

## stdio の全群対応表

global router は以下の20 moduleを登録する。orchestrator router は context、attachments、integration、provider_usage、resources、repository_read、dev_server、self_restart、workspaces、sessions のみを登録し、list_workspaces/delete_workspace を除く。公開範囲と scope guard は変更しない。

| module | 共通処理・必要な扱い |
| --- | --- |
| context / organizations / remote_projects / repos / repository_read / provider_usage | read と共通HTTP診断。接続やparse不能に内部 reqwest 診断を公開しない |
| remote_issues / todos / design_notes | read と mutation をHTTP methodで区別。CAS・request identity・domain receipt を保持 |
| issue_assignees / issue_relationships / issue_tags | 単発 mutation の通信結果不明を terminal failure と断定しない |
| task_attempts / sessions / workspaces | 実行開始/続行/削除の結果不明と入力拒否を区別。executor config parse の動的自由文も遮蔽 |
| attachments | upload/place の部分結果と既存成功形状を保持 |
| integration | 統合開始の受付と公開完了を区別 |
| resources | run/wait/cancel。resource operation 独自statusと recovery_required を保持。queued は成功完了でも失敗でもない |
| dev_server | API経路に加えて accepted:false のdomain結果も isError と診断を付加。部分起動/停止と観測不能は unknown、missing script は failed |
| self_restart | 固定 local engine を直接起動する例外。request_id による照会、refused/failed/interrupted を維持。guard と認証を変更しない |

HTTP 共通 helper は request method・明示した副作用契約・status から診断する。POSTで読む query_repository / read_provider_usage と integration_action の inspect には読取指定を使う。read の応答喪失は副作用不明を意味しない。mutation の送信・応答parse失敗では副作用が成立した可能性を保持する。mutation のHTTPエラーは、実行前の入口認証拒否である401/403を除いて、statusだけで未commitと推定しない。400/409等もcategoryは保持するが、汎用HTTP境界だけでdomainのcommitが否定できなければoutcomeはunknownとなる。remoteのtyped preflight/transaction証拠がある場合との違いは、確定性の根拠が得られるかどうかである。`expand_tags` のタグ読取は既存の best-effort 前処理で、失敗時は元の入力を保持する意図的例外である。

## receipt と安全な復旧

Operations は principal と request_id と input fingerprint で所有者を固定する。待機側が切断されても owned admission/execution は継続し、同じ key で二重実行しない。この不変条件を維持し、自動リトライを追加しない。

| 観測した事実 | 復旧 |
| --- | --- |
| validation/permission 等で実行前に拒否 | 入力・権限を修正。元の操作が存在しないことと guard を確認 |
| receipt accepted/running | 同requestの receipt を参照し待つ。新requestを開始しない |
| 終端 failed、rollback/未commit 確認済み | 原因を修正。CAS競合は対象を再読し新しいrevisionで意図を確認 |
| 応答喪失・commit不明・panic・他processの未完了receipt | 元requestを同入力で回収、receiptと対象状態を読戻す。新requestで盲目的に再実行しない |
| domain成功後にreceipt永続化失敗 | process内にもIDだけの不確実性証拠を保持し、同processでのreceipt再照会もuncertainにする。再起動後は既存の旧process pending回復規則が引き継ぐ。成功の可能性を保持。元requestとtargetで診断し、保存失敗をdomain失敗へ書換えない |
| receipt証拠の破損/読取不能 | 不明を維持し元識別子から診断。証拠を捏造しない |

operation_id/request_id と安全な相関IDを内部診断への手掛かりにする。correlation logにもユーザー入力や秘密のerror chainを含めない。単純な retryable=true で結果不明の再実行を促さない。

## transaction の調査根拠と検証範囲

`routes/local_remote.rs` の Issue 作成・更新は明示transaction、`db/models/todo.rs` の create は counter/insert/read をtransaction内に含める。Todo update/delete は今回明示transactionへ移し、RETURNINGのdecode失敗でもcommit前のrollbackを保証する。`db/models/design_note.rs` はdomain request結果の保存と変更を同transactionでcommitする。comments作成・編集も明示transactionである。commit前の失敗とcommit自体の失敗を型で区別する必要があり、SQLxエラーの型だけではこの区別を表現できない。実行・ファイル・複数系統のdomain処理にはこの一般化を適用しない。

この対応表はソースを静的に照合したもの。実行した検証と対象SHA、残る未検証点は担当Runの検証記録に記載する。実Auth0/ChatGPT、live DB、稼働server、deploy/restartの試験を隔離fixtureの成功として扱わない。

## 公開形状と互換性

- immediate remote tool failure: 既存の `content` と `isError:true` に `structuredContent.error` を追加。
- stdio tool failure: 既存の `success:false` / `error` 文字列を保ち、`diagnostic` を追加。同じJSONを `content` と `structuredContent` に入れる。backend自由文や動的な入力値は転送しない。既知の安全なstatic入力説明は保持する。
- remote receipt: `accepted` / `running` / `succeeded` の成功形状を維持し、`failed` / `uncertain` に `error` を追加。既存result codeを保持。保存済みresult.errorは型の許可リストから再構成し、破損や任意messageからpanic・秘密漏出を起こさない。uncertainなら保存diagnosticよりunknownを優先。
- parameter deserialisation: stdioのrmcpは元々 `isError` tool resultへ変換する。登録済みhandlerの同じ呼出を使い、変換前にserdeの入力値入り自由文を遮蔽する。未知toolのJSON-RPCコードとOAuth標準challengeは維持。
- `failed` は該当段階の終端失敗であり、任意の複数手順操作に対する「副作用ゼロ」の宣言ではない。部分削除の `partial_possible`、resourceの `recovery_required`、engineの `failed/interrupted` 等の個別事実も読む。unknownの診断は新request作成を推奨しない。

`correlation_id` は即時失敗では生成UUID、receiptでは既存operation_idを使う。共通ログはID・分類・確定性だけを記録し、error chain、SQL、host path、credential、利用者入力を追加しない。元requestのreceipt照会にはprincipal scopeとProject照合を維持する。

汎用HTTPが受け取る `success:false` に機械判読可能なdomain証拠がない場合、自由文から原因や未commitを推定しない。旧repository-read試験が期待していたbackendの「revision mismatch」自由文の転送も廃止し、失敗・読取確定性・安全な復旧案内を確認する試験へ更新した。正当なdomain codeや既存の成功レスポンスを一括置換する変更ではない。

## 隔離検証の対象

追加fixtureは、HTTP statusの分類・不正envelope・応答切断・読取POST・stdio実codecの不正引数遮蔽、transaction前のABORT/rollback・commit拒否/通信断、receipt永続化失敗・panic・process再起動・同request再送の副作用不増、破損diagnosticの安全な再構成を対象とする。実行結果と対象SHAは最終報告とこのRunのsemantic draftに記録する。共有稼働環境は試験に使用しない。

archive は filesystem とSQLiteを同一transactionでrollbackできない。物理操作開始を記録してcommit前・後の失敗を区別し、`physical_deletion_may_have_occurred` と `archive_commit_confirmed` が得られる場合は保持する。削除失敗の事実は `deletion_error` として残すが、内部エラー本文は公開しない。古いarchive receiptは読取時にも同じ安全な投影を適用し、元のDB証拠を更新しない。共通APIのserver-errorログとruntime設定のstorageログもraw error chainを含めない。

失敗/不明receiptのtargetや部分結果に含まれるpath/cwd/executable/purpose、指示本文、実行設定等は `"[withheld]"` に投影する。元入力の再掲による漏出を防ぎ、UUID・状態・revision・部分作用の真偽は残す。成功時の要求されたファイル内容・pathの応答は変更しない。
