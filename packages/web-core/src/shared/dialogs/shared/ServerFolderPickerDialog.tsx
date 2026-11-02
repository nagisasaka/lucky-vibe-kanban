import { useEffect, useState } from 'react';
import { create, useModal } from '@ebay/nice-modal-react';
import { useTranslation } from 'react-i18next';
import type { DirectoryListResponse } from 'shared/types';
import { Button } from '@vibe/ui/components/Button';
import { Input } from '@vibe/ui/components/Input';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@vibe/ui/components/KeyboardDialog';
import { fileSystemApi } from '@/shared/lib/api';
import { defineModal } from '@/shared/lib/modals';
import type { FolderPickerDialogProps } from './FolderPickerDialog';

const ServerFolderPickerDialogImpl = create<FolderPickerDialogProps>(
  ({ value = '', hostId }) => {
    const modal = useModal();
    const { t } = useTranslation('common');
    const [path, setPath] = useState(value);
    const [requestedPath, setRequestedPath] = useState({ path: value });
    const [listing, setListing] = useState<DirectoryListResponse | null>(null);
    const [error, setError] = useState(false);
    const [loading, setLoading] = useState(true);

    useEffect(() => {
      if (!modal.visible) return;
      let active = true;
      setLoading(true);
      setListing(null);
      setError(false);
      void fileSystemApi
        .list(requestedPath.path.trim() || undefined, hostId)
        .then((result) => {
          if (!active) return;
          setListing(result);
          setPath(result.current_path);
        })
        .catch(() => {
          if (active) setError(true);
        })
        .finally(() => {
          if (active) setLoading(false);
        });
      return () => {
        active = false;
      };
    }, [requestedPath, hostId, modal.visible]);

    const finish = (result: string | null) => {
      modal.resolve(result);
      modal.hide();
      modal.remove();
    };

    return (
      <Dialog
        open={modal.visible}
        onOpenChange={(open) => {
          if (!open) finish(null);
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t('serverFolderPicker.title')}</DialogTitle>
            <DialogDescription>
              {t('serverFolderPicker.description')}
            </DialogDescription>
          </DialogHeader>
          <form
            className="flex gap-half"
            onSubmit={(event) => {
              event.preventDefault();
              setRequestedPath({ path });
            }}
          >
            <Input
              disabled={loading}
              aria-label={t('serverFolderPicker.path')}
              value={path}
              onChange={(event) => setPath(event.target.value)}
            />
            <Button type="submit" variant="outline" disabled={loading}>
              {t('serverFolderPicker.load')}
            </Button>
          </form>
          {error && (
            <p role="alert" className="text-error">
              {t('serverFolderPicker.loadError')}
            </p>
          )}
          <div
            className="flex max-h-64 flex-col gap-half overflow-auto"
            aria-busy={loading}
          >
            {listing?.entries
              .filter((entry) => entry.is_directory)
              .map((entry) => (
                <Button
                  key={entry.path}
                  variant="outline"
                  className="justify-start"
                  onClick={() => setRequestedPath({ path: entry.path })}
                >
                  {entry.name}
                </Button>
              ))}
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => finish(null)}>
              {t('buttons.cancel')}
            </Button>
            <Button
              disabled={loading || !listing || path !== listing.current_path}
              onClick={() => finish(listing?.current_path ?? null)}
            >
              {t('serverFolderPicker.select')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  }
);

export const ServerFolderPickerDialog = defineModal<
  FolderPickerDialogProps,
  string | null
>(ServerFolderPickerDialogImpl);
