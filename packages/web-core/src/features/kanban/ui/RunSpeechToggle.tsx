import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { SpeakerHighIcon, SpeakerSlashIcon } from '@phosphor-icons/react';
import { makeLocalApiRequest } from '@/shared/lib/localApiTransport';
import {
  createSpeechUtterance,
  getSpeechSynthesis,
  readSpeechPreferences,
} from '@/shared/lib/speechSynthesis';
import { RunSpeechQueue } from '../model/runSpeechQueue';

import {
  startRunObservationPolling,
  type RunObservationPage,
} from '../model/runObservationPolling';

export function RunSpeechToggle({ projectId }: { projectId: string }) {
  const { t } = useTranslation('common');
  const [enabled, setEnabled] = useState(false);
  const [connecting, setConnecting] = useState(false);
  const initialPage = useRef<RunObservationPage | null>(null);
  const connectAbort = useRef<AbortController | null>(null);
  const queue = useRef<RunSpeechQueue>();
  if (!queue.current && getSpeechSynthesis()) {
    queue.current = new RunSpeechQueue({
      shouldSpeak: (kind) =>
        readSpeechPreferences().contentMode === 'all' ||
        kind === 'terminal' ||
        kind === 'waiting',
      speak(text, done) {
        const utterance = createSpeechUtterance(text, 'ja-JP');
        utterance.onend = done;
        utterance.onerror = done;
        window.speechSynthesis.speak(utterance);
      },
      async isActive(run) {
        const response = await makeLocalApiRequest(
          `/api/projects/${projectId}/run-observations`
        );
        if (!response.ok) return false;
        const page: RunObservationPage = await response.json();
        return page.active.includes(run);
      },
      async isPending(run) {
        const response = await makeLocalApiRequest(
          `/api/projects/${projectId}/run-observations`
        );
        if (!response.ok) return false;
        const page: RunObservationPage = await response.json();
        return page.pending.some((q) => q.run_id === run);
      },
      cancel() {
        window.speechSynthesis.cancel();
      },
    });
  }
  useEffect(() => () => connectAbort.current?.abort(), []);
  useEffect(() => {
    const speech = queue.current;
    const initial = initialPage.current;
    if (!enabled || !speech || !initial) return;
    speech.enable();
    const stop = startRunObservationPolling(
      initial,
      async (cursor, signal) => {
        const suffix = cursor === undefined ? '' : `?after=${cursor}`;
        const response = await makeLocalApiRequest(
          `/api/projects/${projectId}/run-observations${suffix}`,
          { signal }
        );
        if (!response.ok) throw new Error('Run observations unavailable');
        return response.json() as Promise<RunObservationPage>;
      },
      (page, baseline) => speech.accept(page.events, page.pending, baseline)
    );
    return () => {
      stop();
      speech.disable();
    };
  }, [enabled, projectId]);
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
      disabled={!queue.current || connecting}
      aria-busy={connecting}
      className="p-half rounded-sm text-low hover:text-normal hover:bg-secondary focus-visible:ring-1 focus-visible:ring-brand disabled:opacity-50"
      onClick={() => {
        if (enabled) {
          // Cancel synchronously, without waiting for React cleanup.
          queue.current?.disable();
          setEnabled(false);
          return;
        }
        if (connectAbort.current) return;
        const abort = new AbortController();
        connectAbort.current = abort;
        setConnecting(true);
        // The switch becomes on only after the baseline is established; every
        // event after that baseline is then fetched using its cursor.
        void makeLocalApiRequest(
          `/api/projects/${projectId}/run-observations`,
          {
            signal: abort.signal,
          }
        )
          .then(async (response) => {
            if (!response.ok) throw new Error('Run observations unavailable');
            const page: RunObservationPage = await response.json();
            if (!abort.signal.aborted) {
              initialPage.current = page;
              setEnabled(true);
            }
          })
          .catch(() => {
            // Leave the switch off if a subscription could not be established.
          })
          .finally(() => {
            if (connectAbort.current === abort) connectAbort.current = null;
            if (!abort.signal.aborted) setConnecting(false);
          });
      }}
    >
      {enabled ? (
        <SpeakerHighIcon className="size-icon-sm" />
      ) : (
        <SpeakerSlashIcon className="size-icon-sm" />
      )}
    </button>
  );
}
