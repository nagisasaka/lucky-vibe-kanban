/** Device/browser preferences, like the device-only mobile text size setting. */
export const SPEECH_SETTINGS_KEY = 'speech-synthesis-preferences';
export const SPEECH_SETTINGS_EVENT = 'speech-synthesis-preferences-changed';
export type SpeechContentMode = 'all' | 'results-and-action';
export type SpeechPreferences = {
  voice: string;
  rate: number;
  contentMode: SpeechContentMode;
};
const defaults: SpeechPreferences = { voice: '', rate: 1, contentMode: 'all' };
let memory = defaults;
let storageWriteFailed = false;

export function readSpeechPreferences(): SpeechPreferences {
  if (storageWriteFailed) return memory;
  try {
    const value = JSON.parse(localStorage.getItem(SPEECH_SETTINGS_KEY) ?? '{}');
    return {
      contentMode:
        value?.contentMode === 'results-and-action'
          ? 'results-and-action'
          : 'all',
      voice: typeof value?.voice === 'string' ? value.voice : '',
      rate:
        typeof value?.rate === 'number' &&
        Number.isFinite(value.rate) &&
        value.rate >= 0.5 &&
        value.rate <= 2
          ? value.rate
          : 1,
    };
  } catch {
    return memory;
  }
}

export function saveSpeechPreferences(value: SpeechPreferences) {
  memory = value;
  try {
    localStorage.setItem(SPEECH_SETTINGS_KEY, JSON.stringify(value));
    storageWriteFailed = false;
  } catch {
    // Keep the choice for this page when browser storage is unavailable.
    storageWriteFailed = true;
  }
  window.dispatchEvent(new Event(SPEECH_SETTINGS_EVENT));
}

export function getSpeechSynthesis(): SpeechSynthesis | undefined {
  return typeof window !== 'undefined' &&
    typeof SpeechSynthesisUtterance !== 'undefined'
    ? window.speechSynthesis
    : undefined;
}

export function getSpeechVoices(): SpeechSynthesisVoice[] {
  return getSpeechSynthesis()?.getVoices() ?? [];
}

export function speechVoiceId(voice: SpeechSynthesisVoice): string {
  return JSON.stringify([voice.voiceURI, voice.name, voice.lang]);
}

/** Read on every utterance so another tab's changes apply without a reload. */
export function createSpeechUtterance(text: string, lang: string) {
  const preferences = readSpeechPreferences();
  const utterance = new SpeechSynthesisUtterance(text);
  const voice = getSpeechVoices().find(
    (candidate) => speechVoiceId(candidate) === preferences.voice
  );
  utterance.lang = voice?.lang ?? lang;
  if (voice) utterance.voice = voice;
  utterance.rate = preferences.rate;
  return utterance;
}

/** Append to native speech without cancelling the board's current utterance. */
export function previewSpeech(
  text: string,
  lang: string,
  done: (success: boolean) => void
) {
  const synthesis = getSpeechSynthesis();
  if (!synthesis) return false;
  const utterance = createSpeechUtterance(text, lang);
  utterance.onend = () => done(true);
  utterance.onerror = () => done(false);
  try {
    synthesis.speak(utterance);
    return true;
  } catch {
    return false;
  }
}
