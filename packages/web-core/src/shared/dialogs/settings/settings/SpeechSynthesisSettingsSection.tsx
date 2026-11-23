import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PrimaryButton } from '@vibe/ui/components/PrimaryButton';
import {
  getSpeechSynthesis,
  getSpeechVoices,
  previewSpeech,
  readSpeechPreferences,
  saveSpeechPreferences,
  speechVoiceId,
  SPEECH_SETTINGS_EVENT,
  SPEECH_SETTINGS_KEY,
} from '@/shared/lib/speechSynthesis';
import { SettingsCard, SettingsField } from './SettingsComponents';

export function SpeechSynthesisSettingsSection() {
  const { t, i18n } = useTranslation('settings');
  const [preferences, setPreferences] = useState(readSpeechPreferences);
  const [voices, setVoices] = useState(getSpeechVoices);
  const [previewing, setPreviewing] = useState(false);
  const [failed, setFailed] = useState(false);
  const synthesis = getSpeechSynthesis();
  useEffect(() => {
    const updateVoices = () => setVoices(getSpeechVoices());
    const updatePreferences = () => setPreferences(readSpeechPreferences());
    const onStorage = (event: StorageEvent) => {
      if (event.key === SPEECH_SETTINGS_KEY || event.key === null)
        updatePreferences();
    };
    synthesis?.addEventListener('voiceschanged', updateVoices);
    window.addEventListener('storage', onStorage);
    window.addEventListener(SPEECH_SETTINGS_EVENT, updatePreferences);
    updateVoices();
    return () => {
      synthesis?.removeEventListener('voiceschanged', updateVoices);
      window.removeEventListener('storage', onStorage);
      window.removeEventListener(SPEECH_SETTINGS_EVENT, updatePreferences);
      // Never cancel global speech when leaving settings.
    };
  }, [synthesis]);
  const missing =
    preferences.voice &&
    !voices.some((voice) => speechVoiceId(voice) === preferences.voice);
  const controlClass =
    'w-full min-w-0 rounded-sm border border-border bg-panel p-base text-normal focus-visible:ring-2 focus-visible:ring-brand';
  return (
    <SettingsCard
      title={t('settings.layout.nav.speech-synthesis')}
      description={t('settings.scope.speech-synthesis')}
    >
      {!synthesis && (
        <p role="status" className="text-low">
          {t('speechSynthesis.unsupported')}
        </p>
      )}
      <SettingsField
        label={t('speechSynthesis.contentMode')}
        description={t('speechSynthesis.contentModeHint')}
      >
        <select
          aria-label={t('speechSynthesis.contentMode')}
          className={controlClass}
          value={preferences.contentMode}
          onChange={(event) =>
            saveSpeechPreferences({
              ...preferences,
              contentMode:
                event.target.value === 'results-and-action'
                  ? 'results-and-action'
                  : 'all',
            })
          }
        >
          <option value="all">{t('speechSynthesis.contentAll')}</option>
          <option value="results-and-action">
            {t('speechSynthesis.contentResults')}
          </option>
        </select>
      </SettingsField>
      <SettingsField label={t('speechSynthesis.voice')}>
        <select
          aria-label={t('speechSynthesis.voice')}
          className={controlClass}
          value={preferences.voice}
          disabled={!synthesis}
          onChange={(event) =>
            saveSpeechPreferences({ ...preferences, voice: event.target.value })
          }
        >
          <option value="">{t('speechSynthesis.defaultVoice')}</option>
          {missing && (
            <option value={preferences.voice}>
              {t('speechSynthesis.unavailableVoice')}
            </option>
          )}
          {voices.map((voice) => (
            <option key={speechVoiceId(voice)} value={speechVoiceId(voice)}>
              {voice.name} ({voice.lang})
            </option>
          ))}
        </select>
        {missing && (
          <p role="status" className="text-low">
            {t('speechSynthesis.fallback')}
          </p>
        )}
      </SettingsField>
      <SettingsField
        label={t('speechSynthesis.rate')}
        description={t('speechSynthesis.rateHint')}
      >
        <select
          aria-label={t('speechSynthesis.rate')}
          className={controlClass}
          value={preferences.rate}
          onChange={(event) =>
            saveSpeechPreferences({
              ...preferences,
              rate: Number(event.target.value),
            })
          }
        >
          {Array.from({ length: 16 }, (_, index) => (index + 5) / 10).map(
            (rate) => (
              <option key={rate} value={rate}>
                {rate.toFixed(1)}×
              </option>
            )
          )}
        </select>
      </SettingsField>
      <p className="text-low">{t('speechSynthesis.previewHint')}</p>
      <PrimaryButton
        disabled={!synthesis || previewing}
        onClick={() => {
          setFailed(false);
          setPreviewing(true);
          if (
            !previewSpeech(
              t('speechSynthesis.sample'),
              i18n.resolvedLanguage ?? i18n.language,
              (success) => {
                setPreviewing(false);
                setFailed(!success);
              }
            )
          ) {
            setPreviewing(false);
            setFailed(true);
          }
        }}
      >
        {t('speechSynthesis.preview')}
      </PrimaryButton>
      {failed && (
        <p role="status" className="text-low">
          {t('speechSynthesis.previewFailed')}
        </p>
      )}
    </SettingsCard>
  );
}
