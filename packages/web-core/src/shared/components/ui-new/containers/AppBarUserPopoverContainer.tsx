import { useTranslation } from 'react-i18next';
import { GearIcon } from '@phosphor-icons/react';
import { SettingsDialog } from '@/shared/dialogs/settings/SettingsDialog';
export function AppBarUserPopoverContainer() {
  const { t } = useTranslation('tasks');
  return (
    <button
      type="button"
      aria-label={t('actionLabels.settings')}
      onClick={() => void SettingsDialog.show()}
      className="flex items-center justify-center w-10 h-10 rounded-lg hover:bg-secondary"
    >
      <GearIcon className="size-icon-base" />
    </button>
  );
}
