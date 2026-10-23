import { create } from 'zustand';

type Draft = { id: string; body: string; category: string };
interface TodoViewState {
  projectViews: Record<string, 'kanban' | 'todos' | 'design-notes'>;
  integrationProject: string | null;
  drafts: Record<string, Draft>;
  workspaceProjects: Record<string, string>;
  setProjectView: (
    project: string,
    view: 'kanban' | 'todos' | 'design-notes'
  ) => void;
  setIntegrationProject: (project: string | null) => void;
  setDraft: (project: string, draft: Draft) => void;
  clearDraft: (project: string) => void;
  setWorkspaceProject: (workspace: string, project: string) => void;
}
// View state and unsent notes survive pane changes; persisted TODOs live in the DB.
export const useTodoViewStore = create<TodoViewState>((set) => ({
  projectViews: {},
  integrationProject: null,
  drafts: {},
  workspaceProjects: {},
  setProjectView: (project, view) =>
    set((s) => ({ projectViews: { ...s.projectViews, [project]: view } })),
  setIntegrationProject: (integrationProject) => set({ integrationProject }),
  setDraft: (project, draft) =>
    set((s) => ({ drafts: { ...s.drafts, [project]: draft } })),
  clearDraft: (project) =>
    set((s) => {
      const drafts = { ...s.drafts };
      delete drafts[project];
      return { drafts };
    }),
  setWorkspaceProject: (workspace, project) =>
    set((s) => ({
      workspaceProjects: { ...s.workspaceProjects, [workspace]: project },
    })),
}));
