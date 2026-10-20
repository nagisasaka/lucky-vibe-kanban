import { useEffect, useMemo, useState } from 'react';
import { cn } from '../lib/cn';

interface ChatElapsedTimeProps {
  startedAt?: string | null;
  endedAt?: string | null;
  active?: boolean;
  className?: string;
}

function parseTime(value?: string | null) {
  if (!value) return null;

  const timestamp = new Date(value).getTime();
  return Number.isNaN(timestamp) ? null : timestamp;
}

function formatElapsedMs(ms: number) {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }

  if (minutes > 0) {
    return `${minutes}m ${seconds}s`;
  }

  return `${seconds}s`;
}

export function ChatElapsedTime({
  startedAt,
  endedAt,
  active = false,
  className,
}: ChatElapsedTimeProps) {
  const [now, setNow] = useState(() => Date.now());
  const startedAtMs = useMemo(() => parseTime(startedAt), [startedAt]);
  const endedAtMs = useMemo(() => parseTime(endedAt), [endedAt]);
  const ticking = active && startedAtMs != null && endedAtMs == null;

  useEffect(() => {
    if (!ticking) return;
    setNow(Date.now());
    const intervalId = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(intervalId);
  }, [ticking, startedAtMs]);

  if (startedAtMs == null) return null;

  // A recorded end always wins, including a temporarily stale active flag.
  // Missing terminal evidence is unknown, never the browser's mount time.
  const effectiveEnd = endedAtMs ?? (ticking ? now : null);

  if (effectiveEnd == null) return null;

  return (
    <span
      className={cn(
        'shrink-0 text-xs tabular-nums text-low',
        ticking && 'text-normal',
        className
      )}
    >
      {formatElapsedMs(effectiveEnd - startedAtMs)}
    </span>
  );
}
