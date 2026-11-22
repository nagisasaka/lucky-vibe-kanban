---
type: concept
title: Change Manifest・Run 完了と Wiki
description: Run の意味記録、Git source、統合済み Wiki、receipt と復旧 artifact の責任を説明する。
tags: [repository-memory, manifest, openwiki, reconciliation]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-22T23:18:32.009Z
sources:
  - id: openwiki-source-21149aa9001849998cda42a2
    resource: repo://crates/db/src/models/repository_state/manifests.rs
  - id: openwiki-source-5720d865d1c837f0e6b7dab7
    resource: repo://crates/server/src/routes/integrations/conversation.rs
  - id: openwiki-source-231241ac3c842dcd67525890
    resource: repo://crates/server/src/routes/openwiki.rs
  - id: openwiki-source-3b873a675f85754e9183d162
    resource: repo://crates/server/src/routes/openwiki/management.rs
  - id: openwiki-source-110c99f4b05152335c067c71
    resource: repo://crates/services/src/services/openwiki/sync_input.rs
  - id: openwiki-source-6067be8d6f85caa8ac03638f
    resource: repo://crates/services/src/services/repository_memory.rs
generated: { by: "codex", at: "2026-11-22T23:18:32.009Z" }
---

# Change Manifest・Run 完了と Wiki

[Change Manifest](glossary.md) は coding Run の目的、変更と判断を伝える記録であり、[Wiki](glossary.md) は統合済み source についての知識である。Session の私的な会話、Git の実際の変更、記録された意図は異なる。現行動作は source・tests・設定で確かめ、Manifest や Notes の理由を裏付けのない実装契約にしない。[型](../../crates/utils/src/repository_memory.rs)・[操作の段階](../quickstart.md#操作の段階)

## Run から確定記録へ

通常の成功 coding Run では host が Repo・Workspace・Run と開始 base を固定し、agent の semantic draft に Git から得た changed paths と source commit を結び付ける。draft は目的、要約、振る舞い・構造・不変条件、判断、見送り案、未決点と検証要約を運ぶ。Git identity や対象ファイルをモデルの申告だけで決めない。[完了](../../crates/services/src/services/repository_memory.rs)・[型](../../crates/utils/src/repository_memory.rs)

変更なしでは空の Manifest や余分な commit を作らない。変更があり draft が欠落・不正な場合は原本を保持し、意図・理由・試験が不明と明示した Git-only checkpoint に縮退する。I/O 障害や Git・Wiki guard を無視する意味ではない。DirectFolder、Review、制御 Run などを一律に通常 coding の自動 commit とみなさない。[完了処理](../../crates/services/src/services/repository_memory.rs)・[Workspace](workspace.md)

確定 Manifest は instance DB の `(repository_id, Run ID)` をキーとする不変 record である。同一内容の再送は許し、同じ identity の異なる内容や Workspace 不一致は拒否する。旧 event ファイルを通常読取 fallback としない。[DB model](../../crates/db/src/models/repository_state/manifests.rs)

Git 変更前には staged tree と Manifest を publication record に固定する。回復は marker、親 commit と tree を照合し、後続の編集や mutable draft を前の Run の成果へ混ぜない。任意の `manifest_in_commit_body` は既定 OFF で、ON なら通常 source commit に人が読める意味要約を併記する。DB 正本を commit 本文から再構築する機能ではない。[publication](../../crates/services/src/services/repository_memory.rs)・[整形](../../crates/utils/src/repository_memory.rs)

## 統合と Wiki を分ける

通常 Workspace の `openwiki/` 変更は source 完了・統合の guard が拒否する。source を統合してから専用 maintenance が canonical Wiki を更新し、通常の複数 branch に派生 Wiki を作って merge しない。[guard](../../crates/services/src/services/repository_memory.rs)・[保守](../operations/openwiki-maintenance.md)

[Auto-merge](formal-integration.md) は受付時に source commit と選択 event を固定し、同じ専用会話で候補・検証・有効な Wiki 更新を行う。Host は code SHA、Wiki SHA、完成候補と証拠を照合してから公開する。source 統合、Wiki 完了、Git 公開、receipt と現在の鮮度は別の事実である。

通常 Sync の入力は Repo、maintenance Workspace、source commit、target、scope と選択 Manifest を固定した manifest と番号付き chunk である。Host は identity と各 SHA-256 を照合する。空 chunk は調査不要の証明ではなく、scope に応じて現在の source または差分を確認する。[Sync 入力](../../crates/services/src/services/openwiki/sync_input.rs)

Receipt は Updated / NoOp / Failed を区別し、失敗を成功した acknowledge と扱わない。検証済み no-op は成功になり得る。Current は Wiki ファイルの存在だけでなく、設定、成功履歴、pending と source の一致を踏まえる。過去の成功 receipt は、その後の source 変更に対する現在の鮮度を保証しない。[状態と receipt](../../crates/utils/src/repository_memory.rs)・[Wiki API](../../crates/server/src/routes/openwiki.rs)

## 保存先と回復

Wiki の指示設定は DB に保存され、保存や解除だけでは Git や公開 Wiki を変更しない。明示更新の受付時に DB の上書き、なければ同梱の既定テンプレートを選び、Run 全体で固定する。成功した明示更新だけが適用済みの指示を進め、自動マージは未反映の編集ではなく最後に適用した上書き、なければ既定テンプレートを使う。repository の `openwiki/INSTRUCTIONS.md` は暗黙の入力ではなく、設定画面からの明示取り込みと一時配置後の原状復元に使う。[設定](../../crates/server/src/routes/openwiki/management.rs)・[選択と完了](../../crates/server/src/routes/openwiki.rs)・[保守](../operations/openwiki-maintenance.md)

確定 Manifest、Wiki の可変 state・receipt・publication と coding Run の永続診断は instance DB に置く。固定 Sync 入力、source publication、setup 原本・復元 journal と Bootstrap report は復旧用の管理 tmp、再生成可能な文書索引は管理 cache に置く。共有 persistent はユーザーが保持する成果物用で、内部 journal の代わりではない。[path と cleanup](../../crates/utils/src/repository_memory.rs)・[DB state](../../crates/db/src/models/repository_state.rs)

Workspace 完了 lock、Repo の Wiki lock、短い publication lock は異なる寿命を持ち、モデルの実行中に publication lock を保持しない。復元・公開・照合と後続読取の完了を確認してから管理 artifact を掃除する。中断や失敗では原本を保持し、時間経過や Run 終端だけで安全に削除できると判断しない。[lock と cleanup](../../crates/utils/src/repository_memory.rs)・[保守の回復](../operations/openwiki-maintenance.md)

読取専用 observer は DB 設定がなければ未設定とし、閲覧だけで管理ディレクトリを作成しない。逆に必要な凍結入力の欠損や破損は成功へ補完しない。[reader](../../crates/db/src/models/repository_state.rs)

通常の source event や backlog の発生は別 Wiki agent の自動起動を意味しない。保存済み owner の回復と、新規の明示保守受付を区別する。Bootstrap の索引と Review report は探索・回復用の入力であり、確定 Manifest や公開 Wiki の代替ではない。[受付と回復](../../crates/server/src/routes/openwiki.rs)・[Bootstrap](../../crates/server/src/workflow_runtime/bootstrap.rs)

Git と DB の間の crash、固定入力の変更、lock と cleanup は個別の fixture で検査する。これらは live DB の移行や実 provider の受入を証明しない。[Sync tests](../../crates/services/src/services/openwiki/sync_input.rs)・[memory tests](../../crates/services/src/services/repository_memory.rs)
