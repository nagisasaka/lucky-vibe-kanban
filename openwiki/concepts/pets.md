---
type: concept
title: Pet の形式・安全な取込・保存と帰属
description: Kanban と Workspace に共通の装飾Pet、v1/v2 atlas互換、ZIP取込と添付保持、帰属および検証の限界を説明する。
tags: [pets, sprites, security, persistence, attribution]
verified:
  - by: openwiki/0.7.2
    at: 2026-11-05T07:15:08.684Z
sources:
  - id: openwiki-source-a72a7581cf97c90cae7862d3
    resource: repo://crates/db/src/models/file.rs
  - id: openwiki-source-4294e32943918c50b31813b6
    resource: repo://crates/services/src/services/file.rs
  - id: openwiki-source-99b81b55f2cc14e47bf7c854
    resource: repo://packages/public/pets/lvk-rabbit/ATTRIBUTION.md
  - id: openwiki-source-9273fb1027dd0aefe3f9eac2
    resource: repo://packages/public/pets/lvk-rabbit/LICENSE
  - id: openwiki-source-72cc5d6937055777a10186c1
    resource: repo://packages/web-core/src/features/pets/format.ts
  - id: openwiki-source-6de065eed4aee1b72ae5cc75
    resource: repo://packages/web-core/src/features/pets/library.ts
  - id: openwiki-source-b917b710d762eaf81a4328c9
    resource: repo://packages/web-core/src/features/pets/PetSprite.tsx
  - id: openwiki-source-cbfe71b82135aa5496989f7e
    resource: repo://packages/web-core/src/features/pets/SidebarPet.tsx
  - id: openwiki-source-6a174a6648f527d717aac5b8
    resource: repo://packages/web-core/src/features/pets/zip.ts
  - id: openwiki-source-235a3eb75f22fdecdbd66a8f
    resource: repo://packages/web-core/src/shared/components/EnvironmentPanel.tsx
  - id: openwiki-source-b885bc17dfe85c4ca45223e9
    resource: repo://packages/web-core/src/shared/hooks/useUiPreferencesScratch.ts
  - id: openwiki-source-8022c542a66858f6a97137d8
    resource: repo://tests/pets/browser.mjs
  - id: openwiki-source-f806f5d600e6f619c7c88371
    resource: repo://tests/pets/integration.mjs
  - id: openwiki-source-5ba29f36fe30fd762731147c
    resource: repo://tests/pets/README.md
generated: { by: "codex", at: "2026-11-05T07:15:08.684Z" }
---

# Pet の形式・安全な取込・保存と帰属

Kanban と Workspace は共通の `EnvironmentPanel` から `SidebarPet` を表示する。既定では装飾Pet一体を80pxで表示し、Sessionの数や活動はPetを増やしたり状態を制御したりしない。クリックは一回の waving を再生する。ホスト表示タイトルと鉛筆から全体設定の「ホストとペット」を開き、選択・非表示・取込・標準復帰・九状態のプレビューを操作する。別の Pet 展開行はない。ホスト表示名と実identityの区別は [Web と環境情報](../architecture/web-and-sync.md#ホスト表示名と設定導線)を参照する。[描画と操作](../../packages/web-core/src/features/pets/SidebarPet.tsx)・[共通入口](../../packages/web-core/src/shared/components/EnvironmentPanel.tsx)

## 同梱素材と帰属

LVK Rabbit v1.1.0 は作者 **Masato Fujino** の素材で、commit `c8436fcc5ec04e806c1fb879d05e7c56254026ac` に固定されている。manifest、atlas、CC BY 4.0全文と [ATTRIBUTION](../../packages/public/pets/lvk-rabbit/ATTRIBUTION.md) を製品アセットとして保持する。そこに元の [source](https://github.com/nagisasaka/lvk-rabbit)、[release](https://github.com/nagisasaka/lvk-rabbit/releases/tag/v1.1.0)、annotated tag、ZIPと各ファイルのchecksum、無改変の範囲、素材検査とrenderer受入の境界が記録されている。設定画面にも作者・版・licenseへのリンクを残す。公開releaseの存在だけでは品質検収を意味しない。

同梱作品・manifest・license自体は無改変で、LVKは帰属と検証記録を追加している。公開者はChatGPTによるデザイン支援とCodex/Hatch Petによるアニメーション制作を説明しているが、独立コミュニティ作品でありOpenAI公式作品や推薦ではない。licenseは許諾者が許諾できる権利の範囲に適用される。取込Petは利用者提供の素材であり、この帰属の対象にしたりLVKの同梱素材として再配布したりしない。[素材の正本](../../packages/public/pets/lvk-rabbit/ATTRIBUTION.md)・[license全文](../../packages/public/pets/lvk-rabbit/LICENSE)

## 固定atlasと版互換

新形式を発明せず、既存Codex/Petdex形式の `pet.json` を受け取る。必須は `id`、`displayName`、`description`、`spritesheetPath` で、任意の `spriteVersionNumber` を扱う。元manifest文字列は未知metadataを含め帰属用に保持し、再生用には検証済みfieldだけを投影する。未知versionや独自states/animations/frameWidth/frameHeight/columns/rows、拡大縮小されたatlasは拒否する。[形式と画像検査](../../packages/web-core/src/features/pets/format.ts#L19-L100)・[元metadata保存](../../packages/web-core/src/features/pets/library.ts)

| 版 | version field | 全体寸法 | 列×行 | セル |
| --- | --- | --- | --- | --- |
| v1 | 省略または1 | 1536×1872px | 8×9 | 192×208px |
| v2 | 明示2 | 1536×2288px | 8×11 | 192×208px |

画像は静止PNG/WebPのみで、headerの実寸法を先に確認し、browser decode後も版の寸法と一致を要求する。保存manifestは版を保持し、過去の版番号なしはv1のまま復元する。v2では73使用セルすべてに画素があり、未使用の尾部15セルが完全透明であることを取込時と復元画像load時に確認する。v1は従来の寸法/decode基準を保持し、空セルや不透明尾部だけを理由に過去の有効素材を拒否しない。セルの構造検査は絵の意味の検収ではない。[alpha検証と互換](../../packages/web-core/src/features/pets/format.ts#L225-L286)・[load時の検査](../../packages/web-core/src/features/pets/PetSprite.tsx#L130-L152)

### 九状態の再生とv2視線

フレーム保持時間はnative側のタイミングを採用し、Web galleryの均等割りを使わない。既存設計の参照版は Petdex commit [31c0aee7f4fb387774b87d40e8047dc0f9cea94b](https://github.com/crafter-station/petdex/tree/31c0aee7f4fb387774b87d40e8047dc0f9cea94b) の `src/lib/sprite-atlas.ts`、`sprite-version.ts`、`pet-states.ts` と `packages/petdex-desktop-native/src/sprite.zig` である。v2方向の契約は既存実装依頼の公式Pets sprite-sheet contractを参照したもので、第三者galleryの動きから推測したものではない。実装上の正本は [PET_STATES と frameAt](../../packages/web-core/src/features/pets/format.ts#L1-L18) に置く。

| 行 | 状態 | frames | 保持時間ms |
| --- | --- | --- | --- |
| 0 | idle | 6 | 280,110,110,140,140,320 |
| 1 | running-right | 8 | 120,120,120,120,120,120,120,220 |
| 2 | running-left | 8 | 120,120,120,120,120,120,120,220 |
| 3 | waving | 4 | 140,140,140,280 |
| 4 | jumping | 5 | 140,140,140,140,280 |
| 5 | failed | 8 | 140,140,140,140,140,140,140,240 |
| 6 | waiting | 6 | 150,150,150,150,150,260 |
| 7 | running | 6 | 120,120,120,120,120,220 |
| 8 | review | 6 | 150,150,150,150,150,280 |

Rabbitのrunning素材はタイピング、左右runningは移動、failed/waiting/reviewは別の表現である。取込素材の絵柄の意味はその作者の素材に依存する。

v2の行9は0〜157.5度、行10は180〜337.5度の、各22.5度刻みの**静止視線**であり追加animation loopではない。画面座標の上=0、右=90、下=180、左=270、時計回りである。0度は上向きでneutralではない。idle中だけPet中心からのmouse位置を最も近い方向に丸め、描画sizeの20%以内は通常idleに戻す。画面外へのpointer退出、blur、cancel、mouse以外の入力も復帰条件で、touch追従はしない。クリック反応と明示的な非idle状態が優先する。非表示tabとreduced motionでは追従とframe更新を停止し、視線pose保持にanimation timerは追加しない。v1には方向行を使わない。[方向変換](../../packages/web-core/src/features/pets/format.ts#L229-L246)・[状態優先度とcleanup](../../packages/web-core/src/features/pets/PetSprite.tsx)

## ローカルZIP取込の安全境界

1. 「ホストとペット」でローカルZIPを選択する。`pet.json` はrootまたはpackage folder内に一つだけ必要である。
2. archive名、展開量、manifest、画像header、browser decode、版別alpha構造を検証する。検証完了までuploadしない。
3. 画像だけを既存attachments APIへ保存し、選択・表示・添付ID・元manifest・package rootのLICENSE/LICENCE/COPYINGを既存UI preferencesへ保存する。
4. scratch writerの応答成功後に画面の選択を確定する。失敗時は最後の選択を保持してエラーを表示する。

ZIPは8MiB、32 entries、宣言展開総量12MiB、単一file8MiB、manifest16KiB、保持license64KiB、保存Pet最大12体に制限する。inflate中も出力量を制限しCRCを検査する。絶対path、traversal、backslash、URL、case-insensitive重複名、symlink、暗号化・multipart・ZIP64、未対応圧縮は拒否する。storeとdeflateを扱うが、browserの `DecompressionStream('deflate-raw')` が使えなければ安全に失敗する。余分なarchive entryも検証するがfilesystemへ抽出しない。manifest URLを取得したりコードを実行したりしない。[ZIP境界](../../packages/web-core/src/features/pets/zip.ts)・[取込](../../packages/web-core/src/features/pets/library.ts#L64-L110)

画像はPNGのacTL/fcTL/fdATとWebPのANIM/ANMFを有界chunk走査で拒否し、虚偽のanimation flagだけを信頼しない。browser decode自体が利用できない場合も失敗する。元manifest/license文字列はtextとして表示し、HTMLや取込hyperlinkを実行しない。[画像chunk検査](../../packages/web-core/src/features/pets/format.ts#L112-L224)・[帰属表示](../../packages/web-core/src/features/pets/SidebarPet.tsx)

## 保存・共有範囲とfallback

Pet設定はIssue/Workspace/Session別ではなく、固定UUIDのUI preferences scratch内の `pet_preferences` でinstance全体に共有する。初期snapshotと接続完了後の既存scratch ownerが専用writerを登録し、Pet側は保存応答を待つ。writer欠落・切断・保存失敗を成功として扱わない。古いscratchはfield欠落をnullとして復元し、他の設定は保持する。[writer接続](../../packages/web-core/src/shared/hooks/useUiPreferencesScratch.ts#L27-L103)・[保存応答](../../packages/web-core/src/shared/hooks/useUiPreferencesScratch.ts#L311-L333)・[serde互換](../../crates/db/src/models/scratch.rs#L101-L112)

upload後の設定保存失敗では未参照画像が残り得るため、既存のattachment cleanupが寿命を扱う。orphan候補取得と条件付きdeleteの両方がPet設定の添付ID参照を保護し、snapshot後に参照が増えても削除しない。Petの削除は設定参照を外す操作で、globally deduplicated attachmentをselectorから直接削除しない。[参照条件](../../crates/db/src/models/file.rs#L7-L23)・[削除時の再検査](../../crates/services/src/services/file.rs#L208-L228)

画像欠損・不正v2画像ではRabbitへfallbackするが、保持済み選択metadataは勝手に書き換えない。実表示にはRabbitのaccessible nameと帰属を使い、選択素材のmetadataとは区別する。復元時の壊れた設定も検証して既定値へ戻す。[復元](../../packages/web-core/src/features/pets/library.ts#L30-L63)・[表示fallback](../../packages/web-core/src/features/pets/SidebarPet.tsx#L39-L128)

## 検証の入口と限界

依存導入後、repository rootで次を実行する。Chromiumが必要で、必要なら既存 `LVK_E2E_CHROMIUM` で実行fileを指定する。詳しいfixtureとcleanupは [tests/pets/README.md](../../tests/pets/README.md) を参照する。

```sh
pnpm --filter @vibe/web-core exec vitest run src/features/pets src/shared/lib/petPreferencesPersistence.test.ts
node tests/pets/browser.mjs
node tests/pets/integration.mjs
node scripts/dev-cargo.mjs test -p db pet_preferences --lib
node scripts/dev-cargo.mjs test -p services pet_attachment_survives_cleanup_and_a_stale_orphan_snapshot --lib
```

browser試験は実PNG/WebP decodeとZIP import、保存reload、v1互換、16方向とstatic hold、state/click優先、touch/leave/reduced motion、英日・狭幅・明暗を生成した幾何fixtureで確認する。これらはPet新作素材ではなく、ユーザー制作中のv2画像は未検証である。静止画像の目視とtimer/frame assertionは、人間が連続再生を見たことを意味しない。

integration試験は実Kanban `ProjectUtilities` とWorkspace `RightSidebar`、共通環境panel、preference store、attachment clientを組み合わせるが、HTTP応答とwriter保存先は隔離fixtureである。0/1/複数repo、決定的なterminal既定root、file-tree選択、sidebar開閉と320/375/768/1000pxを扱う。full router/board/mobile sheet、実PTY、live scratch/attachment認可、実providerは検証しない。専用loopback originだけを使い、共有DB・既存ユーザー設定・previewには触れない。終了時にbrowser/HTTP serverを停止し、一時ZIP/CSS/bundleを削除する。スクリーンショットと結果はGit外の `test-results/pets/` と `test-results/pets-integration/` に残る。

今回のコード検証は `7bcd5db39419d50ad265592e288264c63906e1da` 上の重点試験である。source `f8610053323fb06336eb4cd2e376b4cfb1537db0` の開発時には隔離実API保存検証も記録されているが、それを候補での再実行や公開認証後の実画面の検証と読み替えない。Wiki commitとmainへの公開成立も別の結果として確認する。
