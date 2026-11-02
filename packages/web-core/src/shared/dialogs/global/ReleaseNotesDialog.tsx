import { useTranslation } from 'react-i18next';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@vibe/ui/components/KeyboardDialog';
import { Button } from '@vibe/ui/components/Button';
import { AlertCircle, ExternalLink, Loader2 } from 'lucide-react';
import { create, useModal } from '@ebay/nice-modal-react';
import { defineModal, type NoProps } from '@/shared/lib/modals';
import { useReleases } from '@/shared/hooks/useReleases';
import { SimpleMarkdown } from '@/shared/components/SimpleMarkdown';

const GITHUB_RELEASES_URL = 'https://github.com/BloopAI/vibe-kanban/releases';

function formatDate(dateStr: string, locale: string): string {
  try {
    return new Date(dateStr).toLocaleDateString(locale, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    });
  } catch {
    return dateStr;
  }
}

function extractVersion(tagName: string): string {
  return tagName.replace(/-\d{14}$/, '');
}

const ReleaseNotesDialogImpl = create<NoProps>(() => {
  const { t, i18n } = useTranslation('common');
  const modal = useModal();
  const { data: releases, isLoading, isError } = useReleases();

  const handleOpenInBrowser = () => {
    window.open(GITHUB_RELEASES_URL, '_blank');
  };

  return (
    <Dialog
      role="dialog"
      aria-modal="true"
      aria-label={t('releaseNotes.title')}
      open={modal.visible}
      onOpenChange={(open) => {
        if (!open) {
          modal.resolve();
          modal.hide();
        }
      }}
      className="h-[calc(100%-4rem)]"
    >
      <DialogContent className="flex flex-col w-full h-full max-w-2xl max-h-[calc(100dvh-4rem)] p-0">
        <DialogHeader className="px-6 pt-5 pb-4 border-b flex-shrink-0">
          <DialogTitle className="text-lg font-semibold text-high">
            {t('releaseNotes.title')}
          </DialogTitle>
        </DialogHeader>

        <div className="flex-1 overflow-y-auto px-6 py-4 space-y-6 scrollbar-thin">
          {isLoading && (
            <div
              className="flex items-center justify-center py-12"
              role="status"
              aria-label={t('states.loading')}
            >
              <Loader2 className="h-5 w-5 animate-spin text-low" />
            </div>
          )}

          {isError && (
            <div className="flex flex-col items-center justify-center py-12 text-center space-y-3">
              <AlertCircle className="h-8 w-8 text-low" />
              <p className="text-sm text-low">{t('releaseNotes.failed')}</p>
              <Button variant="outline" size="sm" onClick={handleOpenInBrowser}>
                <ExternalLink className="h-3.5 w-3.5 mr-1.5" />
                {t('releaseNotes.viewOnGitHub')}
              </Button>
            </div>
          )}

          {releases?.map((release) => (
            <article key={release.tag_name} className="space-y-1.5">
              <div className="flex items-baseline gap-2">
                <h2 className="text-sm font-semibold text-high">
                  {extractVersion(release.tag_name)}
                </h2>
                <span className="text-xs text-low">
                  {formatDate(
                    release.published_at,
                    i18n.resolvedLanguage || i18n.language
                  )}
                </span>
              </div>
              {release.body && (
                <SimpleMarkdown
                  content={release.body}
                  className="space-y-1.5 pl-0.5"
                />
              )}
            </article>
          ))}
        </div>

        <DialogFooter className="px-6 py-3 border-t flex-shrink-0">
          <Button variant="outline" size="sm" onClick={handleOpenInBrowser}>
            <ExternalLink className="h-3.5 w-3.5 mr-1.5" />
            {t('releaseNotes.openOnGitHub')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
});

export const ReleaseNotesDialog = defineModal<void, void>(
  ReleaseNotesDialogImpl
);
