import { useState } from "react";
import { WorkspaceFileBrowserPanel } from "@/features/workspace-files/ui/WorkspaceFileBrowserPanel";
import { WorkspaceFilePreviewPanel } from "@/features/workspace-files/ui/WorkspaceFilePreviewPanel";
import type { WorkspaceFilePreviewTarget } from "@/features/workspace-files/model/types";
export function FileViewerFixture() {
  const [target, setTarget] = useState<WorkspaceFilePreviewTarget | null>(null);
  return (
    <div className="h-screen grid grid-cols-1 md:grid-cols-[260px_minmax(0,1fr)] bg-primary text-normal">
      <div className="max-h-64 md:max-h-none">
        <WorkspaceFileBrowserPanel
          workspaceId="raw-file-workspace"
          source="file-tree"
          sessionId="raw-session"
          selectedTarget={target}
          onSelectFile={setTarget}
        />
      </div>
      <div data-testid="file-preview" className="min-w-0 min-h-0">
        <WorkspaceFilePreviewPanel
          target={target}
          onClearTarget={() => setTarget(null)}
        />
      </div>
      <output className="sr-only" data-testid="file-target">
        {JSON.stringify(target)}
      </output>
    </div>
  );
}
