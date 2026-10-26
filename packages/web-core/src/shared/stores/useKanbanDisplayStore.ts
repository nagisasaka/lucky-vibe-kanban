import { create } from 'zustand';
import type {
  IssueArchiveScope,
  ViewNavMode,
} from '@vibe/ui/components/ViewNavTabs';

// Screen state only: survives Issue route remounts, never persisted to Scratch
// or localStorage. A project change (including leaving the project) resets it.
interface State {
  projectId: string | null;
  scope: IssueArchiveScope;
  mode: ViewNavMode;
  enterProject: (projectId: string | null) => void;
  setScope: (scope: IssueArchiveScope) => void;
  toggleMode: () => void;
}
export const useKanbanDisplayStore = create<State>((set, get) => ({
  projectId: null,
  scope: 'active',
  mode: 'kanban',
  enterProject: (projectId) => {
    if (get().projectId !== projectId)
      set({ projectId, scope: 'active', mode: 'kanban' });
  },
  setScope: (scope) => set({ scope }),
  toggleMode: () =>
    set((state) => ({ mode: state.mode === 'kanban' ? 'list' : 'kanban' })),
}));
