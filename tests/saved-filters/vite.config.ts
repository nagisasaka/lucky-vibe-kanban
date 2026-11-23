import base from "../settings-redesign/vite.config";
import { defineConfig } from "vite";
import { fileURLToPath } from "node:url";
export default defineConfig({
  ...base,
  resolve: {
    ...base.resolve,
    alias: [
      {
        find: "@web/app/router",
        replacement: fileURLToPath(
          new URL("./fixture/navigation-router.ts", import.meta.url),
        ),
      },

      {
        find: "@tanstack/react-router",
        replacement: fileURLToPath(
          new URL(
            "../../packages/web-core/node_modules/@tanstack/react-router",
            import.meta.url,
          ),
        ),
      },
      ...(base.resolve!.alias as []),
    ],
  },
  root: fileURLToPath(new URL("./fixture", import.meta.url)),
});
