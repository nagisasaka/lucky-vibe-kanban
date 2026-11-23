import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest';
import {
  createSpeechUtterance,
  getSpeechSynthesis,
  previewSpeech,
  readSpeechPreferences,
  saveSpeechPreferences,
  speechVoiceId,
  SPEECH_SETTINGS_KEY,
} from './speechSynthesis';
import { RunSpeechQueue } from '@/features/kanban/model/runSpeechQueue';

const voice = {
  voiceURI: 'test:ja',
  name: 'Test voice',
  lang: 'ja-JP',
} as SpeechSynthesisVoice;
let voices: SpeechSynthesisVoice[];
let stored: Map<string, string>;
const speak = vi.fn();
const cancel = vi.fn();
beforeEach(() => {
  voices = [];
  stored = new Map();
  vi.stubGlobal('localStorage', {
    getItem: (key: string) => stored.get(key) ?? null,
    setItem: (key: string, value: string) => stored.set(key, value),
  });
  vi.stubGlobal('window', {
    dispatchEvent: vi.fn(),
    speechSynthesis: { getVoices: () => voices, speak, cancel },
  });
  vi.stubGlobal(
    'SpeechSynthesisUtterance',
    class {
      voice = null;
      rate = 1;
      lang = '';
      constructor(public text: string) {}
    }
  );
  saveSpeechPreferences({ voice: '', rate: 1, contentMode: 'all' });
  speak.mockClear();
  cancel.mockClear();
});
afterEach(() => vi.unstubAllGlobals());
describe('browser speech preferences', () => {
  it('defaults to browser voice and 1.0, and reads persisted choices on every utterance', () => {
    expect(readSpeechPreferences()).toEqual({
      voice: '',
      rate: 1,
      contentMode: 'all',
    });
    expect(createSpeechUtterance('test', 'ja-JP').voice).toBeNull();
    voices = [voice];
    saveSpeechPreferences({
      voice: speechVoiceId(voice),
      rate: 1.4,
      contentMode: 'all',
    });
    expect(createSpeechUtterance('test', 'en').voice).toBe(voice);
    expect(createSpeechUtterance('test', 'en').rate).toBe(1.4);
    // Simulate another tab writing storage without a React re-render.
    stored.set(SPEECH_SETTINGS_KEY, JSON.stringify({ voice: '', rate: 0.5 }));
    expect(createSpeechUtterance('next', 'ja-JP').rate).toBe(0.5);
    expect(createSpeechUtterance('next', 'ja-JP').voice).toBeNull();
  });
  it('defaults missing/invalid content mode to all and persists results mode independently of preview', () => {
    for (const contentMode of [undefined, 'unknown', null]) {
      stored.set(
        SPEECH_SETTINGS_KEY,
        JSON.stringify({ voice: 'saved', rate: 1.3, contentMode })
      );
      expect(readSpeechPreferences()).toEqual({
        voice: 'saved',
        rate: 1.3,
        contentMode: 'all',
      });
    }
    saveSpeechPreferences({
      voice: '',
      rate: 1.2,
      contentMode: 'results-and-action',
    });
    expect(JSON.parse(stored.get(SPEECH_SETTINGS_KEY)!).contentMode).toBe(
      'results-and-action'
    );
    expect(readSpeechPreferences().contentMode).toBe('results-and-action');
    expect(previewSpeech('sample', 'en', vi.fn())).toBe(true);
    expect(speak.mock.calls[0][0]).toMatchObject({ text: 'sample', rate: 1.2 });
    stored.set(
      SPEECH_SETTINGS_KEY,
      JSON.stringify({ voice: '', rate: 1, contentMode: 'all' })
    );
    expect(readSpeechPreferences().contentMode).toBe('all');
  });
  it('retains unavailable preference through delayed loading, disappearance and recovery', () => {
    const preference = {
      voice: speechVoiceId(voice),
      rate: 2,
      contentMode: 'all' as const,
    };
    saveSpeechPreferences(preference);
    expect(createSpeechUtterance('test', 'ja-JP').voice).toBeNull();
    voices = [voice];
    expect(createSpeechUtterance('test', 'ja-JP').voice).toBe(voice);
    voices = [];
    expect(createSpeechUtterance('test', 'ja-JP').voice).toBeNull();
    expect(readSpeechPreferences()).toEqual(preference);
    voices = [voice];
    expect(createSpeechUtterance('test', 'ja-JP').voice).toBe(voice);
  });
  it.each([0, 2.1, '1.3', null])(
    'rejects invalid persisted rates: %s',
    (rate) => {
      stored.set(SPEECH_SETTINGS_KEY, JSON.stringify({ rate }));
      expect(readSpeechPreferences().rate).toBe(1);
    }
  );
  it('preview uses choices and never cancels or enables a board queue', () => {
    voices = [voice];
    saveSpeechPreferences({
      voice: speechVoiceId(voice),
      rate: 1.7,
      contentMode: 'all',
    });
    const queue = new RunSpeechQueue({
      speak: (text, done) => {
        const utterance = createSpeechUtterance(text, 'ja-JP');
        utterance.onend = done;
        speak(utterance);
      },
      cancel,
    });
    const event = (run_id: string) => ({
      run_id,
      issue: run_id,
      attempt: 1,
      payload: { type: 'lifecycle_changed', data: { status: 'starting' } },
    });
    queue.accept([event('off')], []);
    expect(speak).not.toHaveBeenCalled();
    queue.enable();
    queue.accept([event('one'), event('two')], []);
    const current = speak.mock.calls[0][0];
    expect(previewSpeech('preview', 'en', vi.fn())).toBe(true);
    expect(speak.mock.calls[1][0]).toMatchObject({
      text: 'preview',
      voice,
      rate: 1.7,
    });
    expect(cancel).not.toHaveBeenCalled();
    current.onend();
    expect(speak).toHaveBeenCalledTimes(3);
    expect(speak.mock.calls[2][0].text).toContain('two');
  });
  it('reports asynchronous preview errors and successful completion', () => {
    const done = vi.fn();
    previewSpeech('sample', 'en', done);
    speak.mock.calls[0][0].onerror();
    expect(done).toHaveBeenLastCalledWith(false);
    previewSpeech('sample', 'en', done);
    speak.mock.calls[1][0].onend();
    expect(done).toHaveBeenLastCalledWith(true);
    expect(cancel).not.toHaveBeenCalled();
  });
  it('retains a page-local choice if storage rejects writes', () => {
    vi.stubGlobal('localStorage', {
      getItem: () => null,
      setItem: () => {
        throw new Error('quota');
      },
    });
    saveSpeechPreferences({ voice: '', rate: 1.6, contentMode: 'all' });
    expect(createSpeechUtterance('sample', 'en').rate).toBe(1.6);
  });
  it('handles unsupported browsers and a rejected preview without cancellation', () => {
    speak.mockImplementationOnce(() => {
      throw new Error('unavailable');
    });
    expect(previewSpeech('sample', 'en', vi.fn())).toBe(false);
    vi.stubGlobal('SpeechSynthesisUtterance', undefined);
    expect(getSpeechSynthesis()).toBeUndefined();
    expect(previewSpeech('sample', 'en', vi.fn())).toBe(false);
    expect(cancel).not.toHaveBeenCalled();
  });
});
