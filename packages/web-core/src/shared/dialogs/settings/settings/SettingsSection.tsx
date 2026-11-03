import { Suspense } from 'react';
import { useTranslation } from 'react-i18next';
import { XIcon } from '@phosphor-icons/react';
import {
  renderSettingsSection,
  type SettingsSectionInitialState,
  type SettingsSectionType,
} from './settingsRegistry';

export type { SettingsSectionInitialState, SettingsSectionType };

interface SettingsSectionProps {
  type: SettingsSectionType;
  onClose?: () => void;
  initialState?: SettingsSectionInitialState[SettingsSectionType];
}

export function SettingsSection({
  type,
  onClose,
  initialState,
}: SettingsSectionProps) {
  const { t } = useTranslation('settings');

  return (
    <div className="flex min-w-0 flex-col min-h-full">
      <div className="sticky top-0 z-10 px-6 py-5 border-b border-border bg-primary hidden md:flex items-center justify-between gap-4">
        <h2 className="text-xl font-semibold tracking-tight text-high">
          {t(`settings.layout.nav.${type}`)}
        </h2>
        {onClose && (
          <button
            onClick={onClose}
            className="rounded-sm p-2 opacity-70 ring-offset-panel transition-opacity hover:opacity-100 focus:outline-none focus:ring-2 focus:ring-brand focus:ring-offset-2"
          >
            <XIcon className="h-4 w-4 text-normal" weight="bold" />
            <span className="sr-only">
              {t('buttons.close', { ns: 'common' })}
            </span>
          </button>
        )}
      </div>

      <div className="mx-auto w-full max-w-[960px] space-y-6 px-4 md:px-6 pt-6 pb-6 min-w-0">
        <Suspense
          key={type}
          fallback={<div role="status">{t('settings.general.loading')}</div>}
        >
          {renderSettingsSection(type, initialState)}
        </Suspense>
      </div>
    </div>
  );
}
