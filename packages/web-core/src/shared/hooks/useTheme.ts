import { useContext } from 'react';
import { createHmrContext } from '@/shared/lib/hmrContext';
import { DarkColorTheme, LightColorTheme, ThemeMode } from 'shared/types';
import type { ResolvedTheme } from '@/shared/lib/theme';

export type ColorThemes = {
  light: LightColorTheme;
  dark: DarkColorTheme;
};

export type ThemeProviderState = {
  theme: ThemeMode;
  setTheme: (theme: ThemeMode) => void;
  /** Applied (saved) colour themes; drafts in settings are not applied. */
  colorThemes: ColorThemes;
  setColorThemes: (colorThemes: ColorThemes) => void;
  /** Light/dark currently displayed, following OS changes for System. */
  resolvedTheme: ResolvedTheme;
};

const initialState: ThemeProviderState = {
  theme: ThemeMode.SYSTEM,
  setTheme: () => null,
  colorThemes: {
    light: LightColorTheme.STANDARD,
    dark: DarkColorTheme.STANDARD,
  },
  setColorThemes: () => null,
  resolvedTheme: 'light',
};

export const ThemeProviderContext = createHmrContext<ThemeProviderState>(
  'ThemeProviderContext',
  initialState
);

export const useTheme = () => {
  const context = useContext(ThemeProviderContext);

  if (context === undefined)
    throw new Error('useTheme must be used within a ThemeProvider');

  return context;
};

export function getResolvedTheme(theme: ThemeMode): ResolvedTheme {
  if (theme === ThemeMode.SYSTEM) {
    return window.matchMedia('(prefers-color-scheme: dark)').matches
      ? 'dark'
      : 'light';
  }
  return theme === ThemeMode.DARK ? 'dark' : 'light';
}
