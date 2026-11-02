import { Actions } from "@/shared/actions";
import type { ActionExecutorContext } from "@/shared/types/actions";
import { useState } from "react";

export function ActionLabelsFixture() {
  const [deletedIds, setDeletedIds] = useState<string[]>([]);
  return (
    <main>
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
