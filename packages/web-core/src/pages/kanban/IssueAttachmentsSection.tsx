import { useRef, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import { formatFileSize } from '@vibe/ui/lib/formatFileSize';
import { issueAttachmentsApi } from '@/shared/lib/api';
import type { AttachmentResponse } from 'shared/types';

export function IssueAttachmentsSection({
  projectId,
  issueId,
}: {
  projectId: string;
  issueId: string;
}) {
  const { t, i18n } = useTranslation('common');
  const client = useQueryClient();
  const input = useRef<HTMLInputElement>(null);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const queryKey = ['issue-attachments', projectId, issueId];
  const attachments = useQuery({
    queryKey,
    queryFn: () => issueAttachmentsApi.list(projectId, issueId),
  });

  async function upload(file: File) {
    setUploading(true);
    setError(null);
    try {
      await issueAttachmentsApi.upload(projectId, issueId, file);
      await client.invalidateQueries({ queryKey, exact: true });
    } catch {
      setError(t('kanban.attachments.uploadFailed'));
    } finally {
      setUploading(false);
    }
  }

  async function download(attachment: AttachmentResponse) {
    setError(null);
    try {
      const blob = await issueAttachmentsApi.download(
        projectId,
        issueId,
        attachment.id
      );
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement('a');
      anchor.href = url;
      anchor.download = attachment.original_name;
      anchor.click();
      // Allow the browser to consume the download before releasing its URL.
      setTimeout(() => URL.revokeObjectURL(url), 1000);
    } catch {
      setError(t('kanban.attachments.downloadFailed'));
    }
  }

  return (
    <section
      className="p-double space-y-base"
      aria-label={t('kanban.attachments.title')}
    >
      <div className="flex flex-wrap items-center justify-between gap-base">
        <h3 className="text-sm font-medium text-high">
          {t('kanban.attachments.title')}
        </h3>
        <PrimaryButton
          variant="secondary"
          disabled={uploading}
          onClick={() => input.current?.click()}
        >
          {uploading
            ? t('kanban.attachmentUploading')
            : t('kanban.attachments.add')}
        </PrimaryButton>
        <input
          ref={input}
          type="file"
          className="hidden"
          aria-label={t('kanban.attachments.add')}
          disabled={uploading}
          onChange={(event) => {
            const file = event.currentTarget.files?.[0];
            event.currentTarget.value = '';
            if (file) void upload(file);
          }}
        />
      </div>
      <p className="text-xs text-low">{t('kanban.attachFileHint')}</p>
      {attachments.isPending && (
        <p role="status" className="text-low">
          {t('states.loading')}
        </p>
      )}
      {attachments.isError && (
        <div role="alert" className="text-error space-y-base">
          <p>{t('kanban.attachments.loadFailed')}</p>
          <PrimaryButton
            variant="tertiary"
            onClick={() => void attachments.refetch()}
          >
            {t('kanban.attachments.retry')}
          </PrimaryButton>
        </div>
      )}
      {error && (
        <div role="alert" className="text-error space-y-base">
          <p>{error}</p>
          <PrimaryButton
            variant="tertiary"
            onClick={() => {
              setError(null);
              void attachments.refetch();
            }}
          >
            {t('kanban.attachments.retry')}
          </PrimaryButton>
        </div>
      )}
      {attachments.isSuccess && attachments.data.length === 0 && (
        <p className="text-low">{t('kanban.attachments.empty')}</p>
      )}
      {attachments.data && (
        <ul className="space-y-half">
          {attachments.data.map((attachment) => (
            <li
              key={attachment.id}
              className="flex min-w-0 items-center gap-base"
            >
              <button
                type="button"
                className="min-h-cta min-w-0 flex-1 break-all text-left text-brand hover:underline focus-visible:outline focus-visible:outline-2"
                onClick={() => void download(attachment)}
                aria-label={`${t('kanban.downloadAttachment')}: ${attachment.original_name}`}
              >
                {attachment.original_name}
              </button>
              <span className="shrink-0 text-xs text-low">
                {formatFileSize(attachment.size_bytes, i18n.language)}
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
