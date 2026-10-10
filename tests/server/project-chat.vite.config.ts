import { fileURLToPath } from "node:url";
import { defineConfig, mergeConfig } from "vite";
import workflowFixture from "../workflow/vite.config";

export default mergeConfig(
  workflowFixture,
  defineConfig({
    cacheDir: fileURLToPath(
      new URL("../../node_modules/.vite/project-chat", import.meta.url),
    ),
  }),
);
