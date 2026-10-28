import { useRef, type HTMLAttributes } from 'react';

// Match TouchSensor's movement tolerance. Native click slop is larger, so a
// canceled long-press candidate can still produce a click (even after moving
// away and returning). Capture also protects controls nested inside the card.
const TOUCH_TOLERANCE = 5;

export function useTouchClickGuard(): HTMLAttributes<HTMLDivElement> {
  const origin = useRef<{ id: number; x: number; y: number } | null>(null);
  const blocked = useRef(false);
  const track: NonNullable<
    HTMLAttributes<HTMLDivElement>['onPointerMoveCapture']
  > = (event) => {
    const start = origin.current;
    if (
      start &&
      start.id === event.pointerId &&
      Math.hypot(event.clientX - start.x, event.clientY - start.y) >
        TOUCH_TOLERANCE
    ) {
      blocked.current = true;
    }
  };
  return {
    onPointerDownCapture(event) {
      if (event.pointerType !== 'touch') {
        blocked.current = false;
        return;
      }
      if (origin.current) {
        blocked.current = true; // Multi-touch is not a tap.
        return;
      }
      blocked.current = false;
      origin.current = {
        id: event.pointerId,
        x: event.clientX,
        y: event.clientY,
      };
    },
    // Pointer moves arrive even within the browser's touchmove scroll slop.
    onPointerMoveCapture: track,
    onPointerUpCapture(event) {
      track(event);
      if (origin.current?.id === event.pointerId) origin.current = null;
    },
    onPointerCancelCapture() {
      origin.current = null;
      blocked.current = true;
    },
    onClickCapture(event) {
      // Keyboard activation has detail=0; a real mouse pointerdown resets us.
      if (blocked.current && event.detail !== 0) {
        event.preventDefault();
        event.stopPropagation();
      }
    },
  };
}
