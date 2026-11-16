# Colour theme regression

From the repository root, after `pnpm install --frozen-lockfile` and installing Playwright Chromium (or setting `LVK_E2E_CHROMIUM` to a Chromium/headless shell binary):

```sh
node tests/color-themes/browser.mjs
pnpm --filter @vibe/web-core exec vitest run src/shared/lib/theme.test.ts src/shared/lib/themeContrast.test.ts src/shared/dialogs/settings/settings/ColorThemeSettings.test.tsx
```

The browser fixture bundles the production `ThemeProvider`, the settings colour theme picker and `PrimaryButton`, with Tailwind-processed `index.css` and diff styles. The saved config is fixture state; no backend, shared database, agent or existing development server is used. It checks that a draft selection does not change the document before saving, that System mode follows OS light/dark changes while keeping each side's saved colour theme, that fixed modes show one row, and keyboard focus/arrow selection in the radio groups. It also checks fixed mode immunity to OS changes, retaining both choices when switching modes, and computed button contrast in all six combinations.

Desktop and mobile screenshots of the six saved combinations are written to ignored `test-results/color-themes/`. The contrast unit test reads the shipped tokens directly. The sample represents board, Issue and Workspace surfaces; it does not mount those full application pages or verify the real settings API. Rust config tests separately exercise all nine light/dark pairs through the production file save/load functions.
