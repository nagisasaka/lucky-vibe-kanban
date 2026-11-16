# stdio get_context と Codex 承認設定の隔離検証

この変更は `get_context` の読み取り専用宣言を修正する。`resources` guide は
データディレクトリを作成し得る `asset_dir()` を使っていたため、既存の
非作成API `asset_dir_path()` に替える。有効な既存インスタンスで返すパスと
guide本文は同じ。引数なし・coding・parallel・resources の応答契約を維持する。
認可、ACL、書き込みツールの注釈・個別承認設定は変更しない。

## 検証境界

- ソース基点: `86f3058779b7ac032ed2c0c1444f76c06faf598d` とこの差分。
- 実行するCodex: `codex-cli 0.163.0`。fixtureはこの版以外を拒否する。
  native executable SHA-256:
  `399ff69c431c066e40de318fba45c2b07f45a76b19939d247ee4828fc70c4215`。
- RustのCodex protocol依存はリポジトリで固定済みの `rust-v0.144.1`。
  これは実行したCLIの版と同一ではない。
- UIは実コンポーネントと実Rust設定サービスを使用する。HTTPルーターは通さず、
  Playwrightが管理するstdin bridgeで接続する。`AgentSettingsService::new` に
  一時homeを指定し、Userスコープの `.codex/config.toml` を実際に更新する。
  一時homeをOSのHOMEや稼働サーバーの保存先に設定しない。
- Run試験は別の一時 `LVK_INSTANCE_DIR`、DB、Git repository、process host、
  `CODEX_HOME` を使う。実LVK API・native Codex・実stdio MCPを通す。
  Responsesだけをloopback fixtureに置き換える。外部モデル・実アカウントは使わない。
- 正式自動マージは `/api/integrations` で受付し、新規担当Runを起動する。
  fixtureモデルは `get_context` だけを呼ぶ。`integration_action` は呼ばず、
  統合を取消し、target SHA不変とプロセス終了を確認する。公開成功の試験ではない。
- 実ユーザー設定、稼働インスタンスのprofile・承認・sandbox、LOCAL-400、
  認証・ACLには触れない。既存Runへの遡及適用は検証対象外。

## 確認項目と再実行

```sh
node scripts/dev-cargo.mjs test -p utils resource_guide_does_not_create_instance_data
node scripts/dev-cargo.mjs test -p mcp stdio_tests
node scripts/dev-cargo.mjs test -p executors approval_
node scripts/dev-cargo.mjs clippy -p utils -p mcp -p executors --all-targets -- -D warnings
python3 scripts/test-codex-approval-config.py /usr/local/bin/codex
python3 scripts/test-codex-mcp-readonly.py /usr/local/bin/codex
LVK_E2E_CHROMIUM=/path/to/chrome pnpm exec playwright test -c tests/settings-redesign/playwright.config.ts approval-policy.spec.ts
node scripts/dev-cargo.mjs build -p server -p local-deployment -p mcp --bin server --bin agent-process-host --bin vibe-kanban-mcp -j 4
python3 scripts/test-codex-approval-runs.py
```

共有Rustビルド待ちがある場合、UI試験は先に同じソースでビルドしたexecutors
test executableを `LVK_APPROVAL_TEST_BINARY` に指定できる。bridgeはその一試験を
`--exact --ignored --nocapture` で実行する。通常のRust試験ではbridgeはignoredであり、
UI試験が明示的に起動して実行する。

UIでは `on-request` → `never` → Restore inheritance を保存・確認し、各回の
ページ再読込と実TOMLの値／削除、既存コメントの保持を照合する。
stdio試験では `tools/list.annotations.readOnlyHint=true`、全guideの応答と、
代表的な書き込みツールが読み取り専用に変わらないことを照合する。

native MCP fixtureの結果:

| approval_policy | Auto + readOnlyHint=true | Auto + readOnlyHint=false |
| --- | --- | --- |
| on-request | 確認0回・実行 | 確認1回。不承認後は未実行 |
| never | 確認0回・実行 | 確認0回・未実行 |

`never` を自動許可とみなしていない。書き込みfixtureには一時ディレクトリの
マーカー以外の副作用を実装していない。

通常／正式自動マージの両方に同じ条件を与えるRun試験:

| 一時configの値 | LVKのfixture variant | thread/start approvalPolicy | native実効値 |
| --- | --- | --- | --- |
| on-request | workspace-write・承認未指定 | null | on-request |
| never | workspace-write・承認未指定 | null | never |
| 未設定 | workspace-write・承認未指定 | null | on-request |
| never | sandbox Auto・承認未指定 | on-request | on-request |
| on-request | workspace-write・承認never | never | never |

fixtureのDEFAULTは隔離試験のためworkspace-writeに固定する。
リポジトリ同梱DEFAULTのdanger-full-accessと混同しない。
起動入力の記録は承認・sandboxだけを抽出し、全文config・環境変数・指示は保存しない。
native出力の一時wireとインスタンスは停止確認後に削除する。

## 設定の不整合と別判断にする最小案

1. **Autoによる上書き（再現済み）**: sandbox未指定／Auto・承認未指定なら
   LVKが `on-request` を明示する。UIが保存するnative値より優先する。
   明示workspace-writeの場合は未指定値をnativeへ委ねる。
   同梱DEFAULTはdanger-full-accessであり、現在の稼働profileは未調査。
   最小案は、Autoを固定presetとして維持するならUIに上書きを明示すること。
   未指定をnative継承に統一する案は実行ポリシー変更となるため別途判断する。
2. **CODEX_HOME（ソース確認、実ユーザー環境は未調査）**:
   設定APIは `dirs::home_dir()/.codex/config.toml`、native Codexは起動環境の
   `CODEX_HOME` を使う。異なる場合は別ファイルになる。
   最小案は選択ホストの実保存先を解決・表示し、設定保存と起動で共有すること。
   Projectスコープや許可済みfile rootsとの整合確認が必要なので今回は変更しない。
3. **継承選択欄（UI再現済み）**: selectの「Follow native config」は空文字を
   通常のselect値として検証し「Choose a listed option.」となる。
   専用「Restore inheritance」では保存・削除・再読込が成功する。
   最小案はこの選択肢も明示unsetへ対応させること。今回のUI試験は専用操作を使う。
4. **native profile互換性（0.163.0で再現済み）**:
   LVKの非空 `profile` は `config.profile` に入るが、このCLIは旧形式として拒否する。
   起動前に拒否されるため承認実効値は得られない。
   最小案は対応版を固定した上で、native profileを新形式のファイル／CLI指定へ
   移す方法を設計すること。LVK variantとnative profileは別物であり、
   今回の10 Runはnative profile未指定で検証する。

書き込み系の一回限りMCP承認を自動承認する変更や、全体のapproval/sandboxを
緩める変更は行わない。上記は実ユーザーの当時の実効値を復元する証拠ではない。

## 実施結果

- utils guide試験: 2 passed。非作成と既存guide再利用を確認。
- stdio codec試験: 2 passed。全guide・引数なし・注釈・書き込みツールを確認。
- executors approval試験: 10 passed / bridge 1 ignored。bridgeは下記UI試験で実行済み。
- utils / mcp / executors のall-targets Clippy: passed (`-D warnings`)。
- Chromium + Rust service UI試験: 1 passed。Userスコープ、保存、継承復帰、再読込、TOMLを確認。
- native config probe: 6優先順位ケースと6旧profile拒否ケースを確認。
- native MCP probe: 上表の4ケースがpassed。
- 実LVK Run試験: 上表5条件 × 通常／正式自動マージ = 10 passed。
  native thread/start入力10件と応答、各Runのsucceeded、get_context完了、承認要求0件を照合。
  自動マージ5件は取消の終端を確認し、各回のtarget SHA不変を確認した。
- server / agent-process-host / stdio MCPの最終ソースビルド: passed。
- Git差分の空白検査、追加Pythonの構文検査、追加Playwright specのPrettier検査: passed。

最終Run試験で実際に使ったバイナリのSHA-256:

| artifact | SHA-256 |
| --- | --- |
| server | `3db6a235f30a5f91b716307edeb028a27d0e7acc4a1ed75273027229d507e2bf` |
| agent-process-host | `2515e420267667f8713ccd1a44c5ea46accf67203effe2e45453e8935d2e5269` |
| vibe-kanban-mcp | `dee69e11ed4930d6a935b76f3f1c43a53f740e85b3e4bdba274a791732204bde` |

fixture整備中には、共有Cargo待ちによるbridge開始timeout、UIの誤ったrole指定、
一時repositoryに `.lvk/.gitignore` がなく自動マージ候補がdirtyになる起動前停止、
担当Run IDの保存直前にGETした一時的なnot foundで失敗した。
roleとfixture初期状態を修正し、読取GETの可視化待ちを追加した後、最終10 Runが成功した。
「Follow native config」の検証エラーと旧native profile拒否は上記の未修正事項として区別する。

全Workspaceの一時DB・native config・wire・サーバーを停止／終了確認後に削除した。
通常のcheckout-local `target/` と依存ライブラリは再利用のため保持する。
全workspace試験、実アカウント／外部モデル、公開・main適用・push・稼働環境再起動は実施していない。
