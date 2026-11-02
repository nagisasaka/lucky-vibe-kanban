import { SelectionDialog } from "@/shared/dialogs/command-bar/SelectionDialog";
import { buildPrioritySelectionPages } from "@/shared/dialogs/command-bar/selections/prioritySelection";
import { Actions } from "@/shared/actions";
import type { ActionExecutorContext } from "@/shared/types/actions";
import { useState } from "react";

export function ActionLabelsFixture() {
  const [selectionResult, setSelectionResult] = useState("none");
  const [deletedIds, setDeletedIds] = useState<string[]>([]);
  return (
    <main>
      <button
        data-testid="select-priority"
        onClick={async () => {
          const pages = buildPrioritySelectionPages();
          const result = await SelectionDialog.show({
            initialPageId: Object.keys(pages)[0],
            pages,
          });
          setSelectionResult(
            result === undefined ? "cancelled" : JSON.stringify(result),
          );
        }}
      >
        Select priority
      </button>
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
        Delete two issues
      </button>
      <output data-testid="deleted-ids">{JSON.stringify(deletedIds)}</output>
    </main>
  );
}
