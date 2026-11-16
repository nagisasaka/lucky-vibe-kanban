import { useId, useRef, type KeyboardEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { DarkColorTheme, LightColorTheme, ThemeMode } from 'shared/types';
import { cn } from '@/shared/lib/utils';
import {
  type ResolvedTheme,
  darkColorThemeAttribute,
  lightColorThemeAttribute,
} from '@/shared/lib/theme';
import { SettingsField } from './SettingsComponents';

const THEME_MODES = [ThemeMode.SYSTEM, ThemeMode.LIGHT, ThemeMode.DARK];
const LIGHT_THEMES = [
  LightColorTheme.STANDARD,
  LightColorTheme.PAPER,
  LightColorTheme.LINEN,
];
const DARK_THEMES = [
  DarkColorTheme.STANDARD,
  DarkColorTheme.NAVY,
  DarkColorTheme.INDIGO,
];

/** Colour theme rows shown for a theme mode: only the side that can be displayed. */
export function colorThemeRows(theme: ThemeMode): ResolvedTheme[] {
  if (theme === ThemeMode.LIGHT) return ['light'];
  if (theme === ThemeMode.DARK) return ['dark'];
  return ['light', 'dark'];
}

type RadioOption<T extends string> = {
  value: T;
  label: string;
  content?: React.ReactNode;
};

/** Radio group with roving focus: Tab enters once, arrow keys move and select. */
function RadioGroup<T extends string>({
  labelledBy,
  value,
  options,
  onChange,
  className,
  optionClassName,
}: {
  labelledBy: string;
  value: T;
  options: RadioOption<T>[];
  onChange: (value: T) => void;
  className?: string;
  optionClassName: (selected: boolean) => string;
}) {
  const buttons = useRef<(HTMLButtonElement | null)[]>([]);
  const selectedIndex = Math.max(
    0,
    options.findIndex((option) => option.value === value)
  );

  const handleKeyDown = (event: KeyboardEvent, index: number) => {
    const step =
      event.key === 'ArrowRight' || event.key === 'ArrowDown'
        ? 1
        : event.key === 'ArrowLeft' || event.key === 'ArrowUp'
          ? -1
          : 0;
    const next =
      event.key === 'Home'
        ? 0
        : event.key === 'End'
          ? options.length - 1
          : step
            ? (index + step + options.length) % options.length
            : null;
    if (next === null) return;
    event.preventDefault();
    onChange(options[next].value);
    buttons.current[next]?.focus();
  };

  return (
    <div role="radiogroup" aria-labelledby={labelledBy} className={className}>
      {options.map((option, index) => {
        const selected = index === selectedIndex;
        return (
          <button
            key={option.value}
            ref={(element) => {
              buttons.current[index] = element;
            }}
            type="button"
            role="radio"
            aria-checked={selected}
            tabIndex={selected ? 0 : -1}
            onClick={() => onChange(option.value)}
            onKeyDown={(event) => handleKeyDown(event, index)}
            className={cn(
              'focus:outline-none focus-visible:ring-2 focus-visible:ring-brand focus-visible:ring-offset-2 focus-visible:ring-offset-primary',
              optionClassName(selected)
            )}
          >
            {option.content ?? option.label}
          </button>
        );
      })}
    </div>
  );
}

/**
 * Miniature of the app drawn with the real theme tokens. The wrapper scopes
 * `.light`/`.dark` and `data-color-theme` to the preview only, so a draft
 * choice is never applied to the document before it is saved.
 */
function ColorThemePreview({
  side,
  attribute,
  label,
}: {
  side: ResolvedTheme;
  attribute: string | null;
  label: string;
}) {
  return (
    <span className="flex w-full flex-col gap-half text-left">
      <span
        aria-hidden="true"
        className={cn(
          side,
          'flex h-14 w-full overflow-hidden rounded-sm border border-black/10'
        )}
        data-color-theme={attribute ?? undefined}
      >
        <span className="w-1/4 bg-secondary" />
        <span className="flex flex-1 flex-col gap-1 bg-primary p-1.5">
          <span className="h-1 w-3/4 rounded-full bg-high" />
          <span className="flex flex-1 flex-col justify-center gap-1 rounded-sm bg-panel px-1">
            <span className="h-1 w-2/3 rounded-full bg-low" />
            <span className="h-1 w-1/3 rounded-full bg-brand" />
          </span>
        </span>
      </span>
      <span className="text-sm text-normal">{label}</span>
    </span>
  );
}

export function ColorThemeSettings({
  theme,
  lightColorTheme,
  darkColorTheme,
  onThemeChange,
  onLightColorThemeChange,
  onDarkColorThemeChange,
}: {
  theme: ThemeMode;
  lightColorTheme: LightColorTheme;
  darkColorTheme: DarkColorTheme;
  onThemeChange: (theme: ThemeMode) => void;
  onLightColorThemeChange: (theme: LightColorTheme) => void;
  onDarkColorThemeChange: (theme: DarkColorTheme) => void;
}) {
  const { t } = useTranslation('settings');
  const id = useId();
  const modeLabels: Record<ThemeMode, string> = {
    [ThemeMode.SYSTEM]: t('settings.general.appearance.theme.options.system'),
    [ThemeMode.LIGHT]: t('settings.general.appearance.theme.options.light'),
    [ThemeMode.DARK]: t('settings.general.appearance.theme.options.dark'),
  };
  const standard = t('settings.general.appearance.colorTheme.names.standard');
  const lightNames: Record<LightColorTheme, string> = {
    [LightColorTheme.STANDARD]: standard,
    [LightColorTheme.PAPER]: t(
      'settings.general.appearance.colorTheme.names.paper'
    ),
    [LightColorTheme.LINEN]: t(
      'settings.general.appearance.colorTheme.names.linen'
    ),
  };
  const darkNames: Record<DarkColorTheme, string> = {
    [DarkColorTheme.STANDARD]: standard,
    [DarkColorTheme.NAVY]: t(
      'settings.general.appearance.colorTheme.names.navy'
    ),
    [DarkColorTheme.INDIGO]: t(
      'settings.general.appearance.colorTheme.names.indigo'
    ),
  };
  const rowLabels: Record<ResolvedTheme, string> =
    theme === ThemeMode.SYSTEM
      ? {
          light: t('settings.general.appearance.colorTheme.systemLight'),
          dark: t('settings.general.appearance.colorTheme.systemDark'),
        }
      : {
          light: t('settings.general.appearance.colorTheme.light'),
          dark: t('settings.general.appearance.colorTheme.dark'),
        };
  const themeLabel = t('settings.general.appearance.theme.label');
  const cardClassName = (selected: boolean) =>
    cn(
      'w-36 max-w-full rounded-sm border-2 p-1.5 transition-colors',
      selected ? 'border-brand' : 'border-transparent hover:border-border'
    );

  return (
    <>
      <SettingsField
        label={themeLabel}
        description={t('settings.general.appearance.theme.helper')}
      >
        <span id={`${id}-mode`} className="sr-only">
          {themeLabel}
        </span>
        <RadioGroup
          labelledBy={`${id}-mode`}
          value={theme}
          onChange={onThemeChange}
          options={THEME_MODES.map((mode) => ({
            value: mode,
            label: modeLabels[mode],
          }))}
          className="inline-flex max-w-full overflow-hidden rounded-sm border border-border"
          optionClassName={(selected) =>
            cn(
              'min-h-10 border-r border-border px-base text-sm last:border-r-0',
              selected
                ? 'bg-brand/20 font-medium text-high'
                : 'bg-panel text-normal hover:bg-secondary'
            )
          }
        />
      </SettingsField>

      <SettingsField
        label={t('settings.general.appearance.colorTheme.label')}
        description={t('settings.general.appearance.colorTheme.helper')}
      >
        <div className="space-y-4">
          {colorThemeRows(theme).map((side) => (
            <div key={side} className="space-y-2">
              <p id={`${id}-${side}`} className="text-sm text-low">
                {rowLabels[side]}
              </p>
              {side === 'light' ? (
                <RadioGroup
                  labelledBy={`${id}-${side}`}
                  value={lightColorTheme}
                  onChange={onLightColorThemeChange}
                  options={LIGHT_THEMES.map((value) => ({
                    value,
                    label: lightNames[value],
                    content: (
                      <ColorThemePreview
                        side="light"
                        attribute={lightColorThemeAttribute(value)}
                        label={lightNames[value]}
                      />
                    ),
                  }))}
                  className="flex flex-wrap gap-2"
                  optionClassName={cardClassName}
                />
              ) : (
                <RadioGroup
                  labelledBy={`${id}-${side}`}
                  value={darkColorTheme}
                  onChange={onDarkColorThemeChange}
                  options={DARK_THEMES.map((value) => ({
                    value,
                    label: darkNames[value],
                    content: (
                      <ColorThemePreview
                        side="dark"
                        attribute={darkColorThemeAttribute(value)}
                        label={darkNames[value]}
                      />
                    ),
                  }))}
                  className="flex flex-wrap gap-2"
                  optionClassName={cardClassName}
                />
              )}
            </div>
          ))}
        </div>
      </SettingsField>
    </>
  );
}
