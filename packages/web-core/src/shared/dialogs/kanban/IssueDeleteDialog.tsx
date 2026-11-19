import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
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
import { errorDiagnostic } from '@/shared/lib/errorDiagnostic';
import { refreshIssueArchive } from '@/shared/lib/issueArchiveApi';
import {
  completedIssueDelete,
  issueDeleteApi,
  type PreparedIssueDelete,
  type IssueDeleteReceipt,
} from '@/shared/lib/issueDeleteApi';

interface Props {
  projectId: string;
  issueId: string;
  onDeleteIssue: () => unknown;
}
const Impl = create<Props>(({ projectId, issueId, onDeleteIssue }) => {
  const modal = useModal();
  const { t } = useTranslation('tasks');
  const reasonLabel = (reason: string) => {
    switch (reason) {
      case 'external':
        return t('issueDelete.reasons.external');
      case 'ownership':
        return t('issueDelete.reasons.ownership');
      case 'shared':
        return t('issueDelete.reasons.shared');
      case 'busy':
        return t('issueDelete.reasons.busy');
      default:
        return t('issueDelete.reasons.protected');
    }
  };
  const [include, setInclude] = useState(false);
  const [prepared, setPrepared] = useState<PreparedIssueDelete | null>(null);
  const [generation, setGeneration] = useState(0);
  const [requestId, setRequestId] = useState<string | null>(null);
  const [receipt, setReceipt] = useState<IssueDeleteReceipt | null>(null);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const submitting = useRef(false);
  const result =
    receipt && prepared && requestId
      ? completedIssueDelete(receipt, prepared.workspaces, requestId)
      : null;
  const refused =
    receipt?.request_id === requestId && receipt?.status === 'failed';
  const blocked = prepared?.workspaces.some((ws) => !ws.excluded && ws.reason);
  useEffect(() => {
    let current = true;
    if (!include) return;
    issueDeleteApi
      .prepare(projectId, issueId)
      .then((value) => {
        if (current) setPrepared(value);
      })
      .catch((error) => {
        if (current) setError(errorDiagnostic(error));
      });
    return () => {
      current = false;
    };
  }, [include, projectId, issueId, generation]);
  const close = () => {
    if (submitting.current) return;
    modal.resolve(!!result);
    modal.hide();
    modal.remove();
  };
  const accept = (value: IssueDeleteReceipt) => {
    setReceipt(value);
    refreshIssueArchive(projectId);
  };
  const confirm = async () => {
    if (submitting.current || requestId || (include && (!prepared || blocked)))
      return;
    submitting.current = true;
    setBusy(true);
    setError('');
    try {
      if (!include) {
        await onDeleteIssue();
        modal.resolve(true);
        modal.hide();
        modal.remove();
      } else if (prepared) {
        const id = crypto.randomUUID();
        setRequestId(id);
        accept(
          await issueDeleteApi.confirm(
            projectId,
            issueId,
            prepared.confirmation_token,
            id
          )
        );
      }
    } catch (error) {
      setError(errorDiagnostic(error));
    } finally {
      submitting.current = false;
      setBusy(false);
    }
  };
  const inspect = async () => {
    if (!requestId || submitting.current) return;
    submitting.current = true;
    setBusy(true);
    setError('');
    try {
      accept(await issueDeleteApi.receipt(requestId));
    } catch (error) {
      setError(errorDiagnostic(error));
    } finally {
      submitting.current = false;
      setBusy(false);
    }
  };
  const refresh = () => {
    setPrepared(null);
    setError('');
    setReceipt(null);
    setRequestId(null);
    setGeneration((n) => n + 1);
  };
  return (
    <Dialog
      open={modal.visible}
      onOpenChange={close}
      role="dialog"
      aria-modal="true"
      aria-labelledby="issue-delete-title"
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle id="issue-delete-title">
            {t('issueDelete.title')}
          </DialogTitle>
          <DialogDescription>{t('issueDelete.description')}</DialogDescription>
        </DialogHeader>
        <p className="break-all text-low">{issueId}</p>
        <label className="flex gap-base items-center">
          <input
            type="checkbox"
            checked={include}
            disabled={busy || !!requestId}
            onChange={(e) => {
              setPrepared(null);
              setError('');
              setInclude(e.target.checked);
            }}
          />
          {t('issueDelete.includeWorkspaces')}
        </label>
        <p>{t(include ? 'issueDelete.warning' : 'issueDelete.issueOnly')}</p>
        {include && <p>{t('issueDelete.preserved')}</p>}
        {include && !prepared && !error && (
          <p role="status">{t('issueDelete.loading')}</p>
        )}
        {include && prepared && (
          <>
            <p>
              {t('issueDelete.count', {
                count: prepared.workspaces.filter((ws) => !ws.excluded).length,
              })}
            </p>
            <ul className="max-h-48 overflow-auto text-sm">
              {prepared.workspaces.map((ws) => (
                <li key={ws.workspace_id} className="break-all py-half">
                  {ws.name} / {ws.workspace_id} / {ws.branch} /{' '}
                  {ws.path ?? t('issueDelete.noWorktree')}
                  {ws.reason && <strong> — {reasonLabel(ws.reason)}</strong>}
                </li>
              ))}
            </ul>
            {blocked && (
              <p role="alert" className="text-error">
                {t('issueDelete.blocked')}
              </p>
            )}
          </>
        )}
        {result ? (
          <div role="status">
            <p>
              {t(
                result.workspaces.every((ws) => ws.cleanup_completed)
                  ? 'issueDelete.completed'
                  : 'issueDelete.cleanupFailed'
              )}
            </p>
            <ul>
              {result.workspaces.map((ws) => (
                <li key={ws.workspace_id} className="break-all">
                  {ws.workspace_id}:{' '}
                  {t(
                    ws.cleanup_completed
                      ? 'issueDelete.cleanupCompleted'
                      : 'issueDelete.cleanupIncomplete'
                  )}
                  {ws.cleanup_errors.map((detail, i) => (
                    <p key={i} className="text-error break-words">
                      {detail}
                    </p>
                  ))}
                </li>
              ))}
            </ul>
          </div>
        ) : (
          requestId && (
            <p role="alert" className="break-all text-error">
              {t(refused ? 'issueDelete.refused' : 'issueDelete.unknown', {
                requestId,
              })}
              {receipt?.result && (
                <span className="block break-words">{receipt.result}</span>
              )}
            </p>
          )
        )}
        {error && (
          <p role="alert" className="break-words text-error">
            {t('issueDelete.error')} {error}
          </p>
        )}
        <DialogFooter>
          <Button variant="outline" disabled={busy} onClick={close}>
            {t(result ? 'issueDelete.close' : 'issueDelete.cancel')}
          </Button>
          {!result && include && (!requestId || refused) && (
            <Button variant="outline" disabled={busy} onClick={refresh}>
              {t('issueDelete.refresh')}
            </Button>
          )}
          {!result &&
            (requestId && !refused ? (
              <Button disabled={busy} onClick={inspect}>
                {t('issueDelete.inspect')}
              </Button>
            ) : (
              !requestId && (
                <Button
                  variant="destructive"
                  disabled={busy || (include && (!prepared || blocked))}
                  onClick={confirm}
                >
                  {t('issueDelete.confirm')}
                </Button>
              )
            ))}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
});
export const IssueDeleteDialog = defineModal<Props, boolean>(Impl);
