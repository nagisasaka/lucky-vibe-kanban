# Issue 採番の永続高水位

Project UUID ごとの `projects.issue_number_high_water` を採番の正本とする。
表示用 Project key の変更、Issue archive/unarchive、最高番号の削除、全 Issue
削除は高水位を下げない。Project 自体の削除はその Project の寿命を終了する。
既存 Issue の UUID、issue_number、simple_id は migration で変更しない。

通常作成は `models::issue_number::next_issue_number` を呼ぶ。
UI、stdio MCP、remote MCP が利用する `create_local_issue_with_tags` と、
システム生成の integration / workspace usage 作成は、既存の
`BEGIN IMMEDIATE` transaction 内で高水位更新と Issue INSERT を行う。
番号は i32::MAX まで。上限到達時は明示的に失敗し、wrap や MAX(row) への
fallback はしない。直接 INSERT/番号更新でも trigger が高水位を記録するが、
直接 SQL による過去番号の指定は通常作成 API の契約外である。

高水位更新と INSERT が commit された番号は永久欠番となる。
INSERT や後続 Tag 検査が失敗した場合、呼出側は transaction 全体を rollback
する。未 commit の予約は外部に公開せず、rollback で予約番号も戻すため、
次の成功作成でその未公開番号を使うことは許容する。予約だけの commit は禁止。

## Migration と過去履歴の限界

`20261027090100_issue_number_high_water.sql` は archive を含む現存 Issue の
最大値（空なら 0）で各 Project を初期化する。新規 Project は 0 から始まる。
既存 remote MCP create receipt は Issue UUID を保存するが割当番号は保存しない。
access audit も削除した旧行の番号を復元する完全な履歴ではない。したがって、
この schema 内の記録だけから削除済み最大番号を推定しない。

導入前に削除された Issue がある Project は、現存最大値を超える過去番号が
存在する可能性があり、過去全体の非再利用は保証できない。新 counter 導入後に
成功して永続化した番号は削除を跨いで非再利用を保証する。

運用導入前には Project ごとに信頼できる保持履歴（当時の DB snapshot、番号と
Project UUID を照合できる記録など）の有無を報告し、確認できた過去最大値が
現存最大値を超える場合は、その値以上への高水位引上げを導入計画に含める。
履歴不足なら初期値、根拠、保証できない期間を明記し、運用側と初期 floor を
決める。任意の snapshot の番号だけで完全な履歴とみなさない。
本変更は履歴探索、liveDB の書換え、旧 LOCAL-205 の復元や現205の再採番を
実施しない。番号を持たない receipt から UUID 順序や作成時刻で補間もしない。

## 回帰検証

`node scripts/dev-cargo.mjs test -p db issue_number` は一時 SQLite DB で
最高削除、全削除後の接続再生成、archive、key 変更、Project 独立性、24 並列作成、
INSERT 失敗後の rollback、上限、移行前後の既存識別子保持と履歴不足を検証する。
既存 `issue_number` migration テストも同時に実行する。
