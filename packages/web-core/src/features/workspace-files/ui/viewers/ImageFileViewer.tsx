import { useTranslation } from 'react-i18next';
import { useState } from 'react';
import { Loader2 } from 'lucide-react';
import { WorkspaceFileEmptyState } from '../WorkspaceFileEmptyState';

interface ImageFileViewerProps {
  src: string | null | undefined;
  alt: string;
}

export function ImageFileViewer({ src, alt }: ImageFileViewerProps) {
  const { t } = useTranslation('common');
  const [isLoading, setIsLoading] = useState(true);
  const [hasError, setHasError] = useState(false);

  if (!src) {
    return (
      <WorkspaceFileEmptyState
        title={t('workspaceFiles.image_unavailable')}
        description={t(
          'workspaceFiles.the_backend_did_not_provide_a_raw_image_url'
        )}
      />
    );
  }

  return (
    <div
      className="relative flex h-full min-h-0 items-center justify-center overflow-auto bg-primary p-double"
      style={{
        backgroundImage:
          'linear-gradient(45deg, hsl(var(--bg-secondary)) 25%, transparent 25%), linear-gradient(-45deg, hsl(var(--bg-secondary)) 25%, transparent 25%), linear-gradient(45deg, transparent 75%, hsl(var(--bg-secondary)) 75%), linear-gradient(-45deg, transparent 75%, hsl(var(--bg-secondary)) 75%)',
        backgroundPosition: '0 0, 0 8px, 8px -8px, -8px 0px',
        backgroundSize: '16px 16px',
      }}
    >
      {isLoading && !hasError && (
        <div className="absolute inset-0 flex items-center justify-center bg-primary/80 text-low">
          <Loader2 className="size-6 animate-spin" />
        </div>
      )}
      {hasError ? (
        <WorkspaceFileEmptyState
          title={t('workspaceFiles.image_failed_to_load')}
          description={t(
            'workspaceFiles.open_the_raw_file_if_you_need_to_inspect_it_directly'
          )}
        />
      ) : (
        <img
          src={src}
          alt={alt}
          className="max-h-full max-w-full object-contain shadow-sm"
          onLoad={() => setIsLoading(false)}
          onError={() => {
            setIsLoading(false);
            setHasError(true);
          }}
        />
      )}
    </div>
  );
}
