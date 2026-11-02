import {
  SelectionDialog,
  type SelectionPage,
} from "@/shared/dialogs/command-bar/SelectionDialog";
import { buildStatusSelectionPages } from "@/shared/dialogs/command-bar/selections/statusSelection";
import { buildPrioritySelectionPages } from "@/shared/dialogs/command-bar/selections/prioritySelection";
import { buildRepoSelectionPages } from "@/shared/dialogs/command-bar/selections/repoSelection";
import { buildBranchSelectionPages } from "@/shared/dialogs/command-bar/selections/branchSelection";
import { buildSubIssueSelectionPages } from "@/shared/dialogs/command-bar/selections/subIssueSelection";
import { buildRelationshipSelectionPages } from "@/shared/dialogs/command-bar/selections/relationshipSelection";
import type { Issue } from "../../../shared/remote-types";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useResolvedPage } from "@/shared/dialogs/command-bar/commandBar/useResolvedPage";
import { Actions } from "@/shared/actions";
import {
  getActionLabel,
  type ActionExecutorContext,
  type ActionVisibilityContext,
} from "@/shared/types/actions";
import type { Workspace } from "../../../shared/types";
export function ActionLabelsFixture() {
  const { t } = useTranslation("tasks");
  const [selectionResult, setSelectionResult] = useState("none");
  const [deletedIds, setDeletedIds] = useState<string[]>([]);
  const [search, setSearch] = useState("");
  const workspace = { id: "raw-workspace", pinned: true } as Workspace;
  const context = {
    isLeftMainPanelVisible: false,
    selectedIssueArchived: false,
    workspaceArchived: false,
    workspaceReadOnly: false,
    devServerState: "stopped",
    logsPanelContent: null,
    layoutMode: "workspaces",
    hasWorkspace: true,
    isCreatingIssue: false,
  } as ActionVisibilityContext;
  const searchPage = useResolvedPage("root", search, context, workspace);
  const openSelection = async (mode: string) => {
    const issues = [
      {
        id: "raw-issue",
        simple_id: "RAW-31",
        title: "High: raw user issue",
        status_id: "raw-status",
      },
    ] as Issue[];
    const choices: Record<string, Record<string, SelectionPage>> = {
      status: buildStatusSelectionPages([
        { id: "raw-status", name: "High", color: "100 50% 50%" },
      ]),
      priority: buildPrioritySelectionPages(),
      repo: buildRepoSelectionPages([
        { id: "raw-repo", display_name: "High: raw repository" },
      ]),
      branch: buildBranchSelectionPages(
        [{ name: "feature/High-raw-branch", isCurrent: true }],
        "High: raw repository",
      ),
      child: buildSubIssueSelectionPages(issues, "addChild"),
      parent: buildSubIssueSelectionPages(issues, "setParent"),
      relationship: buildRelationshipSelectionPages(issues),
    };
    const pages = choices[mode];
    const result = await SelectionDialog.show({
      initialPageId: Object.keys(pages)[0],
      pages,
    });
    setSelectionResult(
      result === undefined ? "cancelled" : JSON.stringify(result),
    );
  };
  return (
    <div className="p-base bg-primary text-normal">
      <button
        data-testid="projects-guide"
        onClick={() => void Actions.ProjectsGuide.execute()}
      >
        fixture-projects-guide
      </button>
      {[
        "status",
        "priority",
        "repo",
        "branch",
        "child",
        "parent",
        "relationship",
      ].map((mode) => (
        <button
          key={mode}
          data-testid={`select-${mode}`}
          onClick={() => void openSelection(mode)}
        >{`fixture-select-${mode}`}</button>
      ))}
      <output data-testid="selection-result">{selectionResult}</output>
      <button
        data-testid="delete-two"
        onClick={() =>
          void Actions.DeleteIssue.execute(
            {
              projectMutations: {
                removeIssue: (id: string) =>
                  setDeletedIds((ids) => [...ids, id]),
              },
            } as ActionExecutorContext,
            "raw-project",
            ["raw-issue-1", "raw-issue-2"],
          )
        }
      >
        fixture-delete-two
      </button>
      <output data-testid="deleted-ids">{JSON.stringify(deletedIds)}</output>
      <input
        aria-label="fixture-search"
        value={search}
        onChange={(event) => setSearch(event.target.value)}
      />
      <div data-testid="search-results">
        {searchPage.groups
          .flatMap((group) => group.items)
          .filter((item) => item.type === "action")
          .map((item) =>
            item.type === "action" ? (
              <span key={item.action.id} data-search-action={item.action.id}>
                {getActionLabel(item.action, context, workspace, t)}
              </span>
            ) : null,
          )}
      </div>
    </div>
  );
}
