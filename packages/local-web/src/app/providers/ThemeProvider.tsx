import React, { useEffect, useMemo, useState } from 'react';
import { DarkColorTheme, LightColorTheme, ThemeMode } from 'shared/types';
import {
  type ColorThemes,
  ThemeProviderContext,
} from '@/shared/hooks/useTheme';
import {
  type ResolvedTheme,
  SYSTEM_DARK_QUERY,
  applyThemeToRoot,
  colorThemeAttribute,
} from '@/shared/lib/theme';

type ThemeProviderProps = {
  children: React.ReactNode;
  initialTheme?: ThemeMode;
  initialLightColorTheme?: LightColorTheme;
  initialDarkColorTheme?: DarkColorTheme;
};

function systemTheme(): ResolvedTheme {
  return window.matchMedia(SYSTEM_DARK_QUERY).matches ? 'dark' : 'light';
}

export function ThemeProvider({
  children,
  initialTheme = ThemeMode.SYSTEM,
  initialLightColorTheme = LightColorTheme.STANDARD,
  initialDarkColorTheme = DarkColorTheme.STANDARD,
  ...props
}: ThemeProviderProps) {
  const [theme, setThemeState] = useState<ThemeMode>(initialTheme);
  const [colorThemes, setColorThemes] = useState<ColorThemes>({
    light: initialLightColorTheme,
    dark: initialDarkColorTheme,
  });
  const [osTheme, setOsTheme] = useState<ResolvedTheme>(systemTheme);

  // Update theme when the saved config changes
  useEffect(() => {
    setThemeState(initialTheme);
  }, [initialTheme]);

  useEffect(() => {
    setColorThemes({
      light: initialLightColorTheme,
      dark: initialDarkColorTheme,
    });
  }, [initialLightColorTheme, initialDarkColorTheme]);

  // Follow OS light/dark changes so System mode switches without a reload.
  useEffect(() => {
    const query = window.matchMedia(SYSTEM_DARK_QUERY);
    const update = () => setOsTheme(query.matches ? 'dark' : 'light');
    update();
    query.addEventListener('change', update);
    return () => query.removeEventListener('change', update);
  }, []);

  const resolvedTheme: ResolvedTheme =
    theme === ThemeMode.SYSTEM
      ? osTheme
      : theme === ThemeMode.DARK
        ? 'dark'
        : 'light';

  useEffect(() => {
    applyThemeToRoot(
      window.document.documentElement,
      resolvedTheme,
      colorThemeAttribute(resolvedTheme, colorThemes.light, colorThemes.dark)
    );
  }, [resolvedTheme, colorThemes]);

  const value = useMemo(
    () => ({
      theme,
      setTheme: setThemeState,
      colorThemes,
      setColorThemes,
      resolvedTheme,
    }),
    [theme, colorThemes, resolvedTheme]
  );

  return (
    <ThemeProviderContext.Provider {...props} value={value}>
      {children}
    </ThemeProviderContext.Provider>
  );
}
