import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "../../packages/web-core/node_modules/@vitejs/plugin-react";
import tailwindcss from "../../packages/web-core/node_modules/tailwindcss";
const root = fileURLToPath(new URL("../..", import.meta.url));
const stub = `${root}/tests/integration-skip/fixture/stubs.tsx`;
export default defineConfig({
  root: `${root}/tests/integration-skip/fixture`,
  plugins: [react()],
  css: { postcss: { plugins: [tailwindcss()] } },
  resolve: {
    alias: [
      ...[
        "shared/hooks/useProjectContext",
        "shared/hooks/useAppNavigation",
        "shared/hooks/useUserSystem",
        "shared/hooks/useExecutorConfig",
        "shared/hooks/useAgentProviderPolicy",
        "shared/components/ModelSelectorContainer",
        "shared/dialogs/settings/SettingsDialog",
        "shared/lib/api",
      ].map((p) => ({ find: `@/${p}`, replacement: stub })),
      { find: "@", replacement: `${root}/packages/web-core/src` },
      { find: "shared", replacement: `${root}/shared` },
      ...[
        "react",
        "i18next",
        "react-i18next",
        "react-dom",
        "react-hotkeys-hook",
        "@tanstack/react-query",
      ].map((p) => ({
        find: p,
        replacement: `${root}/packages/web-core/node_modules/${p}`,
      })),
    ],
  },
  server: { hmr: false, fs: { allow: [root] } },
});
