export function petStatus(
  visible: boolean,
  httpFailed: boolean,
  fresh: boolean,
  sessions: unknown
): 'idle' | 'running' | 'failed' {
  if (visible && httpFailed) return 'failed';
  return visible &&
    fresh &&
    typeof sessions === 'number' &&
    Number.isSafeInteger(sessions) &&
    sessions > 0
    ? 'running'
    : 'idle';
}
