import { GearIcon } from '@phosphor-icons/react';
import { SettingsDialog } from '@/shared/dialogs/settings/SettingsDialog';
export function AppBarUserPopoverContainer() {
  return (
    <button
      type="button"
      aria-label="Settings"
      onClick={() => void SettingsDialog.show()}
      className="flex items-center justify-center w-10 h-10 rounded-lg hover:bg-secondary"
    >
      <GearIcon className="size-icon-base" />
    </button>
  );
}
