import type { MainRepo } from './projectUtilitiesApi';

// Preserve the existing default: the first usable root in API order.
// Pending/failed observations must never start a shell at an implicit cwd.
export function mainTerminalRepo(
  repos: readonly MainRepo[],
  unavailable = false
): MainRepo | undefined {
  return unavailable
    ? undefined
    : repos.find((repo) => repo.path.trim() !== '');
}
