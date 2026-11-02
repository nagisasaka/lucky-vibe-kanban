import { useTranslation } from 'react-i18next';
import { FileSearch } from 'lucide-react';
import { Tooltip } from '@vibe/ui/components/Tooltip';
import { cn } from '@/shared/lib/utils';

interface WorkspaceFilePreviewTriggerProps {
  onClick: () => void;
  disabled?: boolean;
  title?: string;
  className?: string;
  iconClassName?: string;
}

export function WorkspaceFilePreviewTrigger({
  onClick,
  disabled = false,
  title,
  className,
  iconClassName,
}: WorkspaceFilePreviewTriggerProps) {
  const { t } = useTranslation('common');
  const resolvedTitle = title ?? t('workspaceFiles.open_preview');
  const button = (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={cn(
        'flex shrink-0 items-center justify-center rounded text-low transition-colors hover:bg-muted hover:text-normal focus:outline-none focus:ring-1 focus:ring-brand disabled:pointer-events-none disabled:opacity-50',
        className ?? 'p-0.5'
      )}
      aria-label={resolvedTitle}
      title={resolvedTitle}
    >
      <FileSearch className={cn('size-icon-xs', iconClassName)} />
    </button>
  );

  if (disabled) return button;

  return (
    <Tooltip content={resolvedTitle} side="bottom">
      {button}
    </Tooltip>
  );
}
