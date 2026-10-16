import { describe, expect, it } from 'vitest';
import { scopeWorkspaceFileRawUrl } from './workspaceFileRawUrl';

describe('workspace file raw urls', () => {
  it('leaves local raw urls unchanged without a host id', () => {
    expect(
      scopeWorkspaceFileRawUrl(
        '/api/workspaces/workspace-1/files/raw?repo_id=repo-1',
        null
      )
    ).toBe('/api/workspaces/workspace-1/files/raw?repo_id=repo-1');
  });

  it('rejects raw URLs for retired remote hosts', () => {
    expect(
      scopeWorkspaceFileRawUrl(
        '/api/workspaces/workspace-1/files/raw?repo_id=repo-1',
        'host-1'
      )
    ).toBeNull();
  });

  it('rejects already scoped retired URLs', () => {
    expect(
      scopeWorkspaceFileRawUrl(
        '/api/host/host-1/workspaces/workspace-1/files/raw',
        'host-1'
      )
    ).toBeNull();
  });

  it('preserves external URLs', () => {
    expect(scopeWorkspaceFileRawUrl('https://example.com/file.png', null)).toBe(
      'https://example.com/file.png'
    );
  });
});
