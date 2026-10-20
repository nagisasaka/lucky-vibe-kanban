import { describe, expect, it } from 'vitest';
import type { RepoWithTargetBranch } from 'shared/types';
import { resolveChatFileLink } from './chatFileLink';

const repo = {
  id: 'repo',
  name: 'project',
  display_name: 'Project',
} as RepoWithTargetBranch;
const resolve = (
  href: string,
  repos = [repo],
  containerRef = '/work/current'
) =>
  resolveChatFileLink({
    href,
    repos,
    containerRef,
    workspaceId: 'workspace',
    sessionId: 'session',
  });

describe('chat file references', () => {
  it.each([
    'src/a.ts',
    './src/a.ts',
    'project/src/a.ts',
    '/work/current/project/src/a.ts',
  ])('resolves %s through the existing preview contract', (href) => {
    expect(resolve(href)).toEqual({
      workspaceId: 'workspace',
      repoId: 'repo',
      path: 'src/a.ts',
      source: 'chat',
      sessionId: 'session',
    });
  });
  it.each(['src/a.ts:12', 'src/a.ts:12:3', 'src/a.ts#L12', 'src/a.ts#L12-L20'])(
    'separates line location %s',
    (href) => {
      expect(resolve(href)).toMatchObject({ path: 'src/a.ts', line: 12 });
    }
  );
  it('decodes filenames once and preserves encoded delimiters', () => {
    expect(resolve('src/a%20b%23c%3Ad.ts:4')).toMatchObject({
      path: 'src/a b#c:d.ts',
      line: 4,
    });
    expect(resolve('src/%252e%252e/a.ts')).toMatchObject({
      path: 'src/%2e%2e/a.ts',
    });
    expect(resolve('src/日本語.ts')).toMatchObject({ path: 'src/日本語.ts' });
  });
  it('maps Windows workspace roots without enabling arbitrary drives', () => {
    expect(
      resolve('C:\\work\\project\\src\\a.ts:2', [repo], 'C:\\work')
    ).toMatchObject({ path: 'src/a.ts', line: 2 });
  });
  it.each([
    '',
    '#section',
    '//host/file',
    '\\\\host\\file',
    'https://example.com/a',
    'http://example.com/a',
    'javascript:alert(1)',
    'data:text/plain,x',
    'file:///etc/passwd',
    '/etc/passwd',
    '/work/current-other/project/a',
    '/work/current/project/../secret',
    '../secret',
    '%2e%2e/secret',
    'src/%2f../../secret',
    'src/%00file',
    'src/%zz',
    'src/a?query',
    'src/a:9999999999999999999999',
  ])('does not enable %s', (href) => expect(resolve(href)).toBeUndefined());
  it('does not guess a repo for ambiguous relative paths', () => {
    const other = { ...repo, id: 'other', name: 'other' };
    expect(resolve('a.ts', [repo, other])).toBeUndefined();
    expect(resolve('other/a.ts', [repo, other])).toMatchObject({
      repoId: 'other',
      path: 'a.ts',
    });
  });
  it('defers missing files to the existing API and refuses missing workspace context', () => {
    expect(resolve('missing.txt')).toMatchObject({ path: 'missing.txt' });
    expect(
      resolveChatFileLink({ href: 'a.ts', repos: [repo] })
    ).toBeUndefined();
  });
});
