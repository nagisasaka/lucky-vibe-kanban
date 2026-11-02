import { useTranslation } from 'react-i18next';
import { Button } from '@vibe/ui/components/Button';
import { IdeIcon } from '@/shared/components/IdeIcon';

type OpenInIdeButtonProps = {
  onClick: () => void;
  disabled?: boolean;
  className?: string;
};

export function OpenInIdeButton({
  onClick,
  disabled = false,
  className,
}: OpenInIdeButtonProps) {
  const { t } = useTranslation('common');
  const label = t('workspaces.openExternalEditor');

  return (
    <Button
      variant="ghost"
      size="sm"
      className={`h-10 w-10 p-0 hover:opacity-70 transition-opacity ${className ?? ''}`}
      onClick={onClick}
      disabled={disabled}
      aria-label={label}
      title={label}
    >
      <IdeIcon className="h-4 w-4" />
      <span className="sr-only">{label}</span>
    </Button>
  );
}
