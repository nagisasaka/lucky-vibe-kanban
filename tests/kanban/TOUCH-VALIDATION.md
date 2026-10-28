# スマホKanbanのタッチ判定検証（2026-10-27）

## 原因と最小修正

- `KanbanCardContent.tsx:187` のタグ用buttonは、`SearchableTagDropdown` →
  Radix `DropdownMenuTrigger` を使用する。Radixはclickではなくpointerdownで
  開く。修正前の実画面の縦スワイプでは、`pointerdown → focusin → touchstart →
  pointermove/touchmove → touchend` の順序を記録した。指を離す前にメニューへ
  フォーカスが移っていた。タグ幅計測・+N表示は原因ではなく、変更していない。
- `KanbanGrid.tsx:216` は `dragged` だけで詳細のclickを許可する。
  修正前に、指を7 CSS px動かして戻すと `pointermove → pointermove → touchend →
  mousedown → click` が届き、詳細へ遷移した。この小移動ではtouchmoveが届かず、
  DnD未開始のままclickが通る。大きなスワイプは元からブラウザがclickを抑える。
- `SearchableTagDropdown.tsx:94` はtouchのRadix pointerdown既定処理だけを止め、
  ブラウザのtouch由来clickで開く。マウスとキーボードのRadix処理は維持する。
- 新規 `useTouchClickGuard.ts` を `KanbanGridCard` のcaptureに適用する。
  移動が5pxを超えたら、元へ戻ってもclickを止める。cancel・複数指もタップとせず、
  次のpointerdownで再判定する。子buttonのclick前にも抑制できる。
  キーボードのdetail=0、次のマウス操作は通す。
- TouchSensorのdelay=200ms/tolerance=5、DnD配置・アニメーション・保存、CSSは変更しない。
  touch-actionはブラウザのnative panningを維持する。独自timerやジェスチャー体系は追加しない。

## 実行環境・コマンド

Workspace: `868c1e1d-f985-49a5-8dc0-d8561f9ea631`。
開始HEAD: `a3247dadd5b4385e07a7848fd0009aa07d8ffa71`。
source frontendは本WorkspaceのVite。既存Chromiumは `/opt/lvk-browsers`。
バックエンドは既存releaseからWorkspaceの使い捨てinstanceへ専用コピーを作り、
SHA-256一致を検査してコピーだけを起動する。稼働中の実行ファイルへの書込・直接起動は行わない。
ビルドrevisionは
`0a111959a108a8344c9a7604a44092a92707ac2d`、dirty=false。
`crates/`・Cargo・生成API型に開始HEADとの差分がないことを照合し、runnerでも再検査する。

```sh
pnpm install --frozen-lockfile
pnpm run web-core:check
pnpm run ui:check
pnpm run local-web:check
pnpm run ui:lint
pnpm run local-web:lint
pnpm --dir packages/local-web exec eslint --config .eslintrc.cjs ../web-core/src/features/kanban/ui/KanbanGrid.tsx ../web-core/src/features/kanban/ui/useTouchClickGuard.ts --no-ignore --max-warnings 0
node --test tests/kanban/order.test.mjs
PLAYWRIGHT_BROWSERS_PATH=/opt/lvk-browsers node tests/kanban/touch-guard.mjs
PLAYWRIGHT_BROWSERS_PATH=/opt/lvk-browsers node tests/kanban/issue-classification.mjs
PLAYWRIGHT_BROWSERS_PATH=/opt/lvk-browsers node tests/kanban/readability.mjs
PLAYWRIGHT_BROWSERS_PATH=/opt/lvk-browsers LVK_TEST_SERVER=/var/tmp/lvk-instances/dev/releases/78979867-1a39-476c-9a93-fcc72586155d/server node tests/kanban/touch.mjs
```

型3種・lint3種・局所rankテスト4件・上記コンポーネントChromium試験はPASS。
`touch-guard.mjs` は4pxの揺れを許可し、7px往復・cancel・複数指の誤clickを防ぎ、
次のタップ・マウス・キーボード操作が成功することを確認した。
`issue-classification.mjs` はタグ編集の既存キーボード処理等、
`readability.mjs` は本番部品のマウス/キーボードDnDとフォーカス維持等を確認する。
これらはcomponent fixtureであり、実アプリ/API E2Eと区別する。

## 実画面E2Eと証跡

新規の使い捨てSQLite・instance、OS採番のloopbackポートを使用する。
既存のmain/稼働DB/他Workspace/preview2/共有previewを変更しない。
起動環境はallowlistで隔離し、モデル探索CLIを無動作fixtureに置換する。
Issue/Tagのfixtureデータを実APIで作成する。APIレスポンスのmockはない。
Chromium mobile emulationは390×844、isMobile/hasTouch=true。
CDP `Input.dispatchTouchEvent` によりtrusted touchを送り、native scroll offsetも検査する。
デスクトップは1280×900。

最終 `touch.mjs` はexit 0、実画面の全assertionがPASS。縦/横scroll、7px往復、cancel後の再tap、詳細tap、タグのrelease後openとキーボード編集、長押し列移動、デスクトップclick/マウス列移動/キーボード列移動、実API保存を確認した。キーボードdrop後のfocusも最終実行で維持した。

`.build/touch-validation/` に結果を保存する。

- `before-results.json`, `before-run.log`: 修正前の両症状の再現とイベント順。
- `results.json`, `run.log`: 最終実画面試験結果とtouchイベント順。
- `mobile.png`, `desktop.png`: 実画面のスクリーンショット。
- `revision.json`, `tested-source-hashes.json`: 実行ソース、コピー元/実行binaryパス、binary SHA-256、バックエンドbuildの対応。
- `cleanup.json`: Chromium/Vite/backend終了と使い捨てinstance削除の確認。
- `same-column-results.json`, `same-column-diagnostic.log`, `keyboard-debug.log`:
  別Issueに関わるDnD調査の失敗記録。これらの失敗をPASSへ読み替えない。

DnD調査では同一列内の保存順位のassertionが失敗した。また、マウス列移動のAPI保存後、
続くキーボードドラッグが画面内では移動前の列をsourceとして参照する状態を観測した。
移動/反映を扱う別Issueへ変更を混在させず、既存 `isolated.mjs` と同じく保存確認後に
reloadして次のDnDを検査する。今回の検証はこのlive投影問題の解消を保証しない。

実スマホの触り心地・Safari・人間による見た目確認は未検証。
Rustビルド、全体E2E、mainマージ、デプロイは実施しない。
