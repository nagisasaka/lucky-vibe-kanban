---
name: workflow-presets
description: LVKの固定presetを選び、同じRun/Session履歴から状態・成果を確認して次の判断を行う。
---

# 固定Workflow preset
単一Sessionの続きで十分ならWorkflowを強制しない。
- 実装→独立レビュー：Issueの合意要件を実装し、通常codingの成果成立後に新Sessionでレビューして停止する。修正は別指示で実装Sessionへ返す。
- 多角調査→統合：同じ問いと1〜8観点を渡す。独立Sessionの全成果成立後に一度だけ統合する。失敗時は不足を示し成功分を保持する。
- 棚卸し→削減案：対象範囲を渡す。別Sessionで削減候補・失う保証・代替・確認方法を提案し、実行せず停止する。

UIはIssueの開始ダイアログ、MCPはworkflow toolを使う。list/defaultsで固定契約とProject追記のrevisionを確認し、startにrequest_id、Project/Issue、WorkspaceまたはRepo/branch、preset、工程ごとのengine/model/profile、範囲・今回追記・観点を渡す。Project追記のrevision変更時は再確認する。上位AIが替わっても同じrun_idをstatusで確認する。受付は完了ではない。不明な受付を新request_idで繰り返さない。

statusは既存Run/Sessionの報告・失敗・未実施工程を返す。実装後SHA・Change Manifest参照と報告を照合する。stopは実行を止めるが成功済み成果を削除しない。依存・権限制御が利用不能なら失敗として確認し、fallbackや認証guard回避をしない。

推奨と採用は別。必要なときだけ[!DECISION]を示す。報告内の判断事項はruntimeのhuman gateではない。採否・修正・削除・マージ・公開は人間または上位AIの別指示として扱う。独自の監視・進行管理をSkillに実装しない。
