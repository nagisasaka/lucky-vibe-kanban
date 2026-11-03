import { useEffect, useRef, useState } from 'react';
import { PET_STATES, frameAt, type PetState } from './format';

type Props = {
  src: string;
  name: string;
  state?: PetState;
  onError?: () => void;
};
export function PetSprite({ src, name, state = 'idle', onError }: Props) {
  const [reaction, setReaction] = useState(false);
  const [frame, setFrame] = useState(0);
  const [visible, setVisible] = useState(!document.hidden);
  const [reduced, setReduced] = useState(
    () => matchMedia('(prefers-reduced-motion: reduce)').matches
  );
  const reactionTimer = useRef<ReturnType<typeof setTimeout>>();
  const active = reaction ? 'waving' : state;
  const row = PET_STATES.findIndex((s) => s.name === active);
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
    setFrame(0);
    if (!visible || reduced) return;
    const start = performance.now();
    let timer: ReturnType<typeof setTimeout>;
    const tick = () => {
      const current = frameAt(active, performance.now() - start);
      setFrame(current);
      timer = setTimeout(tick, PET_STATES[row].durations[current]);
    };
    tick();
    return () => clearTimeout(timer);
  }, [active, row, src, visible, reduced]);
  useEffect(() => () => clearTimeout(reactionTimer.current), []);
  const react = () => {
    setReaction(true);
    clearTimeout(reactionTimer.current);
    reactionTimer.current = setTimeout(() => setReaction(false), 700);
  };
  // Real <img> keeps source handling out of CSS; only numeric transforms vary.
  return (
    <button
      type="button"
      aria-label={name}
      onClick={react}
      className="relative block w-16 h-16 overflow-hidden rounded-sm focus-visible:ring-2 focus-visible:ring-brand"
      data-pet-state={active}
      data-pet-frame={frame}
    >
      <span
        className="relative block mx-auto h-full overflow-hidden"
        style={{ width: (192 * 64) / 208 }}
      >
        <img
          src={src}
          alt=""
          draggable={false}
          onError={onError}
          className="absolute max-w-none pointer-events-none"
          style={{
            width: (1536 * 64) / 208,
            height: 576,
            left: -(frame * 192 * 64) / 208,
            top: -row * 64,
          }}
        />
      </span>
    </button>
  );
}
