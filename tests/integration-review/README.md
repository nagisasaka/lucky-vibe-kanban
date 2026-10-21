# 競合解消レビューの隔離ブラウザ検証

実行はこのcheckoutのルートから行います。稼働中LVKのURLやDBは指定しません。

```sh
pnpm install --frozen-lockfile
pnpm --dir packages/local-web run build
# AGENTS.mdに従い、利用可能ならRUSTC_WRAPPERとSCCACHE_DIRを設定
cargo build --bin agent-process-host
# Chromiumが未導入なら、Workspace内の専用PLAYWRIGHT_BROWSERS_PATHにインストール
# 既存Chromiumを使う場合はLVK_E2E_CHROMIUMで実行ファイルを指定
python3 tests/integration-review/run.py
```

`run.py`は`.build/integration-review/local9-scenario-*`に新しいinstanceを作成し、
Rustのignored test `isolated_review_browser`を単独実行します。テストが実アプリの
`routes::router`・`server_access::Access`と変更済みのbuilt frontendをloopbackのport 0で
起動します。認証は既存`tests/todos/isolated.mjs`と同じ正式localモードとCSRFヘッダーを使用し、
製品の認可処理やAPIにテスト用例外を追加しません。public Basic/Auth0の検証ではありません。
`server` mainの全監視ループは起動せず、Integrationの本物の`advance`をテストが駆動します。

既存`isolated_merge_approval_publication`のfixtureを共有します。一時Gitに独立した4 branchを
作り、3番目のsourceで同じ行への実競合を発生させます。providerの起動・回答だけは固定イベントで
置換し、解消ファイルをstageします。その後の候補commit生成・解消説明/diff保存・API判断・
取消・公開は実装本体を通します。実AIや実エージェントを含むE2E成功の証拠ではありません。

ブラウザで確認するケース:

- 保存説明と未検証表示を読む → native detailsをEnter/Spaceで開閉 → reload → 承認。
  4番目の無競合sourceまで進み、対象branchが最終候補に一致し、全sourceの祖先関係を保つ。
- 別fixtureで中止。対象branchは開始時のcommitのまま、公開されない。
- レビュー中に実Gitで別候補を作り、実`record_review`で記録。ブラウザの読取pollのみ一時保留し、
  古い画面から承認を送ると実APIが拒否する。reloadで古い候補の操作が消えることを確認して中止。
- 既存保存データで説明が欠落した状態をfixtureで作る。欠落表示を確認して中止。
  現行provider parserは空summaryを拒否するので、これは旧記録等の表示互換の検証である。

ブラウザはAPI responseをmockしません。APIを通さないfixture作成（source選択、providerイベント、
古い説明の欠落、候補更新）はRust test内だけです。Integration新規受付UIと実AIの意味的な解消品質は
このE2Eの検証対象外です。全ケースでSQLite/Gitとブラウザは同一の隔離instanceを参照します。

ログとスクリーンショットは`.build/integration-review/`に保存します。終了時にテストプロセス群と
一時instanceを片付けます。通常の`target/`とbuilt frontendは次の開発に再利用します。
