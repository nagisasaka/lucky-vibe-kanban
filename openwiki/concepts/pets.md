---
type: concept
title: Pet
description: Sidebar Pet の形式、安全な取込、保存と帰属。
tags: [pets, sprites, security, persistence, attribution]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-72cc5d6937055777a10186c1
    resource: repo://packages/web-core/src/features/pets/format.ts
  - id: openwiki-source-6a174a6648f527d717aac5b8
    resource: repo://packages/web-core/src/features/pets/zip.ts
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# Pet の形式・安全な取込・保存と帰属

Pet は Kanban と Workspace の共通 EnvironmentPanel に表示する装飾である。Session 数に応じて Pet を増やすのではなく、既存の環境観測から表示状態を選ぶ。素材の作成・取込や表示を変更するときに読む。[Sidebar](../../packages/web-core/src/features/pets/SidebarPet.tsx)・[操作の段階](../quickstart.md#操作の段階)

## 形式と再生

`pet.json` は `id`、`displayName`、`description`、ローカル `spritesheetPath` を必要とする。未知 version や独自の frame・state 配置は拒否する。元 manifest は未知 metadata も含めて保持し、再生には検証済み field を使う。[形式](../../packages/web-core/src/features/pets/format.ts)・[library](../../packages/web-core/src/features/pets/library.ts)

| 版 | version | atlas | 配置 |
| --- | --- | --- | --- |
| v1 | 省略または1 | 1536×1872 px | 8×9、192×208 px セル |
| v2 | 明示2 | 1536×2288 px | 8×11、同じセル寸法 |

静止 PNG/WebP の header と browser decode の両方で寸法を検査し、アニメーション画像は拒否する。v2 は使用セルの画素と未使用セルの透明性を検査する。旧 v1 は寸法と decode の互換基準を維持する。構造検査は絵の意味や品質の承認ではない。[画像検証](../../packages/web-core/src/features/pets/format.ts)

九状態の frame timing は固定である。v2 の追加2行は16方向の静止視線で、上を0度として時計回りに22.5度刻みで選ぶ。idle/waiting 中の mouse に反応し、中心付近や pointer 離脱で通常 animation に戻る。活動状態とクリック反応が優先し、touch、非表示 tab、reduced motion の扱いは別に制限する。[timing と方向](../../packages/web-core/src/features/pets/format.ts)・[再生](../../packages/web-core/src/features/pets/PetSprite.tsx)

Visible 中の HTTP 失敗は failed、fresh な正の running Session 数は running、それ以外は idle である。古い・非表示・欠損は通信断の証拠ではない。idle の一定時間後は waiting になり、クリックや window 移動に応じた表示は既存の behavior が所有する。[状態](../../packages/web-core/src/features/pets/petStatus.ts)・[動作](../../packages/web-core/src/features/pets/usePetBehavior.ts)

## ZIP 取込と保存

1. 「ホストとペット」でローカル ZIP を選ぶ。root または package folder に `pet.json` が一つ必要である。
2. archive 名、実展開量、manifest、画像 header・decode・版別 atlas を検証する。成功前に upload しない。
3. 画像を既存 attachments API に保存し、選択と元 manifest、package の license text などを UI preferences に保存する。
4. scratch writer の成功応答を確認して選択を確定する。失敗時は元の選択を保ち、エラーを確認して再試行する。

ZIP は8 MiB、32 entries、実展開合計12 MiB、単一 file 8 MiB、manifest 16 KiB、license 64 KiB、保存 Pet 12体を上限とする。実出力量と利用可能な圧縮 size に対する比率も検査し、絶対 path、traversal、backslash、URL と大文字小文字を無視した重複名を拒否する。filesystem 抽出、manifest URL の取得やコード実行は行わない。ZIP の構造解釈は `fflate` が担当し、独自 CRC・Unix 属性検査を保証しない。[ZIP](../../packages/web-core/src/features/pets/zip.ts)・[取込](../../packages/web-core/src/features/pets/library.ts)

設定は instance 共通の UI preferences Scratch に保存する。画像 upload 後に設定保存が失敗すると未参照画像が残り得る。参照中の attachment は既存 cleanup が保護し、Pet 削除は設定参照を外す操作である。画像欠損や不正な v2 は同梱 Rabbit に fallback するが、保持した選択 metadata を勝手に書き換えない。[preferences](../../packages/web-core/src/shared/hooks/useUiPreferencesScratch.ts)・[attachment cleanup](../../crates/services/src/services/file.rs)・[表示](../../packages/web-core/src/features/pets/SidebarPet.tsx)

## 同梱素材と検証

同梱 LVK Rabbit v1.1.0 は Masato Fujino の CC BY 4.0 素材である。固定 source、license、checksum、無改変範囲と素材・renderer の検査記録は [ATTRIBUTION](../../packages/public/pets/lvk-rabbit/ATTRIBUTION.md) にある。利用者が取込む素材はこの帰属の対象や自動再配布物ではない。

[Pet の試験手順](../../tests/pets/README.md)は browser decode、ZIP、版互換、状態・方向と画面配置を検査する。隔離 fixture の HTTP や保存先は mock であり、実認証、実機、利用者の素材、連続再生の人手受入を証明しない。
