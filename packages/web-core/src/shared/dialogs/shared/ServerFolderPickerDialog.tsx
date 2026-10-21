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
    const [error, setError] = useState<string | null>(null);
    const [loading, setLoading] = useState(true);

    useEffect(() => {
      if (!modal.visible) return;
      let active = true;
      setLoading(true);
      setListing(null);
      setError(null);
      void fileSystemApi
        .list(requestedPath.path.trim() || undefined, hostId)
        .then((result) => {
          if (!active) return;
          setListing(result);
          setPath(result.current_path);
        })
        .catch((cause: unknown) => {
          if (active)
            setError(cause instanceof Error ? cause.message : String(cause));
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
            <DialogTitle>
              {t('serverFolderPicker.title', {
                defaultValue: 'Choose server directory',
              })}
            </DialogTitle>
            <DialogDescription>
              {t('serverFolderPicker.description', {
                defaultValue:
                  'Directories on the LVK server, not on this browser’s device.',
              })}
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
              aria-label={t('serverFolderPicker.path', {
                defaultValue: 'Server directory path',
              })}
              value={path}
              onChange={(event) => setPath(event.target.value)}
            />
            <Button type="submit" variant="outline" disabled={loading}>
              {t('serverFolderPicker.load', { defaultValue: 'Load path' })}
            </Button>
          </form>
          {error && (
            <p role="alert" className="text-error">
              {error}
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
              {t('serverFolderPicker.select', {
                defaultValue: 'Select this directory',
              })}
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
