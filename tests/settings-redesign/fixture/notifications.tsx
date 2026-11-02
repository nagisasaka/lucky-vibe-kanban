import { UserAvatar } from "@vibe/ui/components/UserAvatar";
import { useSyncExternalStore } from "react";
import { NotificationsPage } from "@/pages/workspaces/NotificationsPage";
import { AppBarNotificationBellContainer } from "@/pages/workspaces/AppBarNotificationBellContainer";
import { groupNotifications } from "@/shared/lib/notifications";
import type { Notification } from "shared/remote-types";
const params = new URLSearchParams(location.search);
const base = {
  organization_id: "raw-org",
  user_id: "raw-user",
  issue_id: "raw-issue",
  comment_id: null,
  seen: false,
  dismissed_at: null,
};
const rows: Notification[] = [
  {
    ...base,
    id: "n1",
    created_at: "2026-11-01T11:59:59Z",
    notification_type: "issue_comment_added",
    payload: {
      actor_user_id: "raw-actor",
      issue_simple_id: "RAW-12",
      deeplink_path: "/raw/issue-12",
    },
  },
  {
    ...base,
    id: "n2",
    created_at: "2026-11-01T12:00:00Z",
    notification_type: "issue_comment_added",
    payload: {
      actor_user_id: "raw-actor",
      issue_simple_id: "RAW-12",
      deeplink_path: "/raw/issue-12",
    },
  },
  {
    ...base,
    id: "n3",
    issue_id: "raw-other",
    created_at: "2026-11-01T12:00:00Z",
    notification_type: "issue_priority_changed",
    payload: {
      issue_simple_id: "RAW-99",
      old_priority: "low",
      new_priority: "high",
      deeplink_path: "/raw/issue-99",
    },
  },
];
type Patch = { id: string; changes: { seen: boolean } };
let snapshot = {
  data: params.has("notifications-error") ? [] : rows,
  writes: [] as Patch[][],
  navigations: [] as unknown[],
  retries: 0,
};
const listeners = new Set<() => void>();
const emit = () => listeners.forEach((listener) => listener());
const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => listeners.delete(listener);
};
const read = () => snapshot;
export function useNotifications() {
  const state = useSyncExternalStore(subscribe, read);
  const groupedNotifications = groupNotifications(state.data);
  return {
    data: state.data,
    groupedNotifications,
    unseenCount: groupedNotifications.filter((g) => !g.seen).length,
    enabled: !params.has("notifications-disabled"),
    isLoading: false,
    error: params.has("notifications-error")
      ? { message: "Raw internal secret" }
      : null,
    retry: () => {
      snapshot = { ...snapshot, retries: snapshot.retries + 1 };
      emit();
    },
    updateMany: (patches: Patch[]) => {
      snapshot = {
        ...snapshot,
        writes: [...snapshot.writes, patches],
        data: params.has("notifications-mark-error")
          ? snapshot.data
          : snapshot.data.map((row) => {
              const patch = patches.find((p) => p.id === row.id);
              return patch ? { ...row, ...patch.changes } : row;
            }),
      };
      emit();
      return {
        persisted: params.has("notifications-mark-error")
          ? Promise.reject(new Error("Raw mutation secret"))
          : Promise.resolve(),
      };
    },
  };
}
const members = new Map([
  [
    "raw-actor",
    {
      user_id: "raw-actor",
      first_name: "Raw Actor 原文",
      last_name: null,
      username: "raw-user",
      avatar_url: null,
    },
  ],
]);
export const useNotificationMembers = () => ({ membersByUserId: members });
const navigate = (value: unknown) => {
  snapshot = { ...snapshot, navigations: [...snapshot.navigations, value] };
  emit();
};
export const useRouter = () => ({ navigate });
export const useNavigate = () => navigate;
export function NotificationsFixture() {
  const state = useSyncExternalStore(subscribe, read);
  return (
    <main className="h-screen bg-primary text-normal">
      <div className="p-base">
        <AppBarNotificationBellContainer />
        <UserAvatar user={{}} />
      </div>
      <div className="h-[70vh]">
        <NotificationsPage />
      </div>
      <output
        data-testid="notification-evidence"
        className="block break-all text-xs"
      >
        {JSON.stringify(state)}
      </output>
    </main>
  );
}
