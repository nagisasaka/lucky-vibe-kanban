import { useEffect, useState } from 'react';
import type { PetState } from './format';

export const PET_WAIT_MS = 180_000;
// Use only confirmed HTTP failure and fresh session observations supplied by
// EnvironmentPanel. Hidden/stale/missing observations are not failures.
export function usePetBehavior(
  base: 'idle' | 'running' | 'failed',
  enabled: boolean
): PetState {
  const [rest, setRest] = useState<PetState>('idle');
  useEffect(() => {
    if (!enabled || base !== 'idle') return;
    let x = window.screenX;
    let y = window.screenY;
    let stopped: ReturnType<typeof setTimeout>;
    let waiting: ReturnType<typeof setTimeout>;
    const reset = () => {
      setRest('idle');
      clearTimeout(waiting);
      waiting = setTimeout(() => setRest('waiting'), PET_WAIT_MS);
    };
    const enter = (event: PointerEvent) => {
      if (!event.relatedTarget && event.pointerType === 'mouse') reset();
    };
    const leave = (event: PointerEvent) => {
      if (!event.relatedTarget)
        setRest((current) => (current === 'waiting' ? 'idle' : current));
    };
    reset();
    // Browsers have no window-move event. Poll coordinates only; no network.
    const movement = setInterval(() => {
      const nextX = window.screenX,
        nextY = window.screenY;
      if (nextX === x && nextY === y) return;
      const dx = nextX - x;
      x = nextX;
      y = nextY;
      clearTimeout(waiting);
      clearTimeout(stopped);
      setRest(dx < 0 ? 'running-left' : dx > 0 ? 'running-right' : 'idle');
      stopped = setTimeout(reset, 300);
    }, 150);
    document.addEventListener('pointerover', enter);
    document.addEventListener('pointerout', leave);
    return () => {
      clearInterval(movement);
      clearTimeout(waiting);
      clearTimeout(stopped);
      document.removeEventListener('pointerover', enter);
      document.removeEventListener('pointerout', leave);
    };
  }, [base, enabled]);
  return base === 'idle' ? rest : base;
}
