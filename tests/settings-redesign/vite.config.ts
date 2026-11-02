import loadConfig from "../../packages/web-core/node_modules/tailwindcss/loadConfig.js";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "../../packages/web-core/node_modules/@vitejs/plugin-react";
import tailwindcss from "../../packages/web-core/node_modules/tailwindcss";
const root = fileURLToPath(new URL("../..", import.meta.url)).replace(
  /\/$/,
  "",
);
const tokens = loadConfig(`${root}/packages/local-web/tailwind.new.config.js`);
// Only replace dependencies of the named consumer; other product imports stay real.
export default defineConfig({
  root: `${root}/tests/settings-redesign/fixture`,
  publicDir: `${root}/packages/public`,
  envPrefix: "LVK_SETTINGS_FIXTURE_PUBLIC_",
  plugins: [
    {
      name: "fixture-scoped-imports",
      enforce: "pre",
      resolveId(id, importer, options) {
        // Dependency scanning caches resolutions by import string, ignoring owner.
        // Scan real dependencies; apply scoped fixtures only to served modules.
        if (!importer || options.scan) return;
        // Vite's alias plugin may already have expanded @ before this hook.
        const source = id.replace(`${root}/packages/web-core/src/`, "@/");
        return importer
          .split("?")[0]
          .endsWith("/shared/components/ModelSelectorContainer.tsx") &&
          [
            "@/shared/hooks/useUserSystem",
            "@/shared/hooks/useExecutorDiscovery",
          ].includes(source)
          ? `${root}/tests/settings-redesign/fixture/model-selector.tsx`
          : undefined;
      },
    },
    {
      name: "fixture-theme-config",
      enforce: "pre",
      transform(code, id) {
        if (id.endsWith("/app/styles/new/index.css"))
          return code.replace(/^@config .*$/m, "");
      },
    },
    react(),
    {
      name: "fixture-executor-schemas",
      resolveId: (id) =>
        id === "virtual:executor-schemas" ? "\0fixture-schemas" : undefined,
      load: (id) =>
        id === "\0fixture-schemas"
          ? `import codex from "${root}/shared/schemas/codex.json"; import claude from "${root}/shared/schemas/claude_code.json"; export default { CODEX: codex, CLAUDE_CODE: claude };`
          : undefined,
    },
  ],
  optimizeDeps: { include: ["@xyflow/react", "react-hotkeys-hook"] },
  css: {
    postcss: {
      plugins: [
        tailwindcss({
          ...tokens,
          content: [
            `${root}/packages/{web-core,local-web,ui}/src/**/*.{ts,tsx}`,
            `${root}/tests/settings-redesign/fixture/**/*.tsx`,
          ],
        }),
      ],
    },
  },
  resolve: {
    alias: [
      {
        find: "@xyflow/react",
        replacement: `${root}/packages/web-core/node_modules/@xyflow/react`,
      },
      {
        find: /^@vibe\/ui\/lib\/(.+)$/,
        replacement: `${root}/packages/ui/src/lib/$1.ts`,
      },
      {
        find: /^@vibe\/ui\/components\/(.+)$/,
        replacement: `${root}/packages/ui/src/components/$1.tsx`,
      },
      { find: "@", replacement: `${root}/packages/web-core/src` },
      { find: "shared", replacement: `${root}/shared` },
      ...[
        "react",
        "react-dom",
        "i18next",
        "react-i18next",
        "react-hotkeys-hook",
        "@tanstack/react-query",
        "@ebay/nice-modal-react",
      ].map((p) => ({
        find: p,
        replacement: `${root}/packages/web-core/node_modules/${p}`,
      })),
    ],
  },
  server: { hmr: false, fs: { allow: [root] } },
});
