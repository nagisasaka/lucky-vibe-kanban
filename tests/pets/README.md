# Pet の隔離ブラウザ検証

リポジトリルートで実行する。

```sh
node tests/pets/browser.mjs
node tests/pets/integration.mjs
```

Playwright Chromium を使用する。必要なら既存の `LVK_E2E_CHROMIUM` に実行ファイルを指定できる。各スクリプトは一時的なローカル HTTP サーバー・ブラウザーを作成し、終了時に停止する。一時 ZIP・CSS・bundle は削除する。外部 origin の通信は拒否する。既存 LVK サーバー、共有 DB、ユーザー設定は使わない。画像・結果は Git 管理外の `test-results/pets/` と `test-results/pets-integration/` に残す。

`browser.mjs` は実 `PetSprite` / `SidebarPet` / ZIP parser / 画像デコードを使用する。設定 store と attachment transport、設定保存 writer を mock し、専用 origin の localStorage だけに保存する。9状態の行とフレーム進行、クリック反応、reduced motion、ライト／ダーク、選択・非表示・復元・インポート・不正 ZIP・画像欠損時の帰属表示・保存失敗を検証する。

`integration.mjs` は実 `ProjectUtilities`（Kanban）と `RightSidebar`（Workspace）、共通の `UtilitySidebar` / `EnvironmentPanel` / `SidebarPet`、実 `useUiPreferencesStore` / file selection provider / attachment API client を組み合わせる。Kanban の実 Close utilities ボタンと、実 store の `isRightSidebarVisible` / `toggleRightSidebar` に接続した fixture shell で表示寿命を検証する。実 writer 登録の保存先を専用 localStorage にし、実 v1 ZIP import の選択・非表示・標準復帰が画面間で共有されることと選択の reload 復元を確認する。320 / 375 / 768 / 1000 px で水平 overflow、320 / 375 px では設定 popover の viewport 内配置も確認する。

Terminal / Logs / Wiki の provider hooks は inert mock。環境情報・リソース・quota・attachment upload/file は専用 HTTP fixture response。Git / Files / Notes などは実折畳み header のまま閉じ、子機能を実行しない。未使用の executor virtual schema は空 fixture を与える。実画面全体の router、Kanban board、Workspace layout、モバイル sheet、drag/drop、端末起動、live backend・scratch transport の永続化や attachment 認可は検証していない。狭幅 shell は本文を隠して sidebar を置く検証用構成であり、製品のモバイル画面全体を再現したものではない。

スクリーンショットは静止画の検収、timer / frame assertions は自動再生挙動の検証である。人間が連続再生を目視したことを意味しない。
