# Claude Code の対話承認の隔離検証

Claude の Supervised 実行を既存の LVK 承認 UI・Run 別承認サービスへ接続する検証。
fixture は公式 CLI の制御メッセージを模擬するだけで、モデル、認証情報、Bash、
ファイル操作にはアクセスしない。実際のコマンド実行や本番での権限検証ではない。

## プロトコルの根拠

- [公式 permissions](https://code.claude.com/docs/en/agent-sdk/permissions):
  `default` の承認要求は `canUseTool` で処理し、`plan` と
  `bypassPermissions` は別の意味を持つ。
- [公式 Python SDK の query.py](https://github.com/anthropics/claude-agent-sdk-python/blob/main/src/claude_agent_sdk/_internal/query.py):
  `control_request` / `can_use_tool` を受け、元の `request_id` に対する
  `control_response` を返す。許可は `behavior: allow` と元の `updatedInput`、
  拒否は `behavior: deny` と `message`。一回の許可に `updatedPermissions`
  を追加する必要はない。
- [公式 subprocess transport](https://github.com/anthropics/claude-agent-sdk-python/blob/main/src/claude_agent_sdk/_internal/transport/subprocess_cli.py):
  stream-json と `--permission-prompt-tool`、`--permission-mode` の CLI 配線。

これらは接続形式の根拠であり、すべての CLI 版や実アカウントでの互換性を
fixture だけで証明するものではない。初期化・permission mode 設定を確認してから
ユーザーの prompt を送る。接続できなければ AUTO に切り替えない。

## ブラウザー fixture

依存関係、Playwright 対応 Chromium、変更後の `server` と
`agent-process-host` が必要。ビルドは repository の `scripts/dev-cargo.mjs`
経由で担当 coding agent が行う。

```bash
LVK_APPROVAL_TEST_PROVIDER=CLAUDE_CODE python3 tests/supervised/run_isolated.py
```

runner は一時 instance、DB、profile、Claude 設定ディレクトリと動的 loopback
ports を使用する。実ユーザー設定と稼働 instance を変更しない。
`tests/supervised/claude-approval.spec.ts` が共通画面からの許可・拒否・停止、
承認待ちの再読込、二要求の独立解決、元 request ID、元 input の保持、
`updatedPermissions` 非追加、終了後の遅延応答拒否を検査する。

既存 Codex 経路は provider 指定なしで同じ runner を使う。

```bash
python3 tests/supervised/run_isolated.py
```

runner の finally で所有 process と listener の終了を確認し、一時 instance を
削除する。結果とログは `.build/supervised-approval/e2e-*`、ブラウザー画像は
`test-results/claude-approvals/`（Codex は `test-results/supervised/`）。成功した検証と、未実行・失敗した検証は完了報告で
分けて扱う。

## 対応範囲

この接続は一回のツール承認を対象とする。Claude Plan の遷移と質問 UI は
対応済みと扱わない。Supervised では direct CLI と接続済み host を要求し、
router、追加引数付き launcher、追加 CLI 引数、実効設定に残る permission bypass は
起動前に拒否する。AUTO の既存実行条件は維持する。
明示的な Run の Supervised 選択は runtime コピーの permission bypass を解除する。
組み込み DEFAULT が持つ bypass 値を保存したまま実効 mode を `default` にし、
元 profile や AUTO の設定は書き換えない。ブラウザー fixture の一時 profile も
DEFAULT と同じ bypass 値を持たせ、明示 Supervised による絞り込みを検査する。
承認前の保留、取消、切断、期限切れ、重複・別 Run の
応答は Rust の protocol / process-host / approval bridge fixture と合わせて検証する。
ブラウザー再読込は生存中の host への再接続を検査し、host 消失からの承認復元を
保証するものではない。実 CLI・実モデル・本番の権限操作は範囲外。

人が返した拒否理由は共通の解決記録と native deny に保持する。
provider の `decision_reason` 等は元 control request の監査 frame に保存され、
同じ Run / tool ID に相関する。共通画面は操作入力を表示するが、provider 説明文を
表示する専用欄は今回追加しない。実入力の schema を変えないため、理由を
`updatedInput` や `ToolCall.arguments` に混入しない。専用表示には共通の
canonical metadata と UI を拡張する別設計が必要。
