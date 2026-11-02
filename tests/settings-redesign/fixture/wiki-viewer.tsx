import { WorkspaceWikiProvider } from "@/features/wiki/ui/WorkspaceWikiProvider";
import {
  WorkspaceWikiArticle,
  WorkspaceWikiNavigation,
} from "@/features/wiki/ui/WorkspaceWikiPanel";
import type { Workspace, RepoWithTargetBranch } from "../../../shared/types";
const workspace = {
  id: "raw-wiki-workspace",
  workspace_kind: "worktree",
  name: "Raw workspace 原文",
  branch: "feature/raw-branch",
} as Workspace;
const repos = [
  { id: "raw-repo-one", display_name: "Raw repo one 原文" },
  { id: "raw-repo-two", display_name: "Raw repo two 原文" },
] as RepoWithTargetBranch[];
export function WikiViewerFixture() {
  return (
    <WorkspaceWikiProvider workspace={workspace} repos={repos}>
      <div className="grid grid-cols-1 md:grid-cols-[256px_minmax(0,1fr)] h-screen text-normal bg-primary">
        <div className="max-h-80 md:max-h-none overflow-auto">
          <WorkspaceWikiNavigation showReload />
        </div>
        <WorkspaceWikiArticle />
      </div>
    </WorkspaceWikiProvider>
  );
}
