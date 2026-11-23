---
type: concept
title: Runtime access
description: 外部 MCP の直接 file・process 操作と権限境界。
tags: [runtime, mcp, authorization, processes, lifecycle]
verified:
  - by: openwiki/0.9.0
    at: 2026-11-23T00:23:16.293Z
sources:
  - id: openwiki-source-076036ed7bdac82d628d39d1
    resource: repo://crates/server/src/remote_mcp/environment_process.rs
  - id: openwiki-source-0801a7e5bbcc064c12effc80
    resource: repo://crates/server/src/remote_mcp/environment.rs
generated: { by: "codex", at: "2026-11-23T00:23:16.293Z" }
---

# 実行環境へのアクセス

Runtime Access は認証された外部 MCP client が LVK の OS user 権限でファイルを扱い、既存 program を直接起動・観測・停止する機能である。Agent Run を作らず、OS sandbox でもない。出力は参照データであり、操作の認可や成功の証明ではない。[API](../../crates/server/src/remote_mcp/environment.rs)・[操作の段階](../quickstart.md#操作の段階)

## 設定と権限

`files_enabled` と `execution_enabled` は独立し、既定は両方 ON、直接 file API の roots は `/` である。roots は1〜64個の絶対 directory とし、保存時に canonicalize する。**file roots は直接 file API の範囲であり、起動した program のアクセスを閉じ込めない。** File を OFF にしても program の OS 書込みは禁止されない。[設定](../../crates/server/src/remote_mcp/environment.rs)

全 tool は `lvk:read` と `lvk:runtime:read`、直接変更にはさらに `lvk:write`、起動・停止・self-restart には `lvk:runtime:execute` を要求する。能力参照以外は明示 Project を照合するが、Project は filesystem tenant 境界ではない。共有 device・DB・配置では他の作業への影響を確認し、必要に応じ環境や作業順序を分ける。[tool 定義](../../crates/server/src/remote_mcp/environment.rs)

管理者は Settings → MCP 連携から設定を保存する。API は admin と expected revision を再確認し、設定と audit を同じ transaction に保存する。未保存なら既定を使うが、破損 row や DB 障害を緩い既定値に戻さない。各要求と副作用直前に再照合する。Execution OFF は以後の観測・出力・停止も拒否するが、既存 process を自動 kill しない。[設定 API](../../crates/server/src/remote_mcp/environment_settings.rs)

## ファイルと管理 Workspace

直接 file read は list/read、変更は write/delete である。管理 Workspace 自身と子への直接変更、削除時はその祖先も拒否する。Workspace 内の変更は担当 agent の作業として扱う。root から fd 相対に symlink を辿らず、特殊 file や proc/sys を拒否するが、hard link・bind mount・別の OS writer に対する完全隔離ではない。[境界](../../crates/server/src/remote_mcp/environment.rs)・[file 実装](../../crates/server/src/remote_mcp/environment_files.rs)

読取は既定16 KiB、最大512 KiBで、cursor は inode・metadata・anchor による rotation 等を検査する。immutable snapshot や全 rewrite の検出ではない。write は既存 parent 内の UTF-8 通常 file を最大512 KiBで作成または原子的に置換する。期待 snapshot の比較は best effort で kernel CAS ではない。recursive delete は明示が必要で、失敗しても一部が削除済みの場合がある。[file 実装](../../crates/server/src/remote_mcp/environment_files.rs)

## Process と結果

`run_process` は絶対 executable と cwd を要求する。起動時に管理 Workspace 自身または子の cwd を拒否するが、祖先や sibling は許され、起動後の cwd を監視しない。program は cwd と file roots の外も扱える。executable allowlist や command 語の検閲はない。[dispatch](../../crates/server/src/remote_mcp/environment.rs)

子の環境は clear し、`PATH=/usr/local/bin:/usr/bin:/bin` と `LANG=C.UTF-8` を渡す。server の HOME、provider 資格情報や開発環境を継承する前提にしない。必要な実行 account と設定 directory は事前に用意する。有限実行は1〜3600秒、service は1〜86400秒で、期限後 TERM、必要なら5秒後に KILL する。[process](../../crates/server/src/remote_mcp/environment_process.rs)

観測・停止は同 UID と process identity を確認し、PID 1、LVK 自身と祖先を保護する。pidfd による停止は一 PID のみで、子孫や process group の停止保証ではない。停止 receipt は終了証拠ではなく、再観測する。出力は stream ごと最大1 MiB、最大128記録の揮発 buffer で、server 再起動後に出力と monitor を復元しない。[process registry](../../crates/server/src/remote_mcp/environment_process.rs)

起動 receipt は開始受付であり health や test pass ではない。変更後に結果が確定できない場合は `uncertain` / `outcome: unknown` を維持し、同じ request ID・同じ入力の receipt と実状態を確認する。新しい ID で盲目的に再実行しない。[operation](../../crates/server/src/remote_mcp/operations.rs)・[外部 MCP](../integrations/remote-access.md)

開発 instance の `self_restart` は専用 switch engine を使い、一般 `run_process` の代用ではない。受付と cutover 成功を分け、再接続後に元の request ID で結果を読む。[切替手順](../../deploy/developer/switch/README.md)・[bridge](../../crates/server/src/remote_mcp/environment_switch.rs)

Provider の設定発見、profile override、起動準備と実際の成功は別段階である。CLI と設定 directory は実行 account から利用可能でなければならない。[provider の準備](../integrations/agent-providers.md)

隔離 HTTP・設定 fixture は scope、Project、path、receipt を検査するが、実 Auth0、稼働 server の restart や共有 DB の安全を証明しない。[tests](../../crates/server/src/remote_mcp/environment_tests.rs)・[設定 tests](../../crates/server/src/remote_mcp/environment_settings_tests.rs)
