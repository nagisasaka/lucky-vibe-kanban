import { speechText } from './speechText';

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

export function normalizeSpeechLanguage(tag: string): string | undefined {
  try {
    return Intl.getCanonicalLocales(tag.trim().replaceAll('_', '-'))[0];
  } catch {
    return undefined;
  }
}

export function browserSpeechLanguage(): string | undefined {
  if (typeof navigator === 'undefined') return undefined;
  return [...(navigator.languages ?? []), navigator.language]
    .filter((tag): tag is string => typeof tag === 'string')
    .map(normalizeSpeechLanguage)
    .find(Boolean);
}

export function getSpeechVoices(): SpeechSynthesisVoice[] {
  const language = browserSpeechLanguage()?.split('-')[0];
  if (!language) return [];
  // Keep every provider/region for the preferred language, using only API data.
  return (getSpeechSynthesis()?.getVoices() ?? []).filter(
    (voice) => normalizeSpeechLanguage(voice.lang)?.split('-')[0] === language
  );
}

export function resolveSpeechVoice(
  voices: SpeechSynthesisVoice[],
  savedVoice: string
): SpeechSynthesisVoice | undefined {
  return voices.length === 1
    ? voices[0]
    : voices.find((candidate) => speechVoiceId(candidate) === savedVoice);
}

export function speechVoiceId(voice: SpeechSynthesisVoice): string {
  return JSON.stringify([voice.voiceURI, voice.name, voice.lang]);
}

/** Read on every utterance so another tab's changes apply without a reload.
 * Parse the body before adding the plain source label, preserving block Markdown.
 */
export function createSpeechUtterance(text: string, lang: string, prefix = '') {
  const preferences = readSpeechPreferences();
  const utterance = new SpeechSynthesisUtterance(prefix + speechText(text));
  const voice = resolveSpeechVoice(getSpeechVoices(), preferences.voice);
  utterance.lang =
    (voice && normalizeSpeechLanguage(voice.lang)) ??
    browserSpeechLanguage() ??
    lang;
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
