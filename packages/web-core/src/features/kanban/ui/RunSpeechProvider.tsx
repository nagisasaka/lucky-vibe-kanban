import {
  createContext,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { makeLocalApiRequest } from '@/shared/lib/localApiTransport';
import { RunSpeechQueue } from '../model/runSpeechQueue';
import {
  startRunObservationPolling,
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

// Owned by the app layout above route, loading and mobile/detail remounts.
// Key by the current board project; leaving it destroys this ephemeral session.
export function RunSpeechProvider({
  projectId,
  children,
}: {
  projectId: string;
  children: ReactNode;
}) {
  const [enabled, setEnabled] = useState(false);
  const [connecting, setConnecting] = useState(false);
  const initialPage = useRef<RunObservationPage | null>(null);
  const connectAbort = useRef<AbortController | null>(null);
  const queue = useRef<RunSpeechQueue>();
  if (
    !queue.current &&
    typeof window !== 'undefined' &&
    'speechSynthesis' in window
  ) {
    queue.current = new RunSpeechQueue({
      speak(text, done) {
        const utterance = new SpeechSynthesisUtterance(text);
        utterance.lang = 'ja-JP';
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
  const toggle = () => {
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
    void makeLocalApiRequest(`/api/projects/${projectId}/run-observations`, {
      signal: abort.signal,
    })
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
  };
  return (
    <RunSpeechContext.Provider
      value={{ enabled, connecting, supported: Boolean(queue.current), toggle }}
    >
      {children}
    </RunSpeechContext.Provider>
  );
}
