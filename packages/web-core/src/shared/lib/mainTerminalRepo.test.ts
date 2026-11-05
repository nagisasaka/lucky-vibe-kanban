import { describe, expect, it } from 'vitest';
import { mainTerminalRepo } from './mainTerminalRepo';
const first = {
  id: 'first',
  name: 'First',
  path: '/fixture/first',
  wiki_enabled: false,
};
const second = { ...first, id: 'second', path: '/fixture/second' };

describe('main terminal default root', () => {
  it('waits for repositories and follows the first usable root after loading', () => {
    expect(mainTerminalRepo([])).toBeUndefined();
    expect(mainTerminalRepo([first])).toBe(first);
  });
  it('uses API order without changing the repository list', () => {
    const repos = Object.freeze([second, first]);
    expect(mainTerminalRepo(repos)).toBe(second);
    expect(repos).toEqual([second, first]);
  });
  it('does not use stale roots after a fetch error', () => {
    expect(mainTerminalRepo([first], true)).toBeUndefined();
  });
  it('skips empty roots and preserves actual path characters', () => {
    expect(mainTerminalRepo([{ ...first, path: ' ' }, second])).toBe(second);
    expect(mainTerminalRepo([{ ...first, path: '' }])).toBeUndefined();
    expect(mainTerminalRepo([{ ...first, path: '/fixture/a b' }])?.path).toBe(
      '/fixture/a b'
    );
  });
});
