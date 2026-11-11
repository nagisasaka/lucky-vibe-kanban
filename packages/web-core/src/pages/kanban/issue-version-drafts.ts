export interface VersionDraft {
  name: string;
  description: string;
  revision: number;
  saved: boolean;
}

/** A completed save acknowledges its captured text, never later local edits. */
export function acknowledgeVersionSave(
  latest: VersionDraft,
  captured: VersionDraft,
  revision: number
): VersionDraft {
  return {
    ...latest,
    revision,
    saved:
      latest.name === captured.name &&
      latest.description === captured.description,
  };
}
