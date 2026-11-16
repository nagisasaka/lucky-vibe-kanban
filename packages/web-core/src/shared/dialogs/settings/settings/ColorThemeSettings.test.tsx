import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it, vi } from 'vitest';
import { DarkColorTheme, LightColorTheme, ThemeMode } from 'shared/types';
import { ColorThemeSettings, colorThemeRows } from './ColorThemeSettings';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
// The picker is a controlled draft editor: rendering it must never reach the
// applied theme, so a draft choice cannot change the app before saving.
vi.mock('@/shared/hooks/useTheme', () => ({
  useTheme: () => {
    throw new Error('ColorThemeSettings must not apply themes');
  },
}));

function render(theme: ThemeMode) {
  return renderToStaticMarkup(
    createElement(ColorThemeSettings, {
      theme,
      lightColorTheme: LightColorTheme.PAPER,
      darkColorTheme: DarkColorTheme.INDIGO,
      onThemeChange: () => undefined,
      onLightColorThemeChange: () => undefined,
      onDarkColorThemeChange: () => undefined,
    })
  );
}

describe('ColorThemeSettings', () => {
  it('shows only the rows that the theme mode can display', () => {
    expect(colorThemeRows(ThemeMode.LIGHT)).toEqual(['light']);
    expect(colorThemeRows(ThemeMode.DARK)).toEqual(['dark']);
    expect(colorThemeRows(ThemeMode.SYSTEM)).toEqual(['light', 'dark']);

    const light = render(ThemeMode.LIGHT);
    expect(light).toContain('settings.general.appearance.colorTheme.light');
    expect(light).not.toContain('data-color-theme="navy"');

    const system = render(ThemeMode.SYSTEM);
    expect(system).toContain(
      'settings.general.appearance.colorTheme.systemLight'
    );
    expect(system).toContain(
      'settings.general.appearance.colorTheme.systemDark'
    );
  });

  it('exposes keyboard-operable radio groups with the saved selection', () => {
    const html = render(ThemeMode.SYSTEM);
    expect(html.match(/role="radiogroup"/g)).toHaveLength(3);
    expect(html.match(/role="radio"/g)).toHaveLength(9);
    // One tab stop per group (roving tabindex) on the selected option.
    expect(html.match(/tabindex="0"/g)).toHaveLength(3);
    expect(html.match(/aria-checked="true"/g)).toHaveLength(3);
  });

  it('scopes previews to the card instead of the document', () => {
    const html = render(ThemeMode.SYSTEM);
    expect(html).toMatch(/class="light [^"]*" data-color-theme="paper"/);
    expect(html).toMatch(/class="dark [^"]*" data-color-theme="navy"/);
    // Standard previews use the side's base tokens without an attribute.
    expect(html).toMatch(/class="light [^"]*"><span/);
  });
});
