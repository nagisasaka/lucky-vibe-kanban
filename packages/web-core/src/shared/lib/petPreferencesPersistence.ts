import type { JsonValue } from 'shared/types';

type Writer = (preferences: JsonValue) => Promise<void>;
let writer: Writer | null = null;

// The existing scratch owner supplies this only after its authoritative snapshot.
export function registerPetPreferencesWriter(next: Writer): () => void {
  writer = next;
  return () => {
    if (writer === next) writer = null;
  };
}
export async function persistPetPreferences(
  preferences: JsonValue
): Promise<void> {
  if (!writer) throw new Error('UI preferences are not connected');
  await writer(preferences);
}
