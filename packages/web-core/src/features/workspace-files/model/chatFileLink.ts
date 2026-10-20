import type { RepoWithTargetBranch } from 'shared/types';
import { resolveWorkspaceFilePreviewTarget } from './useWorkspaceFilePreviewResolver';
import type { WorkspaceFilePreviewTarget } from './types';

/** Translate a chat reference into the existing repo-scoped preview contract.
 * This is navigation only; the files API owns authorization and confinement.
 */
export function resolveChatFileLink({
  href,
  workspaceId,
  containerRef,
  repos,
  sessionId,
}: {
  href: string;
  workspaceId?: string;
  containerRef?: string | null;
  repos: RepoWithTargetBranch[];
  sessionId?: string;
}): WorkspaceFilePreviewTarget | undefined {
  let path = href.trim();
  if (!path || path.startsWith('#') || path.startsWith('//')) return;
  // Strip location before decoding, so encoded # and : remain filename data.
  const location = path.match(
    /(?::([1-9]\d*)(?::\d+)?|#L([1-9]\d*)(?:C\d+)?(?:-L?\d+(?:C\d+)?)?)$/
  );
  const line = location ? Number(location[1] ?? location[2]) : undefined;
  if (location) path = path.slice(0, location.index);
  if (line !== undefined && !Number.isSafeInteger(line)) return;
  if (/[?#]/.test(path)) return;
  try {
    path = decodeURIComponent(path);
  } catch {
    return;
  }
  path = path.replace(/\\/g, '/');
  if (
    Array.from(path).some(
      (char) => char.charCodeAt(0) < 32 || char.charCodeAt(0) === 127
    ) ||
    path.startsWith('//')
  )
    return;
  if (/^[a-z][a-z0-9+.-]*:/i.test(path) && !/^[a-z]:\//i.test(path)) return;
  if (path.split('/').includes('..')) return;

  let repoId: string | undefined;
  if (path.startsWith('/') || /^[a-z]:\//i.test(path)) {
    // Only the current workspace's repo roots, never the host checkout repo.path.
    const root = containerRef?.replace(/\\/g, '/').replace(/\/+$/, '');
    if (!root) return;
    const repo = repos.find((candidate) =>
      path.startsWith(`${root}/${candidate.name}/`)
    );
    if (!repo) return;
    path = path.slice(`${root}/${repo.name}/`.length);
    repoId = repo.id;
  } else {
    path = path.replace(/^(?:\.\/)+/, '');
    const repo = repos.find((candidate) =>
      path.startsWith(`${candidate.name}/`)
    );
    if (repo) {
      path = path.slice(repo.name.length + 1);
      repoId = repo.id;
    }
  }
  const resolution = resolveWorkspaceFilePreviewTarget({
    workspaceId,
    repos,
    path,
    repoId,
    sessionId,
    source: 'chat',
  });
  if (resolution.status !== 'resolved') return;
  return { ...resolution.target, ...(line ? { line } : {}) };
}
