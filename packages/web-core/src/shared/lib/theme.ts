import { DarkColorTheme, LightColorTheme, ThemeMode } from 'shared/types';

export type ResolvedTheme = 'light' | 'dark';

/** Value of `html[data-color-theme]`; standard themes have no attribute. */
export type ColorThemeAttribute = 'paper' | 'linen' | 'navy' | 'indigo';

export const SYSTEM_DARK_QUERY = '(prefers-color-scheme: dark)';

/**
 * Resolves the actual theme (light/dark) based on the theme mode setting.
 * Handles system theme detection properly.
 */
export function getActualTheme(
  themeMode: ThemeMode | undefined
): ResolvedTheme {
  if (!themeMode || themeMode === ThemeMode.LIGHT) {
    return 'light';
  }

  if (themeMode === ThemeMode.SYSTEM) {
    // Check system preference
    return window.matchMedia(SYSTEM_DARK_QUERY).matches ? 'dark' : 'light';
  }

  // ThemeMode.DARK
  return 'dark';
}

const LIGHT_ATTRIBUTES: Record<LightColorTheme, ColorThemeAttribute | null> = {
  [LightColorTheme.STANDARD]: null,
  [LightColorTheme.PAPER]: 'paper',
  [LightColorTheme.LINEN]: 'linen',
};

const DARK_ATTRIBUTES: Record<DarkColorTheme, ColorThemeAttribute | null> = {
  [DarkColorTheme.STANDARD]: null,
  [DarkColorTheme.NAVY]: 'navy',
  [DarkColorTheme.INDIGO]: 'indigo',
};

export function lightColorThemeAttribute(
  theme: LightColorTheme | undefined
): ColorThemeAttribute | null {
  return (theme && LIGHT_ATTRIBUTES[theme]) ?? null;
}

export function darkColorThemeAttribute(
  theme: DarkColorTheme | undefined
): ColorThemeAttribute | null {
  return (theme && DARK_ATTRIBUTES[theme]) ?? null;
}

/** Colour theme attribute for the side that is currently displayed. */
export function colorThemeAttribute(
  resolved: ResolvedTheme,
  lightTheme: LightColorTheme | undefined,
  darkTheme: DarkColorTheme | undefined
): ColorThemeAttribute | null {
  return resolved === 'dark'
    ? darkColorThemeAttribute(darkTheme)
    : lightColorThemeAttribute(lightTheme);
}

type ThemeRoot = {
  classList: Pick<DOMTokenList, 'add' | 'remove'>;
  setAttribute(name: string, value: string): void;
  removeAttribute(name: string): void;
};

/**
 * Applies the resolved appearance and colour theme to the document root.
 * Only the saved (applied) settings should reach this function.
 */
export function applyThemeToRoot(
  root: ThemeRoot,
  resolved: ResolvedTheme,
  attribute: ColorThemeAttribute | null
) {
  root.classList.remove('light', 'dark');
  root.classList.add(resolved);
  if (attribute) {
    root.setAttribute('data-color-theme', attribute);
  } else {
    root.removeAttribute('data-color-theme');
  }
}
