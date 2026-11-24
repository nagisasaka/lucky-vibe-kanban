import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "../../packages/web-core/node_modules/@vitejs/plugin-react";
import tailwindcss from "../../packages/web-core/node_modules/tailwindcss";
import loadConfig from "../../packages/web-core/node_modules/tailwindcss/loadConfig.js";
const root = fileURLToPath(new URL("../..", import.meta.url)).replace(
  /\/$/,
  "",
);
export default defineConfig({
  root: `${root}/tests/run-selection/fixture`,
  envPrefix: "LVK_RUN_SELECTION_FIXTURE_PUBLIC_",
  plugins: [
    react(),
    {
      name: "fixture-theme",
      enforce: "pre",
      transform(code, id) {
        if (id.endsWith("/app/styles/new/index.css"))
          return code.replace(/^@config .*$/m, "");
      },
    },
  ],
  css: {
    postcss: {
      plugins: [
        tailwindcss({
          ...loadConfig(`${root}/packages/local-web/tailwind.new.config.js`),
          content: [
            `${root}/packages/{web-core,ui}/src/**/*.{ts,tsx}`,
            `${root}/tests/run-selection/fixture/*.tsx`,
          ],
        }),
      ],
    },
  },
  resolve: {
    alias: [
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
        "@tanstack/react-query",
      ].map((p) => ({
        find: p,
        replacement: `${root}/packages/web-core/node_modules/${p}`,
      })),
    ],
  },
  server: { hmr: false, fs: { allow: [root] } },
});
