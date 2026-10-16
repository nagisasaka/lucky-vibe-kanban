export function scopeWorkspaceFileRawUrl(
  rawUrl: string | null | undefined,
  hostId: string | null
): string | null {
  if (hostId) return null;
  return rawUrl ?? null;
}
