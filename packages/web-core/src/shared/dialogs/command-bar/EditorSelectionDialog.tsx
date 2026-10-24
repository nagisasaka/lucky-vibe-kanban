import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@vibe/ui/components/KeyboardDialog';
import { create, useModal } from '@ebay/nice-modal-react';
import { defineModal } from '@/shared/lib/modals';
import { workspacesApi, repoApi } from '@/shared/lib/api';
import { useUserSystem } from '@/shared/hooks/useUserSystem';

export interface EditorSelectionDialogProps {
  selectedAttemptId?: string;
  repoId?: string;
  filePath?: string;
}

const EditorSelectionDialogImpl = create<EditorSelectionDialogProps>(
  ({ selectedAttemptId, repoId, filePath }) => {
    const modal = useModal();
    const { t } = useTranslation('settings');
    const { config } = useUserSystem();
    const { data, error, isFetching } = useQuery({
      queryKey: [
        'external-editor-links',
        selectedAttemptId,
        repoId,
        filePath,
        config?.editor,
      ],
      queryFn: () =>
        selectedAttemptId
          ? workspacesApi.openEditor(selectedAttemptId, {
              repo_id: repoId ?? null,
              file_path: filePath ?? null,
            })
          : repoApi.openEditor(repoId!, { repo_id: null, file_path: null }),
      enabled: modal.visible && !!(selectedAttemptId || repoId),
      retry: false,
      staleTime: 0,
    });
    const close = () => {
      modal.resolve(null);
      modal.hide();
    };
    return (
      <Dialog open={modal.visible} onOpenChange={(open) => !open && close()}>
        <DialogContent className="sm:max-w-[525px]">
          <DialogHeader>
            <DialogTitle>{t('externalEditors.open')}</DialogTitle>
            <DialogDescription>
              {t('externalEditors.delegate')}
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-base">
            {isFetching && <p role="status">{t('externalEditors.loading')}</p>}
            {error && (
              <p role="alert" className="text-error">
                {error.message}
              </p>
            )}
            {!isFetching && !error && data?.entries.length === 0 && (
              <p>{t('externalEditors.empty')}</p>
            )}
            {!isFetching &&
              !error &&
              data?.entries.map((entry, index) => (
                <div key={index}>
                  {entry.url ? (
                    <a
                      className="block break-words rounded border border-border p-base text-high hover:bg-secondary focus:ring-1 focus:ring-brand"
                      href={entry.url}
                      target="_blank"
                      rel="noopener noreferrer"
                    >
                      {entry.name}
                    </a>
                  ) : (
                    <div className="break-words text-low">
                      <span aria-disabled="true">{entry.name}</span>
                      <p className="text-sm">{entry.unavailable_reason}</p>
                    </div>
                  )}
                </div>
              ))}
          </div>
        </DialogContent>
      </Dialog>
    );
  }
);
export const EditorSelectionDialog = defineModal<
  EditorSelectionDialogProps,
  null
>(EditorSelectionDialogImpl);
