import { useSyncExternalStore } from "react";
let preferences = JSON.parse(localStorage.getItem("pet-fixture") || "null");
const listeners = new Set<() => void>();
const state = () => ({
  petPreferences: preferences,
  setPetPreferences: (next: unknown) => {
    preferences = next;
    localStorage.setItem("pet-fixture", JSON.stringify(next));
    listeners.forEach((f) => f());
  },
});
export const useUiPreferencesStore = Object.assign(
  (selector: (s: ReturnType<typeof state>) => unknown) => {
    useSyncExternalStore(
      (cb) => {
        listeners.add(cb);
        return () => listeners.delete(cb);
      },
      () => preferences,
    );
    return selector(state());
  },
  { getState: state },
);
