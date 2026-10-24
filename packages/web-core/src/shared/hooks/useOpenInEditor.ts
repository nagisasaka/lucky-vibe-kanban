import { useCallback } from 'react';
import { EditorSelectionDialog } from '@/shared/dialogs/command-bar/EditorSelectionDialog';

type OpenEditorOptions = { filePath?: string; repoId?: string };

export function useOpenInEditor(workspaceId?: string) {
  return useCallback(
    async (options?: OpenEditorOptions): Promise<void> => {
      if (!workspaceId) return;
      await EditorSelectionDialog.show({
        selectedAttemptId: workspaceId,
        ...options,
      });
    },
    [workspaceId]
  );
}
