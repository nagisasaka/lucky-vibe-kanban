# Frontend and UI Guidelines

This guide covers frontend changes in `packages/local-web`, `packages/web-core`
and `packages/ui`, as referenced by the root AGENTS.md. Paths below are relative
to this directory unless stated otherwise.

## Layout and Components

- `src/app` and `src/routes` contain the local app shell and routes. Shared frontend features/pages live in `../web-core/src`; reusable components and utilities live in `../ui/src`.
- Follow the existing feature/shared boundaries and nearby file naming. Keep data fetching and orchestration separate from presentation where that pattern is established; do not assume every component belongs in legacy `views/`, `containers/` or `ui-new/` directories.
- Reuse existing UI components before creating new ones. Import UI components through `@vibe/ui/components/*` subpaths, following the package exports and ESLint rules.

## Theme and Styling

- The theme tokens are defined in [`../web-core/src/app/styles/new/index.css`](../web-core/src/app/styles/new/index.css) and mapped by [`tailwind.new.config.js`](tailwind.new.config.js). Use these definitions as the source of truth for colours, typography, spacing and radii.
- Theme variables use `:root` and `.dark`; styling is not confined to a `.new-design` wrapper. Preserve light/dark theme behaviour and mobile text scaling.
- Use semantic text colours such as `text-high`, `text-normal` and `text-low`, and background colours such as `bg-primary`, `bg-secondary` and `bg-panel`. Use the existing `brand`, `error`, `success` and `merged` tokens where appropriate.
- Reuse typography tokens (`text-xs` through `text-xl`, `font-ibm-plex-sans`, `font-ibm-plex-mono`) and spacing tokens (`half`, `base`, `plusfifty`, `double`). Font sizes and spacing are defined in rem; do not rely on old pixel conversions in comments or documentation.
- Follow existing components for borders, rounded corners, hover, disabled and focus states. Preserve visible keyboard focus, accessible labels and contrast; use existing focus-ring utilities instead of removing the outline without a replacement.

## Validation

Follow the root change-based validation guidance. Use the affected packages'
existing type checks, lint scripts and relevant tests; include consuming packages
when changing shared UI. Format changed files with the existing package
configuration. For visual changes, inspect the affected states, screen sizes and
themes where relevant, and report any visual verification that was unavailable.
