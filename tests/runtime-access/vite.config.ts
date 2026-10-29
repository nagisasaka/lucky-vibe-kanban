import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "../../packages/web-core/node_modules/@vitejs/plugin-react";
import tailwindcss from "../../packages/web-core/node_modules/tailwindcss";
const root = fileURLToPath(new URL("../..", import.meta.url));
export default defineConfig({
  root: `${root}/tests/runtime-access/fixture`,
  plugins: [react()],
  css: {
    postcss: {
      plugins: [
        tailwindcss({
          config: `${root}/packages/local-web/tailwind.new.config.js`,
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
      ].map((p) => ({
        find: p,
        replacement: `${root}/packages/web-core/node_modules/${p}`,
      })),
    ],
  },
  server: { hmr: false, fs: { allow: [root] } },
});
