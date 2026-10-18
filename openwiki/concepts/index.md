# ファイル

- [Arena による比較と選択](arena.md) - 同じ Issue に対する複数の Workspace を比較する Arena の意味、Design と Implementation の契約、選択・再試行・終了の寿命。
- [Card context と並列作業の参照](card-context-and-llm-wiki.md) - 保存済み Card context の継承、並列 Workspace の参照、固定 commit の読取、および旧 LLM Wiki の廃止境界。
- [正式 Integration の選択・検証・公開](formal-integration.md) - local Board で採用した Card と Workspace の変更を固定し、host 検証済み commit だけを公開する契約。予約、取消、条件付き Done、Wiki と回復の境界。
- [Issue の添付・購読・通知](issue-collaboration.md) - local Board の担当・購読・コメントと実行通知、Session 添付との境界。
- [Organization・Member・Invitation](organization-and-membership.md) - 固定Organizationと認証actor、承認制メンバー、過去の所有者IDを区別する。
- [Project・Issue と実施の関係](project-and-issue.md) - ボード上の仕事と実行環境を結ぶ Project・Issue の意味、ローカル保存、旧 Task との境界。
- [Astra 廃止と共有機能の移行境界](project-chat.md) - 内蔵 Project Chat の廃止、専用履歴を削除する migration、外部 MCP と Workspace 質問回答に残る契約を説明する。
- [Project Todo の共有メモ](project-todos.md) - Project に保存する Todo と Issue・実行の違い、Web と二つの MCP の共有データ、競合・再送・表示の契約。
- [Repository Memory の三層と統合契約](repository-memory.md) - 会話、Workspace Memory、canonical OpenWiki の権威と寿命、Change Manifest と統合記録・receipt の関係。
- [サーバー認証・設定・セッション](server-access.md) - Local/Public配置、Basic/Auth0の有効化、Webセッションと内部capability、設定破損時の閉鎖と復旧の契約。
- [Session・AgentRun・RunAttempt](session-and-agent-run.md) - 会話・依頼・実行試行の識別、継続と retry、provider binding、Goal と停止の契約。
- [共有リソースの所有権と AI 調停](shared-resource-coordination.md) - 複製できない実機・DB・環境を並列 Card から扱うための排他制御、状態変更、調停と復旧の境界。
- [Workflow Attempt とグラフの契約](workflow-attempt.md) - Issue 専用の実施グラフ、安定 Session、run snapshot、辺の発火・分岐・再実行の意味。
- [Workspace と Repository](workspace.md) - 作業環境の所有権と用途、実行所有者、複数 Repo、共有資源、削除と回復の境界。
