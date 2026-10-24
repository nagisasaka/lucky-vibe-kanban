import { describe, expect, it } from 'vitest';
import {
  initializeExternalEditors,
  editorFilePathFromDiff,
} from './externalEditors';

describe('external editor initialization', () => {
  it('fixes the initial browser hostname once in editable data', () => {
    const initial = initializeExternalEditors({ entries: null }, 'lvk.example');
    expect(initial.entries).toEqual([
      {
        name: 'VS Code',
        uri_template:
          'vscode://vscode-remote/ssh-remote+lvk.example<lvk_workspace_dir>',
      },
    ]);
    expect(initializeExternalEditors(initial, 'different.example')).toBe(
      initial
    );
    const edited = {
      entries: [
        {
          name: 'My IDE',
          uri_template: 'http://ide/?path=<lvk_workspace_dir>',
        },
      ],
    };
    expect(initializeExternalEditors(edited, 'new.example')).toBe(edited);
  });
  it('preserves intentionally empty lists', () => {
    expect(initializeExternalEditors({ entries: [] }, 'lvk.example')).toEqual({
      entries: [],
    });
  });
  it('converts diff paths to the same repo-relative convention as previews', () => {
    expect(editorFilePathFromDiff('repo/src/日本 語.ts')).toBe(
      'src/日本 語.ts'
    );
    expect(editorFilePathFromDiff('repo/repo/file')).toBe('repo/file');
    expect(editorFilePathFromDiff('invalid')).toBe('');
  });
});
