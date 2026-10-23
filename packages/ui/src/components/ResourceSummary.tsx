import { resourceTone, resourceView } from '../lib/resourceObservation';
import { Tooltip } from './Tooltip';

function percentage(value: number | null) {
  // Floor avoids displaying 70/80 in the preceding colour band.
  return value === null ? '—' : `${Math.floor(value)}%`;
}
function space(value: number | null) {
  return value === null ? 'unknown' : `${(value / 1024 ** 3).toFixed(1)} GiB`;
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
      className="flex items-center gap-base text-xs shrink-0"
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
        content={`${view.stale ? 'Unavailable or stale. ' : ''}DISK: configured LVK data directory (D) and worktree base directory (W), separately; other workspace destinations are not measured. ${view.disks.map((disk, i) => `${i === 0 ? 'D' : 'W'}: ${percentage(disk.percent)}, ${space(disk.available)} available, filesystem ${disk.filesystem}.`).join(' ')} Usage: used / (used + available). Observed: ${view.timestamp}.`}
      >
        <span tabIndex={0} className="text-low" data-testid="resource-disk">
          DISK{' '}
          <span className={resourceTone(view.disks[0].percent)}>
            D {percentage(view.disks[0].percent)}
          </span>{' '}
          <span className={resourceTone(view.disks[1].percent)}>
            W {percentage(view.disks[1].percent)}
          </span>
        </span>
      </Tooltip>
    </div>
  );
}
