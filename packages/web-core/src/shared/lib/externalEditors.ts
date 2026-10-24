import type { EditorConfig } from 'shared/types';

export function initializeExternalEditors(
  editor: EditorConfig,
  hostname: string
): EditorConfig {
  if (editor.entries !== null && editor.entries !== undefined) return editor;
  return {
    entries: [
      {
        name: 'VS Code',
        uri_template: `vscode://vscode-remote/ssh-remote+${hostname}<lvk_workspace_dir>`,
      },
    ],
  };
}

// Diff streams always prefix paths with the owning repository's name. File
// previews already use repo-relative paths; keep this conversion at the source.
export function editorFilePathFromDiff(path: string): string {
  const slash = path.indexOf('/');
  return slash < 0 ? '' : path.slice(slash + 1);
}
