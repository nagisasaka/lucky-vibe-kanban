import {
  createContext,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { makeLocalApiRequest } from '@/shared/lib/localApiTransport';
import {
  createSpeechUtterance,
  getSpeechSynthesis,
  readSpeechPreferences,
} from '@/shared/lib/speechSynthesis';
import { RunSpeechQueue } from '../model/runSpeechQueue';
import {
  RunObservationHub,
  type RunObservationPage,
} from '../model/runObservationPolling';

const RunSpeechContext = createContext<{
  enabled: boolean;
  connecting: boolean;
  supported: boolean;
  toggle: () => void;
} | null>(null);

export function useRunSpeech() {
  const speech = useContext(RunSpeechContext);
  if (!speech) throw new Error('RunSpeechProvider is required');
  return speech;
}

const RunObservationContext = createContext<RunObservationHub | null>(null);

/** The board Project's shared observation, or null outside a board. */
export function useRunObservations() {
  return useContext(RunObservationContext);
}

// Owned by the app layout above route, loading and mobile/detail remounts.
// Key by the current board project; leaving it destroys this ephemeral session.
export function RunSpeechProvider({
  projectId,
  children,
}: {
  projectId: string;
  children: ReactNode;
}) {
  // Speech only listens to the board's observation; it does not own polling.
  const [requested, setRequested] = useState(false);
  const [enabled, setEnabled] = useState(false);
  const [hub] = useState(() =>
    projectId
      ? new RunObservationHub(projectId, async (cursor, signal) => {
          const suffix = cursor === undefined ? '' : `?after=${cursor}`;
          const response = await makeLocalApiRequest(
            `/api/projects/${projectId}/run-observations${suffix}`,
            { signal }
          );
          if (!response.ok) throw new Error('Run observations unavailable');
          return response.json() as Promise<RunObservationPage>;
        })
      : null
  );
  const queue = useRef<RunSpeechQueue>();
  if (!queue.current && getSpeechSynthesis()) {
    queue.current = new RunSpeechQueue({
      shouldSpeak: (kind) =>
        readSpeechPreferences().contentMode === 'all' ||
        kind === 'terminal' ||
        kind === 'waiting',
      speak(text, done, prefix) {
        const utterance = createSpeechUtterance(text, 'ja-JP', prefix);
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
  useEffect(() => {
    const speech = queue.current;
    if (!requested || !speech || !hub) return;
    // The switch becomes on only after a baseline is established; later
    // pages carry only events after that cursor.
    let joined = false;
    const unsubscribe = hub.subscribe({
      page(page, baseline) {
        if (!joined) {
          joined = true;
          speech.enable();
          setEnabled(true);
        }
        speech.accept(page.events, page.pending, baseline);
      },
      // Leave the switch off if a subscription could not be established.
      error: () => setRequested(false),
    });
    return () => {
      unsubscribe();
      if (!joined) return;
      speech.disable();
      setEnabled(false);
    };
  }, [requested, hub]);
  const toggle = () => {
    if (enabled) {
      // Cancel synchronously, without waiting for React cleanup.
      queue.current?.disable();
      setEnabled(false);
      setRequested(false);
      return;
    }
    setRequested(true);
  };
  return (
    <RunSpeechContext.Provider
      value={{
        enabled,
        connecting: requested && !enabled,
        supported: Boolean(queue.current),
        toggle,
      }}
    >
      <RunObservationContext.Provider value={hub}>
        {children}
      </RunObservationContext.Provider>
    </RunSpeechContext.Provider>
  );
}
