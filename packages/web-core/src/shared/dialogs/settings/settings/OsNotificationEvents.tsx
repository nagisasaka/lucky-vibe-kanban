import { useTranslation } from 'react-i18next';
import { SettingsCheckbox } from './SettingsComponents';

export function OsNotificationEvents({
  mask,
  onChange,
}: {
  mask: number;
  onChange: (mask: number) => void;
}) {
  const { t } = useTranslation('settings');
  return (
    <div className="space-y-base">
      <p className="text-sm text-low">{t('notifications.os.scope')}</p>
      <p className="text-sm text-low">{t('notifications.os.languageHint')}</p>
      {[
        {
          bit: 1,
          label: t('notifications.os.question.label'),
          description: t('notifications.os.question.description'),
        },
        {
          bit: 2,
          label: t('notifications.os.approval.label'),
          description: t('notifications.os.approval.description'),
        },
        {
          bit: 4,
          label: t('notifications.os.success.label'),
          description: t('notifications.os.success.description'),
        },
        {
          bit: 8,
          label: t('notifications.os.failure.label'),
          description: t('notifications.os.failure.description'),
        },
      ].map((event) => (
        <SettingsCheckbox
          key={event.bit}
          id={`os-push-event-${event.bit}`}
          label={event.label}
          description={event.description}
          checked={(mask & event.bit) !== 0}
          onChange={(checked) =>
            onChange(checked ? mask | event.bit : mask & ~event.bit)
          }
        />
      ))}
    </div>
  );
}
