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
export default defineConfig({
  root: `${root}/tests/settings-redesign/fixture`,
  envPrefix: "LVK_SETTINGS_FIXTURE_PUBLIC_",
  plugins: [
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
        id === "\0fixture-schemas" ? "export default {}" : undefined,
    },
  ],
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
      { find: "@", replacement: `${root}/packages/web-core/src` },
      { find: "shared", replacement: `${root}/shared` },
      ...[
        "react",
        "react-dom",
        "i18next",
        "react-i18next",
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
