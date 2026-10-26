import { useEffect, useState } from 'react';
import { create, useModal } from '@ebay/nice-modal-react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@vibe/ui/components/KeyboardDialog';
import { Button } from '@vibe/ui/components/Button';
import { defineModal } from '@/shared/lib/modals';
import {
  issueArchiveApi,
  refreshIssueArchive,
  receiptMessage,
  type PreparedIssueArchive,
  type IssueArchiveReceipt,
} from '@/shared/lib/issueArchiveApi';
interface Props {
  projectId: string;
  issueId: string;
}
const Impl = create<Props>(({ projectId, issueId }) => {
  const modal = useModal();
  const [archiveWorkspaces, setArchiveWorkspaces] = useState(true);
  const [prepared, setPrepared] = useState<PreparedIssueArchive | null>(null);
  const [error, setError] = useState('');
  const [requestId, setRequestId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [finished, setFinished] = useState(false);
  useEffect(() => {
    let current = true;
    setPrepared(null);
    setError('');
    issueArchiveApi
      .prepare(projectId, issueId, archiveWorkspaces)
      .then((value) => {
        if (current) setPrepared(value);
      })
      .catch((err) => {
        if (current) setError(String(err.message ?? err));
      });
    return () => {
      current = false;
    };
  }, [projectId, issueId, archiveWorkspaces]);
  const close = () => {
    if (!busy) {
      modal.resolve(finished);
      modal.hide();
      modal.remove();
    }
  };
  const accept = (receipt: IssueArchiveReceipt) => {
    refreshIssueArchive(projectId);
    const message = receiptMessage(receipt);
    if (receipt.status === 'succeeded') {
      if (message) {
        setFinished(true);
        setError(message);
      } else {
        modal.resolve(true);
        modal.hide();
        modal.remove();
      }
    } else {
      setError(message);
    }
  };
  const confirm = async () => {
    if (
      !prepared ||
      prepared.archive_workspaces !== archiveWorkspaces ||
      requestId
    )
      return;
    const id = crypto.randomUUID();
    setRequestId(id);
    setBusy(true);
    try {
      accept(
        await issueArchiveApi.confirm(
          projectId,
          issueId,
          prepared.confirmation_token,
          id
        )
      );
    } catch (err) {
      setError(
        `結果不明。一部の物理削除が完了した可能性があります。再実行せず「結果を確認」を使ってください。request_id: ${id}\n${String(err)}`
      );
    } finally {
      setBusy(false);
    }
  };
  const inspect = async () => {
    if (!requestId) return;
    setBusy(true);
    try {
      accept(await issueArchiveApi.receipt(requestId));
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog
      open={modal.visible}
      onOpenChange={close}
      role="dialog"
      aria-modal="true"
      aria-labelledby="issue-archive-title"
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle id="issue-archive-title">Issueをアーカイブ</DialogTitle>
          <DialogDescription>
            仕事のステータスは変更しません。本文・コメント・関連・履歴は保持します。
          </DialogDescription>
        </DialogHeader>
        <label className="flex gap-base items-center">
          <input
            type="checkbox"
            checked={archiveWorkspaces}
            disabled={busy || !!requestId}
            onChange={(e) => {
              setPrepared(null);
              setArchiveWorkspaces(e.target.checked);
            }}
          />
          配下のワークスペースもアーカイブする
        </label>
        <p>
          {archiveWorkspaces
            ? '未アーカイブの配下Workspaceをアーカイブし、ワークツリーを物理削除します。DB履歴・Git branchは保持します。使用中の対象がある場合は全体を拒否します。'
            : 'Issueのみアーカイブします。Workspace・プロセス・ファイルは変更しません。'}
        </p>
        {prepared && (
          <ul className="max-h-48 overflow-auto text-sm">
            {prepared.workspaces.map((ws) => (
              <li key={ws.workspace_id} className="break-all">
                {ws.workspace_id} / {ws.branch} / {ws.path ?? 'worktreeなし'}
                {ws.archived
                  ? '（既にアーカイブ済み・変更なし）'
                  : !archiveWorkspaces
                    ? '（変更なし）'
                    : ''}
              </li>
            ))}
          </ul>
        )}
        {error && (
          <p role="alert" className="whitespace-pre-wrap text-error">
            {error}
          </p>
        )}
        <DialogFooter>
          <Button variant="outline" onClick={close} disabled={busy}>
            {finished ? '閉じる' : 'キャンセル'}
          </Button>
          {requestId && !finished ? (
            <Button disabled={busy} onClick={inspect}>
              結果を確認
            </Button>
          ) : (
            !finished && (
              <Button disabled={!prepared || busy} onClick={confirm}>
                アーカイブ
              </Button>
            )
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
});
export const IssueArchiveDialog = defineModal<Props, boolean>(Impl);
