import { Actions } from "@/shared/actions";
import type { ActionExecutorContext } from "@/shared/types/actions";
import { ConfirmDialog } from "@vibe/ui/components/ConfirmDialog";
import { actionErrorMessage } from "@/shared/lib/actionMessages";
import i18n from "@/i18n/config";
import { useState } from "react";
import { IssueArchiveDialog } from "@/shared/dialogs/kanban/IssueArchiveDialog";
import type { PreparedIssueArchive } from "@/shared/lib/issueArchiveApi";
import type { IssueArchiveReceipt } from "@/shared/lib/issueArchiveNotice";
const params = new URLSearchParams(location.search);
const calls: unknown[] = [];
const prepared = (archiveWorkspaces: boolean): PreparedIssueArchive => ({
  confirmation_token: archiveWorkspaces ? "token-on" : "token-off",
  archive_workspaces: archiveWorkspaces,
  message: "Raw prepared secret",
  workspaces: [
    {
      workspace_id: "Raw workspace 原文",
      branch: "raw/branch",
      path: "/raw/path 原文",
      archived: false,
      worktree_deleted: false,
    },
    {
      workspace_id: "raw-archived",
      branch: "raw/old",
      path: null,
      archived: true,
      worktree_deleted: true,
    },
  ],
});
export const issueArchiveApi = {
  prepare: async (
    projectId: string,
    issueId: string,
    archiveWorkspaces: boolean,
  ) => {
    calls.push({ kind: "prepare", projectId, issueId, archiveWorkspaces });
    if (params.has("archive-prepare-error")) throw Error("Raw prepare secret");
    return prepared(archiveWorkspaces);
  },
  confirm: async (
    projectId: string,
    issueId: string,
    token: string,
    requestId: string,
  ): Promise<IssueArchiveReceipt> => {
    calls.push({ kind: "confirm", projectId, issueId, token, requestId });
    if (params.has("archive-lost-response"))
      throw Error("Raw confirmation secret");
    return {
      request_id: requestId,
      status: "succeeded",
      result: params.has("archive-warning")
        ? JSON.stringify({
            workspaces: [
              {
                workspace_id: "Raw workspace 原文",
                deletion_error: "Raw deletion secret",
              },
            ],
          })
        : "{}",
    };
  },
  receipt: async (requestId: string): Promise<IssueArchiveReceipt> => {
    calls.push({ kind: "receipt", requestId });
    if (params.has("archive-inspect-error"))
      throw Error("Raw inspection secret");
    return { request_id: requestId, status: "succeeded", result: "{}" };
  },
};
export const refreshIssueArchive = (projectId: string) => {
  calls.push({ kind: "refresh", projectId });
};
Object.defineProperty(window, "__issueArchiveCalls", { value: calls });
export function IssueArchiveFixture() {
  const [result, setResult] = useState<boolean | null>(null);
  const restore = async () => {
    try {
      await Actions.ArchiveIssue.execute(
        {
          projectMutations: { getIssue: () => ({ archived: true }) },
        } as unknown as ActionExecutorContext,
        "raw-project",
        ["raw-issue"],
      );
      setResult(true);
    } catch (error) {
      await ConfirmDialog.show({
        title: () => i18n.t("common:error"),
        message: () => actionErrorMessage(error),
        confirmText: () => i18n.t("common:ok"),
        showCancelButton: false,
      });
    }
  };
  return (
    <main className="min-h-screen bg-primary text-normal p-double">
      <button
        onClick={() => {
          void IssueArchiveDialog.show({
            projectId: "raw-project",
            issueId: "raw-issue",
          }).then(setResult);
        }}
      >
        Open archive fixture
      </button>
      <button onClick={() => void restore()}>Restore fixture</button>
      <output data-testid="archive-result">{JSON.stringify(result)}</output>
    </main>
  );
}
