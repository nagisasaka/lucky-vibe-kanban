import { resourceTone, resourceView } from '../lib/resourceObservation';
import { Tooltip } from './Tooltip';

function percentage(value: number | null) {
  // Floor avoids displaying 70/80 in the preceding colour band.
  return value === null ? '—' : `${Math.floor(value)}%`;
}
function space(value: number | null) {
  return value === null ? '—' : `${Number((value / 1e9).toFixed(2))}GB`;
}

export function ResourceSummary({
  data,
  unavailable,
  now,
}: {
  data: unknown;
  unavailable: boolean;
  now: number;
}) {
  const view = resourceView(data, unavailable, now);
  const scope = `System-visible; physical host unknown. ${view.stale ? 'Unavailable or stale. ' : ''}Observed: ${view.timestamp}.`;
  return (
    <div
      className="flex max-w-full shrink-0 flex-wrap items-center gap-base text-xs"
      aria-label="Server resources"
      data-testid="resource-summary"
    >
      <Tooltip
        className="max-w-[min(24rem,calc(100vw-1rem))] whitespace-normal"
        content={`CPU ${percentage(view.cpu)}. ${scope} Sample: ${view.duration === null ? 'unknown' : `${Math.round(view.duration)} ms`}.`}
      >
        <span
          tabIndex={0}
          className={resourceTone(view.cpu)}
          data-testid="resource-cpu"
        >
          CPU {percentage(view.cpu)}
        </span>
      </Tooltip>
      <Tooltip
        className="max-w-[min(24rem,calc(100vw-1rem))] whitespace-normal"
        content={`RAM ${percentage(view.ram)}: (total − available) / total. ${scope}`}
      >
        <span
          tabIndex={0}
          className={resourceTone(view.ram)}
          data-testid="resource-ram"
        >
          RAM {percentage(view.ram)}
        </span>
      </Tooltip>
      <Tooltip
        className="max-w-[min(24rem,calc(100vw-1rem))] whitespace-normal"
        content={`${view.stale ? 'Unavailable or stale. ' : ''}Worktree filesystem: used / total capacity. 1 GB = 1,000,000,000 bytes. Observed: ${view.timestamp}.`}
      >
        <span
          tabIndex={0}
          className={resourceTone(view.disk.percent)}
          data-testid="resource-disk"
        >
          DISK {percentage(view.disk.percent)} ({space(view.disk.used)} /{' '}
          {space(view.disk.total)})
        </span>
      </Tooltip>
    </div>
  );
}
