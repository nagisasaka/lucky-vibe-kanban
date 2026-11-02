import { SoundFile } from 'shared/types';

const labels: Record<SoundFile, (t: (key: string) => string) => string> = {
  [SoundFile.ABSTRACT_SOUND1]: (t) =>
    t('common:notificationSounds.ABSTRACT_SOUND1'),
  [SoundFile.ABSTRACT_SOUND2]: (t) =>
    t('common:notificationSounds.ABSTRACT_SOUND2'),
  [SoundFile.ABSTRACT_SOUND3]: (t) =>
    t('common:notificationSounds.ABSTRACT_SOUND3'),
  [SoundFile.ABSTRACT_SOUND4]: (t) =>
    t('common:notificationSounds.ABSTRACT_SOUND4'),
  [SoundFile.COW_MOOING]: (t) => t('common:notificationSounds.COW_MOOING'),
  [SoundFile.FAHHHHH]: (t) => t('common:notificationSounds.FAHHHHH'),
  [SoundFile.PHONE_VIBRATION]: (t) =>
    t('common:notificationSounds.PHONE_VIBRATION'),
  [SoundFile.ROOSTER]: (t) => t('common:notificationSounds.ROOSTER'),
};

// Translate shipped sound names only; future canonical identities remain original.
export function notificationSoundLabel(
  value: string,
  t: (key: string) => string
): string {
  return Object.hasOwn(labels, value) ? labels[value as SoundFile](t) : value;
}
