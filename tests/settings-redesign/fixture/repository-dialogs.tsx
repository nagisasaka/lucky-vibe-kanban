import { CreatePRDialog } from "@/shared/dialogs/command-bar/CreatePRDialog";
import { ChangeTargetDialog } from "@vibe/ui/components/ChangeTargetDialog";
import { useState } from "react";

export function RepositoryDialogsFixture() {
  const [events, setEvents] = useState<unknown[]>([]);
  const record = (value: unknown) => setEvents((old) => [...old, value]);
  return (
    <main>
      <button
        onClick={() =>
          void ChangeTargetDialog.show({
            branches: [
              { name: "main", isCurrent: false },
              { name: "feature/raw-target-原文", isCurrent: true },
            ],
            onChangeTargetBranch: async (branch) => {
              record({ kind: "branch", branch });
              throw Error("Raw target secret /internal/path");
            },
          })
        }
      >
        Change fixture target
      </button>
      <button
        onClick={() =>
          void CreatePRDialog.show({
            attempt: { id: "raw-pr-workspace" },
            repoId: "raw-repo",
            targetBranch: "feature/raw-target-原文",
          }).then((result) => record({ kind: "pr-result", ...result }))
        }
      >
        Create fixture PR
      </button>
      <output data-testid="repo-events">{JSON.stringify(events)}</output>
    </main>
  );
}
