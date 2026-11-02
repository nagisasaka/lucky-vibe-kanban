import {
  receiptMessage,
  type IssueArchiveReceipt,
} from '@/shared/lib/issueArchiveNotice';
import { useTranslation } from 'react-i18next';
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
  type PreparedIssueArchive,
} from '@/shared/lib/issueArchiveApi';
interface Props {
  projectId: string;
  issueId: string;
}
const Impl = create<Props>(({ projectId, issueId }) => {
  const modal = useModal();
  const { t } = useTranslation('tasks');
  const [archiveWorkspaces, setArchiveWorkspaces] = useState(true);
  const [prepared, setPrepared] = useState<PreparedIssueArchive | null>(null);
  const [error, setError] = useState<
    | { kind: 'prepare' | 'confirm' | 'inspect' }
    | { kind: 'receipt'; receipt: IssueArchiveReceipt }
    | null
  >(null);
  const [requestId, setRequestId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [finished, setFinished] = useState(false);
  useEffect(() => {
    let current = true;
    setPrepared(null);
    setError(null);
    issueArchiveApi
      .prepare(projectId, issueId, archiveWorkspaces)
      .then((value) => {
        if (current) setPrepared(value);
      })
      .catch(() => {
        if (current) setError({ kind: 'prepare' });
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
    const message = receiptMessage(receipt, t);
    if (receipt.status === 'succeeded') {
      if (message) {
        setFinished(true);
        setError({ kind: 'receipt', receipt });
      } else {
        modal.resolve(true);
        modal.hide();
        modal.remove();
      }
    } else {
      setError({ kind: 'receipt', receipt });
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
    } catch {
      setError({ kind: 'confirm' });
    } finally {
      setBusy(false);
    }
  };
  const inspect = async () => {
    if (!requestId) return;
    setBusy(true);
    try {
      accept(await issueArchiveApi.receipt(requestId));
    } catch {
      setError({ kind: 'inspect' });
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
          <DialogTitle id="issue-archive-title">
            {t('issueArchive.title')}
          </DialogTitle>
          <DialogDescription>{t('issueArchive.description')}</DialogDescription>
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
          {t('issueArchive.includeWorkspaces')}
        </label>
        <p>
          {archiveWorkspaces
            ? t('issueArchive.workspaceWarning')
            : t('issueArchive.issueOnly')}
        </p>
        {prepared && (
          <ul className="max-h-48 overflow-auto text-sm">
            {prepared.workspaces.map((ws) => (
              <li key={ws.workspace_id} className="break-all">
                {ws.workspace_id} / {ws.branch} /{' '}
                {ws.path ?? t('issueArchive.noWorktree')}
                {ws.archived
                  ? t('issueArchive.alreadyArchived')
                  : !archiveWorkspaces
                    ? t('issueArchive.unchanged')
                    : ''}
              </li>
            ))}
          </ul>
        )}
        {error && (
          <p
            role="alert"
            className="whitespace-pre-wrap break-words text-error"
          >
            {error.kind === 'receipt'
              ? receiptMessage(error.receipt, t)
              : error.kind === 'prepare'
                ? t('issueArchive.prepareError')
                : error.kind === 'confirm'
                  ? t('issueArchive.confirmUnconfirmed', { requestId })
                  : t('issueArchive.inspectError', { requestId })}
          </p>
        )}
        <DialogFooter>
          <Button variant="outline" onClick={close} disabled={busy}>
            {finished ? t('issueArchive.close') : t('issueArchive.cancel')}
          </Button>
          {requestId && !finished ? (
            <Button disabled={busy} onClick={inspect}>
              {t('issueArchive.inspect')}
            </Button>
          ) : (
            !finished && (
              <Button disabled={!prepared || busy} onClick={confirm}>
                {t('issueArchive.confirm')}
              </Button>
            )
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
});
export const IssueArchiveDialog = defineModal<Props, boolean>(Impl);
