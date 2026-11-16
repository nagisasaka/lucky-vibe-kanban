import { describe, expect, it } from 'vitest';
import { DarkColorTheme, LightColorTheme } from 'shared/types';
import { applyThemeToRoot, colorThemeAttribute } from './theme';

function fakeRoot() {
  const classes = new Set<string>(['light']);
  const attributes = new Map<string, string>();
  return {
    classes,
    attributes,
    classList: {
      add: (...names: string[]) => names.forEach((name) => classes.add(name)),
      remove: (...names: string[]) =>
        names.forEach((name) => classes.delete(name)),
    },
    setAttribute: (name: string, value: string) => attributes.set(name, value),
    removeAttribute: (name: string) => attributes.delete(name),
  };
}

describe('colour theme resolution', () => {
  it('uses the colour theme of the side that is displayed', () => {
    expect(
      colorThemeAttribute('light', LightColorTheme.LINEN, DarkColorTheme.NAVY)
    ).toBe('linen');
    expect(
      colorThemeAttribute('dark', LightColorTheme.LINEN, DarkColorTheme.NAVY)
    ).toBe('navy');
    expect(
      colorThemeAttribute('dark', LightColorTheme.PAPER, DarkColorTheme.INDIGO)
    ).toBe('indigo');
  });

  it('has no attribute for standard or missing (legacy) settings', () => {
    expect(
      colorThemeAttribute(
        'light',
        LightColorTheme.STANDARD,
        DarkColorTheme.NAVY
      )
    ).toBeNull();
    expect(
      colorThemeAttribute(
        'dark',
        LightColorTheme.PAPER,
        DarkColorTheme.STANDARD
      )
    ).toBeNull();
    expect(colorThemeAttribute('light', undefined, undefined)).toBeNull();
    expect(colorThemeAttribute('dark', undefined, undefined)).toBeNull();
  });

  it('switches the root class and attribute together when the OS flips', () => {
    const root = fakeRoot();
    const apply = (resolved: 'light' | 'dark') =>
      applyThemeToRoot(
        root,
        resolved,
        colorThemeAttribute(
          resolved,
          LightColorTheme.PAPER,
          DarkColorTheme.NAVY
        )
      );

    apply('dark');
    expect([...root.classes]).toEqual(['dark']);
    expect(root.attributes.get('data-color-theme')).toBe('navy');

    apply('light');
    expect([...root.classes]).toEqual(['light']);
    expect(root.attributes.get('data-color-theme')).toBe('paper');

    applyThemeToRoot(root, 'light', null);
    expect(root.attributes.has('data-color-theme')).toBe(false);
  });
});
