# MCPで素材ファイルを渡す

Remote MCPとstdio MCPは、既存Attachment/FileServiceを使って素材bytesを保存し、
Workspace内のSession作業ディレクトリへ配置する。保存・配置だけではAgentRunを作らない。
添付はWorkspace共有で、Session間の秘密保管領域ではない。

## 入力と上限

Remoteの `upload_attachment` はProject、安定したrequest_id、original_nameと、
fileまたはdata_base64の一方を受け取る。fileはOpenAIの
[fileParams契約](https://developers.openai.com/plugins/reference#define-file-inputs)で宣言する。
download_url/file_idは必須、mime_type/file_nameは宣言済みの任意項目である。
ホストのMIMEはヒントとし、保存MIMEは既存の拡張子推定を使う。
Library IDやChatGPT内のパスをサーバーが直接読む機能はない。

- inlineは標準base64、decoded bytesで2MiB。stdioも同じ上限。
- fileParamsの取得bytesは20MiB。HTTP Content-Lengthとstream受信の双方を制限する。
- Remote MCPと添付用JSON APIはHTTP本体4MiB。base64には約4/3の増分がある。
- UI multipartは本体全体20MiBなので、ファイルpayloadだけで20MiBを送れる保証ではない。
- ChatGPT・公開proxyの実上限は別途確認が必要。上記はホストの上限を示さない。

取得URLはHTTPS/443、userinfoなし、redirectなし、公開IPのみとし、DNS結果を接続先へ固定する。
既定の許可先は `files.oaiusercontent.com` のみ。追加ホストが実ホストで必要な場合は、
運用者が確認した完全一致ホストをサーバーの `LVK_ATTACHMENT_FILE_HOSTS` にカンマ区切りで指定する。
ツール呼出しから許可先を変更できない。proxy経由でDNS固定を迂回せず、DNSは5秒、取得は30秒に制限する。
実接続でこの既定ホストが使われることは未検証である。任意URLを受け入れる設定にはしない。
URL・base64本文・file_idはreceiptに保存せず、外部取得エラーもURLを含む詳細を返さない。

## 保存、配置、実行

保存はAttachmentResponseのID、原名、推定MIME、サイズ、SHA-256、相対パスを返す。
同一内容でも別名なら別レコードとし、最初の原名/MIMEを引き継がない。
Project関連付けにより未配置ファイルを孤立掃除から保護する。同じProjectの既存Workspaceに付いたUI添付も再利用できる。保護期間はProject参照の寿命で、
Projectが残る限り無期限保持で、TTLはない。Project削除で参照が消え、他のWorkspace参照もなくなったファイルは、次回起動時の既存孤立掃除対象になる。削除時にも参照を再確認し、一覧取得後に付いた参照を誤って消さない。個別の期限付きuploadセッションや定期GCは導入しない。
総保存容量・Project別quotaは追加していない。per-file上限と配置件数上限はディスク総量制限ではないため、容量管理は運用上の残課題である。

`place_attachments` はProject/Workspace/Sessionと1回最大20個のattachment_idsを指定する。
対象の所属を確認し、Sessionの実ディレクトリをWorkspace内に限定する。
不明IDを無視せず、既存先もサイズ/SHA-256を検証する。改変先は上書きしない。
symlinkの添付ディレクトリ・配置先は拒否する。新しい配置先は一時ファイルからno-clobberで公開する。
部分成功は各要素のsaved/associated/verifiedと全体all_verifiedを分けて返す。
一部失敗なら成功済みファイルを削除せず、実行を開始しない。

新規 `start_workspace` のattachment_idsは、作成前にProjectからの利用権限を検証する。
初回・続行ともSessionの作業ディレクトリへの配置を検証してから実行を受け付け、
promptに安全な相対パスを付ける。Remoteの `continue_execution` / `steer_execution` は
選択した最新Runと既存の実行状態・ロックの契約を維持する。
stdioの続行は `run_session_prompt`。添付付きstartにはrequest_id、添付付き続行には
project_id/request_idが必要である。実行中の添付保存・配置から自動でsteerや続行しない。

各操作は既存Operationsのprincipal＋request_idを利用する。同内容の再送はreceiptを再利用し、
異内容なら競合する。保存の内容identityは取得後のbytesのSHA-256と名前/Projectで、期限付きURLは含めない。
URL更新後も同じbytesなら同じ要求になる。任意のexpected_sha256を指定した場合は保存前に一致を確認する。
**成功済みでもupload_attachmentの再送は再取得を行い、署名URL期限切れ等で失敗し得る。**
request_id/file_idだけで新しいbytesの内容identityを推定せず、取得不能時はuploadを再試行せず既存receiptを読む。
このreceipt読取には元ファイル・URL・base64は不要である。
Remoteは `get_operation`、stdioは `get_attachment_operation` を使う。
失敗receiptは新しい実行を勝手に再送しない。配置を修復して改めて行う場合は、新しいrequest_idを明示的に使う。
uncertainは失敗の証明ではなく、対象Workspace/Sessionとreceiptを確認する必要がある。

## 検証と公開後ゲート

`services::file::attachment_tests` は名前・bytes・上限・同名/別名・欠落・改変・symlink、legacy cacheのみの再利用、孤立掃除の参照競合を検証する。
取得不能なfile参照を模した再送が失敗しても、元request_idのget_operationから成功receiptを回収できることを隔離fixtureで確認する。実署名URLのHTTP期限切れ応答は公開後ゲートである。
serverのattachment用fixtureは、新しいtask所有のLVK_INSTANCE_DIRとXDG_CACHE_HOMEを要求し、
実HTTP MCP/stdio codecから保存・配置する。共有DBや共有previewは使用しない。
初回・続行は既存の実行受付adapterを通し、native実行境界だけをfixtureに置き換える。実モデルのCodexやChatGPT/Libraryはこのfixtureの対象ではない。

公開は別の許可作業である。承認された配備後に次を行う。

1. `tools/list` でupload_attachment/place_attachments、fileParams、write scope、既存実行のattachment_idsを確認する。
2. ChatGPT接続のカタログを再取得する。配布Skillはリポジトリの `plugins/lvk/skills/lvk-work` を使って通常の更新手順で反映する。
3. 実ChatGPTの添付とLibraryを分け、SVGとPNGの搬送、サイズ/SHA-256、URL期限、許可ホストを確認する。
4. 初回/続行/実行中の各経路と、保存だけではRunが増えないことを確認する。

サーバー側fixture成功だけで実ChatGPT経路を完成扱いしない。
