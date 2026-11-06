import { useEffect, useRef, useState } from 'react';
import {
  PET_STATES,
  frameAt,
  lookDirection,
  validateAtlas,
  type PetState,
} from './format';

type Props = {
  src: string;
  name: string;
  state?: PetState;
  size?: number;
  version?: 1 | 2;
  onError?: () => void;
};
export function PetSprite({
  src,
  name,
  state = 'idle',
  size = 64,
  version = 1,
  onError,
}: Props) {
  const button = useRef<HTMLButtonElement>(null);
  const [direction, setDirection] = useState<number | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [reaction, setReaction] = useState<PetState | null>(null);
  const [frame, setFrame] = useState(0);
  const [visible, setVisible] = useState(!document.hidden);
  const [reduced, setReduced] = useState(
    () => matchMedia('(prefers-reduced-motion: reduce)').matches
  );
  const reactionTimer = useRef<ReturnType<typeof setTimeout>>();
  const active = state === 'failed' ? 'failed' : (reaction ?? state);
  const animationRow = PET_STATES.findIndex((s) => s.name === active);
  const following =
    version === 2 &&
    (active === 'idle' || active === 'waiting') &&
    visible &&
    !reduced &&
    loaded;
  const looking = following ? direction : null;
  const row = looking === null ? animationRow : 9 + Math.floor(looking / 8);
  const column =
    looking === null
      ? Math.min(frame, PET_STATES[animationRow].durations.length - 1)
      : looking % 8;
  useEffect(() => {
    const media = matchMedia('(prefers-reduced-motion: reduce)');
    const changed = () => setReduced(media.matches);
    const visibility = () => setVisible(!document.hidden);
    media.addEventListener('change', changed);
    document.addEventListener('visibilitychange', visibility);
    return () => {
      media.removeEventListener('change', changed);
      document.removeEventListener('visibilitychange', visibility);
    };
  }, []);
  useEffect(() => {
    setDirection(null);
    if (!following) return;
    const reset = () => setDirection(null);
    const move = (event: PointerEvent) => {
      if (event.pointerType !== 'mouse') {
        reset();
        return;
      }
      const bounds = button.current?.getBoundingClientRect();
      if (!bounds) return;
      setDirection(
        lookDirection(
          event.clientX - bounds.left - bounds.width / 2,
          event.clientY - bounds.top - bounds.height / 2,
          size * 0.2
        )
      );
    };
    const leave = (event: PointerEvent) => {
      if (!event.relatedTarget) reset();
    };
    window.addEventListener('pointermove', move, { passive: true });
    window.addEventListener('pointerdown', move, { passive: true });
    document.addEventListener('pointerout', leave);
    window.addEventListener('blur', reset);
    window.addEventListener('pointercancel', reset);
    return () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerdown', move);
      document.removeEventListener('pointerout', leave);
      window.removeEventListener('blur', reset);
      window.removeEventListener('pointercancel', reset);
    };
  }, [following, size]);
  useEffect(() => {
    setLoaded(false);
  }, [src, version]);
  useEffect(() => {
    setFrame(0);
    if (!visible || reduced || looking !== null) return;
    const start = performance.now();
    let timer: ReturnType<typeof setTimeout>;
    const tick = () => {
      const current = frameAt(active, performance.now() - start);
      setFrame(current);
      timer = setTimeout(tick, PET_STATES[animationRow].durations[current]);
    };
    tick();
    return () => clearTimeout(timer);
  }, [active, animationRow, src, visible, reduced, looking]);
  useEffect(() => () => clearTimeout(reactionTimer.current), []);
  const react = () => {
    if (state !== 'idle' && state !== 'running') return;
    const next = state === 'running' ? 'review' : 'jumping';
    setReaction(next);
    clearTimeout(reactionTimer.current);
    const duration = PET_STATES.find((s) => s.name === next)!.durations.reduce(
      (a, b) => a + b,
      0
    );
    reactionTimer.current = setTimeout(() => setReaction(null), duration);
  };
  // Real <img> keeps source handling out of CSS; only numeric transforms vary.
  return (
    <button
      ref={button}
      type="button"
      aria-label={name}
      onClick={react}
      className="relative block overflow-hidden rounded-sm focus-visible:ring-2 focus-visible:ring-brand"
      style={{ width: size, height: size }}
      data-pet-state={active}
      data-pet-frame={column}
      data-pet-direction={looking ?? undefined}
      data-pet-version={version}
    >
      <span
        className="relative block mx-auto h-full overflow-hidden"
        style={{ width: (192 * size) / 208 }}
      >
        <img
          src={src}
          alt=""
          draggable={false}
          onError={onError}
          onLoad={(event) => {
            const image = event.currentTarget;
            try {
              validateAtlas(
                image,
                image.naturalWidth,
                image.naturalHeight,
                version
              );
              setLoaded(true);
            } catch {
              setLoaded(false);
              onError?.();
            }
          }}
          className="absolute max-w-none pointer-events-none"
          style={{
            width: (1536 * size) / 208,
            height: (version === 2 ? 11 : 9) * size,
            left: -(column * 192 * size) / 208,
            top: -row * size,
          }}
        />
      </span>
    </button>
  );
}
