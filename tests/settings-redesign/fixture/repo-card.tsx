import { CreatePRDialog } from "@/shared/dialogs/command-bar/CreatePRDialog";
import { useState } from "react";
import { RepoCard, type RepoAction } from "@vibe/ui/components/RepoCard";
import { ChangeTargetDialog } from "@vibe/ui/components/ChangeTargetDialog";
const branches = [
  { name: "main", isCurrent: false },
  { name: "feature/raw-target-原文", isCurrent: true },
];
const params = new URLSearchParams(location.search);
export function RepoCardFixture() {
  const [mode, setMode] = useState("unlinked");
  const [selection, setSelection] = useState<RepoAction>("merge");
  const [events, setEvents] = useState<unknown[]>([]);
  const record = (value: unknown) => setEvents((old) => [...old, value]);
  const change = () => {
    void ChangeTargetDialog.show({
      branches,
      onChangeTargetBranch: async (branch) => {
        record({ kind: "branch", branch });
        if (params.has("branch-error"))
          throw Error("Raw target secret /internal/path");
      },
    });
  };
  return (
    <main className="min-h-screen bg-primary text-normal p-base">
      {params.has("pr") && (
        <button
          onClick={() => {
            void CreatePRDialog.show({
              attempt: { id: "raw-pr-workspace" },
              repoId: "raw-repo",
              targetBranch: "feature/raw-target-原文",
            }).then((result) => record({ kind: "pr-result", ...result }));
          }}
        >
          Create fixture PR
        </button>
      )}
      <div>
        {["unlinked", "open", "linked", "remote", "readonly"].map((value) => (
          <button
            key={value}
            onClick={() => {
              setMode(value);
              setSelection(value === "linked" ? "link-pr" : "merge");
            }}
          >
            {value}
          </button>
        ))}
      </div>
      <section data-testid="repo-card">
        <RepoCard
          repoId="raw-repo"
          name="Raw repository 原文"
          targetBranch="feature/raw-target-原文"
          commitsAhead={1234}
          commitsBehind={2000}
          prNumber={mode === "open" || mode === "linked" ? 12345 : undefined}
          prStatus={
            mode === "open" ? "open" : mode === "linked" ? "merged" : undefined
          }
          isTargetRemote={mode === "remote"}
          readOnly={mode === "readonly"}
          selectedAction={selection}
          onSelectedActionChange={setSelection}
          onActionsClick={(action) => record({ kind: "action", action })}
          onChangeTarget={change}
        />
      </section>
      <output data-testid="repo-events" className="block break-all">
        {JSON.stringify(events)}
      </output>
      <output data-testid="repo-selection">{selection}</output>
    </main>
  );
}
