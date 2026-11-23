import { useTranslation } from 'react-i18next';
import { SpeakerHighIcon, SpeakerSlashIcon } from '@phosphor-icons/react';
import { useRunSpeech } from './RunSpeechProvider';

export function RunSpeechToggle() {
  const { t } = useTranslation('common');
  const { enabled, connecting, supported, toggle } = useRunSpeech();
  const label = connecting
    ? t('kanban.speechConnecting', 'Connecting Run speech')
    : enabled
      ? t('kanban.speechOff', 'Turn off Run speech')
      : t('kanban.speechOn', 'Turn on Run speech');
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      aria-pressed={enabled}
      disabled={!supported || connecting}
      aria-busy={connecting}
      className="p-half rounded-sm text-low hover:text-normal hover:bg-secondary focus-visible:ring-1 focus-visible:ring-brand disabled:opacity-50"
      onClick={toggle}
    >
      {enabled ? (
        <SpeakerHighIcon className="size-icon-sm" />
      ) : (
        <SpeakerSlashIcon className="size-icon-sm" />
      )}
    </button>
  );
}
