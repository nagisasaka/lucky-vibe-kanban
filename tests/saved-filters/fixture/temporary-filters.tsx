import React, { useState } from "react";
import { KanbanFiltersDialog } from "@/shared/dialogs/kanban/KanbanFiltersDialog";
import { DEFAULT_KANBAN_FILTER_STATE } from "@/shared/stores/useUiPreferencesStore";

export function TemporaryFilters() {
  const [open, setOpen] = useState(false);
  const [filters, setFilters] = useState(DEFAULT_KANBAN_FILTER_STATE);
  return (
    <main>
      <button onClick={() => setOpen(true)}>Open temporary filters</button>
      <KanbanFiltersDialog
        open={open}
        onOpenChange={setOpen}
        projectId="project"
        currentUserId={null}
        users={[]}
        tags={[{ id: "tag", name: "Test tag", color: "#123456" } as any]}
        filters={filters}
        showSubIssues
        showWorkspaces
        hideBlocked={false}
        onPrioritiesChange={(priorities) =>
          setFilters((f) => ({ ...f, priorities }))
        }
        onTagsChange={(tagIds) => setFilters((f) => ({ ...f, tagIds }))}
        onAssigneesChange={(assigneeIds) =>
          setFilters((f) => ({ ...f, assigneeIds }))
        }
        onSortChange={(sortField, sortDirection) =>
          setFilters((f) => ({ ...f, sortField, sortDirection }))
        }
        onShowSubIssuesChange={() => {}}
        onShowWorkspacesChange={() => {}}
        onHideBlockedChange={() => {}}
      />
    </main>
  );
}
