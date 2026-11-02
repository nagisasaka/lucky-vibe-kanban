import { useState } from "react";
const params = new URLSearchParams(location.search);
const timestamp = "2026-01-02T15:04:05Z";
export function useAuth() {
  return { isSignedIn: true, isLoaded: true };
}
export function useUserOrganizations() {
  return {
    data: {
      organizations: [
        { id: "raw-organization", name: "High: raw organization" },
      ],
    },
  };
}
export function useCurrentUser() {
  return { data: { user_id: "raw-user" } };
}
const writes: unknown[] = [];
Object.defineProperty(window, "__electricMutations", { value: writes });
export function useShape(shape: { table: string }, scope: unknown) {
  const [failed, setFailed] = useState(
    params.get("diagnostic-mode") === "error",
  );
  const row = {
    id: `raw-${shape.table}`,
    name: `High: raw ${shape.table}`,
    key: "RAW",
    color: "#3b82f6",
    created_at: timestamp,
    updated_at: timestamp,
  };
  const record = (operation: string, value: unknown) => {
    writes.push({ table: shape.table, scope, operation, value });
    return { data: row, persisted: Promise.resolve(row) };
  };
  return {
    data: [row],
    isLoading: false,
    error: failed
      ? { status: 503, message: "Raw diagnostic 原文 / internal-test-path" }
      : null,
    retry: () => setFailed(false),
    insert: (value: unknown) => record("insert", value),
    update: (id: string, value: unknown) =>
      record("update", { id, changes: value }),
    remove: (id: string) => record("remove", id),
  };
}
