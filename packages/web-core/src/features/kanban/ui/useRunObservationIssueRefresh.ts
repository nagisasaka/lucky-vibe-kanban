import { useEffect } from 'react';
import { PROJECT_ISSUES_SHAPE } from 'shared/remote-types';
import { refreshShapeFallback } from '@/shared/lib/electric/collections';
import { createIssueRefreshListener } from '../model/runObservationPolling';
import { useRunObservations } from './RunSpeechProvider';

/**
 * While the Project's board is open and visible, observe Run lifecycle events
 * so cards follow Run start/end without waiting for the 30s fallback poll.
 * Independent of speech: the shared observation stays single when speech is on.
 */
export function useRunObservationIssueRefresh(projectId: string) {
  const hub = useRunObservations();
  useEffect(() => {
    if (!hub || hub.projectId !== projectId) return;
    const refresh = () =>
      void refreshShapeFallback(PROJECT_ISSUES_SHAPE, {
        project_id: projectId,
      });
    let unsubscribe: (() => void) | null = null;
    let hidden = false;
    const update = () => {
      const visible = document.visibilityState !== 'hidden';
      if (visible && !unsubscribe) {
        // Returning from a hidden tab may have skipped transitions.
        unsubscribe = hub.subscribe(
          createIssueRefreshListener(refresh, {
            refreshOnFirstBaseline: hidden,
          })
        );
      } else if (!visible) {
        hidden = true;
        unsubscribe?.();
        unsubscribe = null;
      }
    };
    update();
    document.addEventListener('visibilitychange', update);
    return () => {
      document.removeEventListener('visibilitychange', update);
      unsubscribe?.();
    };
  }, [hub, projectId]);
}
